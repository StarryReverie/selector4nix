use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result as AnyhowResult;
use bytes::Bytes;
use futures::{FutureExt, Stream, StreamExt};

use crate::throttler::ThrottlerPermit;
use crate::{SBoxFuture, SBoxStream};

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
                }]
                .into()
            } else {
                VecDeque::new()
            },
            window_offset: 0,
            window_max_len: args.window_max_len,
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

            let offset = (this.window_offset + this.window.len()) * usize::from(this.chunk_max_len);
            let len = (this.bytes_total - offset).min(usize::from(this.chunk_max_len));
            tracing::trace!(?offset, ?len, "launch chunk transfer");
            let future = this.connector.get(offset, len);
            this.window.push_back(Chunk::Connecting { future });
        }

        // Poll all chunks' `Future`s or `Stream`s, and exit if any error occurred.
        for i in 0..this.window.len() {
            let res = match this.window[i] {
                Chunk::Connecting { .. } => this.poll_connecting_chunk(i, cx),
                Chunk::Transferring { .. } => this.poll_transferring_chunk(i, cx),
                Chunk::Finished { .. } => Ok(()),
            };
            if let Err(err) = res {
                return Poll::Ready(Some(Err(err)));
            }
        }

        // Try to consume the first `Bytes` from the window's start.
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

    fn poll_connecting_chunk(&mut self, index: usize, cx: &mut Context<'_>) -> AnyhowResult<()> {
        let offset = (self.window_offset + index) * usize::from(self.chunk_max_len);
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
                };
                self.poll_transferring_chunk(index, cx)
            }
            Poll::Ready(Err(err)) => {
                // If an error occurred, return the error earlier and the consumer will cancel
                // this `Stream`. We also need to release all resources and terminate this
                // `Stream` in case that the consumer continues to call `poll_next()`.
                self.clear();
                Err(err)
            }
            Poll::Pending => Ok(()),
        }
    }

    fn poll_transferring_chunk(&mut self, index: usize, cx: &mut Context<'_>) -> AnyhowResult<()> {
        let offset = (self.window_offset + index) * usize::from(self.chunk_max_len);
        let chunk = &mut self.window[index];
        let Chunk::Transferring { buffer, stream } = chunk else {
            unreachable!(
                "`self.window[idx]` should be `Chunk::Transferring` if `poll_tranferring_chunk` is called"
            );
        };

        while let Poll::Ready(produced) = stream.poll_next_unpin(cx) {
            match produced {
                Some(Ok(bytes)) => {
                    self.bytes_received += bytes.len();
                    buffer.push_back(bytes);
                }
                Some(Err(err)) => {
                    // If an error occurred, return the error earlier and the consumer will cancel
                    // this `Stream`. We also need to release all resources and terminate this
                    // `Stream` in case that the consumer continues to call `poll_next()`.
                    self.clear();
                    return Err(err);
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

enum Chunk {
    Connecting {
        future: SBoxFuture<AnyhowResult<SBoxStream<AnyhowResult<Bytes>>>>,
    },
    Transferring {
        buffer: VecDeque<Bytes>,
        stream: SBoxStream<AnyhowResult<Bytes>>,
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
            Self::Transferring { .. } => false,
            Self::Finished { buffer } => buffer.is_empty(),
        }
    }

    fn consume(&mut self) -> Option<Bytes> {
        match self {
            Self::Connecting { .. } => None,
            Self::Transferring { buffer, .. } => buffer.pop_front(),
            Self::Finished { buffer } => buffer.pop_front(),
        }
    }
}

#[cfg(test)]
#[path = "./chunked_tests.rs"]
mod tests;
