use std::num::NonZeroUsize;
use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use bytes::Bytes;
use futures::StreamExt;
use http::{HeaderMap, StatusCode, header};
use reqwest::{
    Client, ClientBuilder, Error as ReqwestError, IntoUrl, RequestBuilder, Response, Url,
};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::SBoxStream;
use crate::stream::{
    BoundedHttpStream, ChunkedStream, ChunkedStreamArgs, FullStream, HttpChunkConnector,
};
use crate::throttler::{PerHostHttpThrottler, ThrottlerAdapter, ThrottlerPermit};

struct StreamingClientContext {
    client: Client,
    throttler: Arc<PerHostHttpThrottler>,
    enable_chunked_streaming: bool,
    chunk_max_len: NonZeroUsize,
    window_max_len: NonZeroUsize,
}

pub struct StreamingClient {
    context: Arc<StreamingClientContext>,
}

impl StreamingClient {
    pub fn new(
        client: ClientBuilder,
        max_concurrent_requests: usize,
        enable_chunked_streaming: bool,
        chunk_max_len: NonZeroUsize,
        window_max_len: NonZeroUsize,
    ) -> Self {
        Self {
            context: Arc::new(StreamingClientContext {
                client: client
                    .http1_only()
                    .build()
                    .expect("invalid reqwest client configuration"),
                throttler: Arc::new(PerHostHttpThrottler::new(max_concurrent_requests)),
                enable_chunked_streaming,
                chunk_max_len,
                window_max_len,
            }),
        }
    }

    pub fn get<U>(&self, url: U) -> StreamingRequest
    where
        U: IntoUrl,
    {
        let url = url.into_url().expect("should be a valid URL");
        let host = url
            .host_str()
            .expect("`url` should have a host")
            .to_string();

        let request = self.context.client.get(url);
        StreamingRequest {
            request,
            host,
            context: Arc::clone(&self.context),
            configure: None,
        }
    }
}

pub struct StreamingRequest {
    request: RequestBuilder,
    host: String,
    configure: Option<Box<dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static>>,
    context: Arc<StreamingClientContext>,
}

impl StreamingRequest {
    pub fn configure<F>(mut self, func: F) -> Self
    where
        F: Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static,
    {
        self.configure = if let Some(prev) = self.configure.take() {
            Some(Box::new(move |request| func(prev(request))))
        } else {
            Some(Box::new(func))
        };
        self
    }

    pub async fn send(self) -> Result<StreamingResponse, StreamHttpBodyError> {
        let permit = self.context.throttler.acquire(&self.host).await;

        let request = (self.configure.as_deref().unwrap_or(&|x| x))(self.request);

        let request = if self.context.enable_chunked_streaming {
            // The first request always asks for the leading chunk so that servers supporting range
            // requests can be served via a chunked response, while servers that ignore `Range` fall
            // back to a full stream.
            request.header(
                header::RANGE,
                format!("bytes=0-{}", usize::from(self.context.chunk_max_len) - 1),
            )
        } else {
            request
        };

        let response = request.send().await.context(TransportSnafu)?;
        StreamingResponse::from_response(response, self.configure, self.host, self.context, permit)
    }
}

pub struct StreamingResponse(StreamingResponseInner);

enum StreamingResponseInner {
    Full {
        response: Response,
        content_length: Option<u64>,
        permit: ThrottlerPermit,
    },
    Chunked {
        response: Response,
        bytes_total: usize,
        initial_chunk_len: usize,
        url: Url,
        host: String,
        configure: Option<Box<dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static>>,
        context: Arc<StreamingClientContext>,
        permit: ThrottlerPermit,
    },
}

impl StreamingResponse {
    fn from_response(
        response: Response,
        configure: Option<Box<dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static>>,
        host: String,
        context: Arc<StreamingClientContext>,
        permit: ThrottlerPermit,
    ) -> Result<StreamingResponse, StreamHttpBodyError> {
        match response.status() {
            StatusCode::OK => {
                tracing::debug!(url = %response.url(), "select full (unchunked) stream");
                Ok(StreamingResponse(StreamingResponseInner::Full {
                    content_length: response.content_length(),
                    response,
                    permit,
                }))
            }
            StatusCode::PARTIAL_CONTENT => {
                let bytes_total = response
                    .headers()
                    .get(header::CONTENT_RANGE)
                    .and_then(|h| h.to_str().ok())
                    .and_then(|h| h.rsplit_once('/'))
                    .and_then(|(_, total)| total.parse::<usize>().ok())
                    .context(InvalidResponseSnafu {
                        message:
                            "206 Partial Content response is missing a valid Content-Range header",
                    })?;

                let initial_chunk_len = response.content_length().map(|len| len as usize).context(
                    InvalidResponseSnafu {
                        message: "206 Partial Content response is missing Content-Length",
                    },
                )?;

                let url = response.url().clone();
                tracing::debug!(%url, ?bytes_total, ?initial_chunk_len, "select chunked stream");
                Ok(StreamingResponse(StreamingResponseInner::Chunked {
                    response,
                    bytes_total,
                    initial_chunk_len,
                    url,
                    host,
                    configure,
                    context,
                    permit,
                }))
            }
            StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => Err(StreamHttpBodyError::NotFound),
            status => Err(StreamHttpBodyError::InvalidStatus { status }),
        }
    }

    pub fn content_length(&self) -> Option<u64> {
        match &self.0 {
            StreamingResponseInner::Full { content_length, .. } => *content_length,
            StreamingResponseInner::Chunked { bytes_total, .. } => Some(*bytes_total as u64),
        }
    }

    pub fn raw_headers(&self) -> &HeaderMap {
        match &self.0 {
            StreamingResponseInner::Full { response, .. } => response.headers(),
            StreamingResponseInner::Chunked { response, .. } => response.headers(),
        }
    }

    pub fn into_stream(self) -> SBoxStream<AnyhowResult<Bytes>> {
        match self.0 {
            StreamingResponseInner::Full {
                response, permit, ..
            } => Box::pin(FullStream::new(
                response.bytes_stream().map(|chunk| {
                    anyhow::Context::with_context(chunk, || "failed to read byte stream")
                }),
                permit,
            )),
            StreamingResponseInner::Chunked {
                response,
                bytes_total,
                initial_chunk_len,
                url,
                configure,
                host,
                context,
                permit,
            } => Box::pin(ChunkedStream::new(ChunkedStreamArgs {
                chunk_max_len: context.chunk_max_len,
                bytes_total,
                window_max_len: context.window_max_len,
                connector: Box::new(HttpChunkConnector::new(
                    context.client.clone(),
                    url,
                    configure,
                )),
                throttler: Box::new(ThrottlerAdapter::new(Arc::clone(&context.throttler), host)),
                initial_permit: permit,
                initial_chunk_stream: Box::pin(BoundedHttpStream::new(
                    Box::pin(response.bytes_stream()),
                    initial_chunk_len,
                )),
            })),
        }
    }
}

#[derive(Snafu, Debug)]
pub enum StreamHttpBodyError {
    #[snafu(display("HTTP transport error"))]
    Transport { source: ReqwestError },
    #[snafu(display("resource not found"))]
    NotFound,
    #[snafu(display("unexpected HTTP status {status}"))]
    InvalidStatus { status: StatusCode },
    #[snafu(display("invalid response from the server: {message}"))]
    InvalidResponse { message: &'static str },
}
