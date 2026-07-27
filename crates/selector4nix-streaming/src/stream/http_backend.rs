use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{Context as _, Result as AnyhowResult};
use bytes::Bytes;
use futures::Stream;
use http::{StatusCode, header};
use reqwest::{Client, Error as ReqwestError, RequestBuilder, Url};

use crate::stream::ChunkConnector;
use crate::{SBoxFuture, SBoxStream};

pub struct HttpChunkConnector {
    client: Client,
    url: Url,
    configure: Option<Box<dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static>>,
}

impl HttpChunkConnector {
    pub fn new(
        client: Client,
        url: Url,
        configure: Option<Box<dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static>>,
    ) -> Self {
        Self {
            client,
            url,
            configure,
        }
    }
}

impl ChunkConnector for HttpChunkConnector {
    fn get(
        &self,
        offset: usize,
        len: usize,
    ) -> SBoxFuture<AnyhowResult<SBoxStream<AnyhowResult<Bytes>>>> {
        let end = offset
            .checked_add(len)
            .expect("chunk range end must not overflow");
        let range = format!("bytes={offset}-{}", end - 1);

        let url = self.url.clone();
        let request = self.client.get(url.clone());
        let request = (self.configure.as_deref().unwrap_or(&|r| r))(request);
        let request = request.header(header::RANGE, range);

        Box::pin(async move {
            let response = request
                .send()
                .await
                .with_context(|| format!("failed to request chunk [{offset}, {end})"))
                .map_err(|err| {
                    tracing::debug!(%url, ?offset, ?len, %err, "chunk request failed");
                    err
                })?;

            let status = response.status();
            if status != StatusCode::PARTIAL_CONTENT {
                tracing::debug!(%url, ?offset, ?len, %status, "received unexpected chunk response status");
                return Err(anyhow::anyhow!(
                    "expected 206 Partial Content for chunk [{offset}, {end}), got {status}"
                ));
            }

            let bounded: SBoxStream<AnyhowResult<Bytes>> = Box::pin(BoundedHttpStream::new(
                Box::pin(response.bytes_stream()),
                len,
            ));
            Ok(bounded)
        })
    }
}

pub struct BoundedHttpStream {
    inner: SBoxStream<Result<Bytes, ReqwestError>>,
    remaining: usize,
}

impl BoundedHttpStream {
    pub fn new(inner: SBoxStream<Result<Bytes, ReqwestError>>, remaining: usize) -> Self {
        Self { inner, remaining }
    }
}

impl Stream for BoundedHttpStream {
    type Item = AnyhowResult<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.remaining == 0 {
            return Poll::Ready(None);
        }

        match this.inner.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                tracing::debug!(remaining = ?this.remaining, "chunk body ended prematurely");
                Poll::Ready(Some(Err(anyhow::anyhow!(
                    "chunk stream ended prematurely: {} byte(s) short",
                    this.remaining
                ))))
            }
            Poll::Ready(Some(Err(err))) => {
                Poll::Ready(Some(Err(err).context("failed to read chunk stream")))
            }
            Poll::Ready(Some(Ok(bytes))) => {
                if bytes.len() <= this.remaining {
                    this.remaining -= bytes.len();
                    if this.remaining == 0 {
                        tracing::trace!("chunk body fully received");
                    }
                    Poll::Ready(Some(Ok(bytes)))
                } else {
                    let got = bytes.len();
                    tracing::trace!(?got, remaining = ?this.remaining, "truncated over-delivered chunk body");
                    let head = bytes.slice(..this.remaining);
                    this.remaining = 0;
                    Poll::Ready(Some(Ok(head)))
                }
            }
        }
    }
}
