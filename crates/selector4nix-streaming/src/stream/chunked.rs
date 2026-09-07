use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::{Error as AnyhowError, Result as AnyhowResult};
use bytes::Bytes;
use futures::{FutureExt, Stream, StreamExt};

use crate::throttler::ThrottlerPermit;
use crate::{SBoxFuture, SBoxStream};

const CHUNK_RETRY_BACKOFF_BASE_MS: u64 = 100;
const CHUNK_RETRY_BACKOFF_MAX_SHIFT: usize = 5;

pub trait ChunkConnector: Send + Sync + Unpin {
    fn get(
        &self,
        offset: usize,
        len: usize,
    ) -> SBoxFuture<AnyhowResult<SBoxStream<AnyhowResult<Bytes>>>>;
}

pub trait ChunkTrottler: Send + Sync + Unpin {
    fn try_acquire(&self) -> Option<ThrottlerPermit>;
}

pub struct ChunkedStreamArgs {
    pub chunk_max_len: NonZeroUsize,
    pub bytes_total: usize,
    pub window_max_len: NonZeroUsize,
    pub max_retries: usize,
    pub connector: Box<dyn ChunkConnector>,
    pub throttler: Box<dyn ChunkTrottler>,
    pub initial_permit: ThrottlerPermit,
    pub initial_chunk_stream: SBoxStream<AnyhowResult<Bytes>>,
}

pub struct ChunkedStream {
    chunk_max_len: NonZeroUsize,
    bytes_total: usize,
    bytes_consumed: usize,
    bytes_received: usize,
    window: VecDeque<Chunk>,
    window_offset: usize,
    window_max_len: NonZeroUsize,
    max_retries: usize,
    connector: Box<dyn ChunkConnector>,
    throttler: Box<dyn ChunkTrottler>,
    permits: Vec<ThrottlerPermit>,
}

impl ChunkedStream {
    pub fn new(args: ChunkedStreamArgs) -> Self {
        Self {
            chunk_max_len: args.chunk_max_len,
            bytes_total: args.bytes_total,
            bytes_consumed: 0,
            bytes_received: 0,
            window: if args.bytes_total > 0 {
                vec![Chunk::Transferring {
                    buffer: VecDeque::new(),
                    stream: args.initial_chunk_stream,
                    bytes_received: 0,
                    retries_used: 0,
                }]
                .into()
            } else {
                VecDeque::new()
            },
            window_offset: 0,
            window_max_len: args.window_max_len,
            max_retries: args.max_retries,
            connector: args.connector,
            throttler: args.throttler,
            permits: if args.bytes_total > 0 {
                vec![args.initial_permit]
            } else {
                Vec::new()
            },
        }
    }

    fn poll_next_impl(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<AnyhowResult<Bytes>>> {
        let this = self.get_mut();

        tracing::trace!(window_offset = ?this.window_offset, window_len = ?this.window.len(), bytes_consumed = ?this.bytes_consumed, "poll chunked stream");

        // Strip the first consumed chunks.
        while this.window.pop_front_if(|c| c.is_exhausted()).is_some() {
            this.window_offset += 1;
        }

        // Try to acquire throttler permits to start transferring more chunks.
        let chunks_total = this.bytes_total.div_ceil(usize::from(this.chunk_max_len));
        let chunks_not_finished = this.window.iter().filter(|c| !c.is_finished()).count();
        let mut acquired_free_permits = this.permits.len().saturating_sub(chunks_not_finished);
        while this.window_offset + this.window.len() < chunks_total
            && this.window.len() < usize::from(this.window_max_len)
        {
            if acquired_free_permits > 0 {
                // Some permits may not be returned to avoid contention. Use these permits first.
                acquired_free_permits -= 1;
            } else {
                // Try to acquire a permit. Because this method doesn't wait for a permits becoming
                // available, the acquisition will fail immediately if the load is high, where
                // streaming different files is preferred over streaming chunks of a single file
                // concurrently.
                if let Some(permit) = this.throttler.try_acquire() {
                    this.permits.push(permit);
                } else {
                    tracing::trace!(window_offset = ?this.window_offset, window_len = ?this.window.len(), "defer chunk launches while throttle saturated");
                    break;
                }
            }

            let range = this.chunk_range(this.window.len());
            tracing::trace!(?range.offset, ?range.len, "launch chunk transfer");
            let future = this.connector.get(range.offset, range.len);
            this.window.push_back(Chunk::Connecting { future });
        }

        // Poll all chunks' `Future`s or `Stream`s, and exit if any error occurred.
        for i in 0..this.window.len() {
            let res = match this.window[i] {
                Chunk::Connecting { .. } => this.poll_connecting_chunk(i, cx),
                Chunk::Reconnecting { .. } => this.poll_reconnecting_chunk(i, cx),
                Chunk::Transferring { .. } => this.poll_transferring_chunk(i, cx),
                Chunk::Finished { .. } => Ok(()),
            };
            if let Err(err) = res {
                return Poll::Ready(Some(Err(err)));
            }
        }

        // Try to consume the first `Bytes` from the window's start. A chunk waiting out a
        // retry backoff still serves its retained buffer here.
        if let Some(front) = this.window.front_mut() {
            match front.consume() {
                Some(bytes) => {
                    this.bytes_consumed += bytes.len();
                    Poll::Ready(Some(Ok(bytes)))
                }
                None => Poll::Pending,
            }
        } else {
            tracing::debug!(chunks = ?this.window_offset, ?this.bytes_consumed, ?this.bytes_received, ?this.bytes_total, "completed chunked stream");
            Poll::Ready(None)
        }
    }

    fn chunk_range(&self, index: usize) -> ChunkRange {
        let offset = (self.window_offset + index) * usize::from(self.chunk_max_len);
        let len = (self.bytes_total - offset).min(usize::from(self.chunk_max_len));
        ChunkRange { offset, len }
    }

    fn chunk_retry_future(
        &self,
        offset: usize,
        len: usize,
        retries_used: usize,
    ) -> SBoxFuture<AnyhowResult<SBoxStream<AnyhowResult<Bytes>>>> {
        let future = self.connector.get(offset, len);
        let backoff = Duration::from_millis(
            CHUNK_RETRY_BACKOFF_BASE_MS
                * (1u64 << (retries_used - 1).min(CHUNK_RETRY_BACKOFF_MAX_SHIFT)),
        );
        Box::pin(async move {
            tokio::time::sleep(backoff).await;
            future.await
        })
    }

    fn retry_chunk(
        &mut self,
        index: usize,
        err: AnyhowError,
        buffer: VecDeque<Bytes>,
        bytes_received: usize,
        retries_used: usize,
        cx: &mut Context<'_>,
    ) -> AnyhowResult<()> {
        let ChunkRange {
            offset,
            len: chunk_len,
        } = self.chunk_range(index);
        let retry_len = chunk_len.saturating_sub(bytes_received);
        if retry_len == 0 {
            // Everything has already arrived, so the trailing error is discarded instead of
            // dropping healthy data.
            self.window[index] = Chunk::Finished { buffer };
            return Ok(());
        }
        if retries_used >= self.max_retries {
            // If an error occurred, return the error earlier and the consumer will cancel
            // this `Stream`. We also need to release all resources and terminate this
            // `Stream` in case that the consumer continues to call `poll_next()`.
            tracing::warn!(?offset, retries_used, attempts = ?self.max_retries, %err, "chunk transfer failed, retries exhausted");
            self.clear();
            return Err(err);
        }

        let retries_used = retries_used + 1;
        let retry_offset = offset + bytes_received;
        let future = self.chunk_retry_future(retry_offset, retry_len, retries_used);
        tracing::warn!(?offset, ?retry_offset, ?retry_len, ?bytes_received, retries_used, %err, "chunk transfer failed, retrying from received offset");
        self.window[index] = Chunk::Reconnecting {
            future,
            buffer,
            bytes_received,
            retries_used,
        };
        // The retry future is delayed by a backoff, so poll it right away to register its
        // waker, or the stream would stall if no other chunk produces a wake-up.
        self.poll_reconnecting_chunk(index, cx)
    }

    fn poll_connecting_chunk(&mut self, index: usize, cx: &mut Context<'_>) -> AnyhowResult<()> {
        let offset = self.chunk_range(index).offset;
        let chunk = &mut self.window[index];
        let Chunk::Connecting { future } = chunk else {
            unreachable!(
                "`self.window[idx]` should be `Chunk::Connecting` if `poll_connecting_chunk` is called"
            );
        };

        match future.poll_unpin(cx) {
            Poll::Ready(Ok(stream)) => {
                tracing::trace!(?offset, "chunk transfer started");
                *chunk = Chunk::Transferring {
                    buffer: VecDeque::new(),
                    stream,
                    bytes_received: 0,
                    retries_used: 0,
                };
                self.poll_transferring_chunk(index, cx)
            }
            Poll::Ready(Err(err)) => self.retry_chunk(index, err, VecDeque::new(), 0, 0, cx),
            Poll::Pending => Ok(()),
        }
    }

    fn poll_reconnecting_chunk(&mut self, index: usize, cx: &mut Context<'_>) -> AnyhowResult<()> {
        let offset = self.chunk_range(index).offset;
        let chunk = &mut self.window[index];
        let Chunk::Reconnecting {
            future,
            buffer,
            bytes_received,
            retries_used,
        } = chunk
        else {
            unreachable!(
                "`self.window[idx]` should be `Chunk::Reconnecting` if `poll_reconnecting_chunk` is called"
            );
        };

        match future.poll_unpin(cx) {
            Poll::Ready(Ok(stream)) => {
                tracing::trace!(?offset, "chunk transfer started");
                *chunk = Chunk::Transferring {
                    buffer: std::mem::take(buffer),
                    stream,
                    bytes_received: *bytes_received,
                    retries_used: *retries_used,
                };
                self.poll_transferring_chunk(index, cx)
            }
            Poll::Ready(Err(err)) => {
                let buffer = std::mem::take(buffer);
                let bytes_received = *bytes_received;
                let retries_used = *retries_used;
                self.retry_chunk(index, err, buffer, bytes_received, retries_used, cx)
            }
            Poll::Pending => Ok(()),
        }
    }

    fn poll_transferring_chunk(&mut self, index: usize, cx: &mut Context<'_>) -> AnyhowResult<()> {
        let offset = self.chunk_range(index).offset;
        let chunk = &mut self.window[index];
        let Chunk::Transferring {
            buffer,
            stream,
            bytes_received,
            retries_used,
        } = chunk
        else {
            unreachable!(
                "`self.window[idx]` should be `Chunk::Transferring` if `poll_tranferring_chunk` is called"
            );
        };

        while let Poll::Ready(produced) = stream.poll_next_unpin(cx) {
            match produced {
                Some(Ok(bytes)) => {
                    let len = bytes.len();
                    self.bytes_received += len;
                    buffer.push_back(bytes);
                    *bytes_received += len;
                }
                Some(Err(err)) => {
                    let buffer = std::mem::take(buffer);
                    let bytes_received = *bytes_received;
                    let retries_used = *retries_used;
                    return self.retry_chunk(index, err, buffer, bytes_received, retries_used, cx);
                }
                None => {
                    // If the `Stream` for this chunk has been exhausted, then release the `Stream`
                    // and change this chunk's state to `Finished`.
                    let buffer = std::mem::take(buffer);
                    *chunk = Chunk::Finished { buffer };
                    tracing::trace!(?offset, bytes_received = ?self.bytes_received, "chunk transfer finished");

                    // The corresponding throttler permit is also released, except it's the only
                    // one that acquired currently. This prevents stalling the entire stream where
                    // all permits are returned but no permit can be acquired afterwards due to
                    // contention.
                    if self.permits.len() > 1 || self.bytes_received >= self.bytes_total {
                        tracing::trace!(?offset, permits = ?self.permits.len(), bytes_received = ?self.bytes_received, "release chunk permit");
                        self.permits.pop();
                    } else {
                        tracing::trace!(?offset, permits = ?self.permits.len(), bytes_received = ?self.bytes_received, "retain chunk permit");
                    }

                    break;
                }
            }
        }

        Ok(())
    }

    fn clear(&mut self) {
        self.bytes_total = 0;
        self.bytes_consumed = 0;
        self.bytes_received = 0;
        self.window_offset = 0;
        self.window.clear();
        self.permits.clear();
    }
}

impl Stream for ChunkedStream {
    type Item = AnyhowResult<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.poll_next_impl(cx)
    }
}

struct ChunkRange {
    offset: usize,
    len: usize,
}

enum Chunk {
    Connecting {
        future: SBoxFuture<AnyhowResult<SBoxStream<AnyhowResult<Bytes>>>>,
    },
    // Waiting out the retry backoff and re-opening the connection, resuming from
    // `chunk_offset + bytes_received` instead of the chunk start.
    Reconnecting {
        future: SBoxFuture<AnyhowResult<SBoxStream<AnyhowResult<Bytes>>>>,
        buffer: VecDeque<Bytes>,
        bytes_received: usize,
        retries_used: usize,
    },
    Transferring {
        buffer: VecDeque<Bytes>,
        stream: SBoxStream<AnyhowResult<Bytes>>,
        bytes_received: usize,
        retries_used: usize,
    },
    Finished {
        buffer: VecDeque<Bytes>,
    },
}

impl Chunk {
    fn is_finished(&self) -> bool {
        matches!(self, Self::Finished { .. })
    }

    fn is_exhausted(&self) -> bool {
        match self {
            Self::Connecting { .. } => false,
            Self::Reconnecting { .. } => false,
            Self::Transferring { .. } => false,
            Self::Finished { buffer } => buffer.is_empty(),
        }
    }

    fn consume(&mut self) -> Option<Bytes> {
        match self {
            Self::Connecting { .. } => None,
            Self::Reconnecting { buffer, .. } => buffer.pop_front(),
            Self::Transferring { buffer, .. } => buffer.pop_front(),
            Self::Finished { buffer } => buffer.pop_front(),
        }
    }
}

#[cfg(test)]
#[path = "./chunked_tests.rs"]
mod tests;
