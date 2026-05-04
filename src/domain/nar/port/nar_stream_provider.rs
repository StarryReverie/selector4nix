use std::pin::Pin;

use anyhow::{Error as AnyhowError, Result as AnyhowResult};
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;

use crate::domain::substituter::model::Url;

#[async_trait]
pub trait NarStreamProvider: Send + Sync {
    async fn stream_nar(&self, urls: &[Url]) -> AnyhowResult<Option<NarStreamData>>;
}

pub struct NarStreamData {
    pub stream: NarStreamSource,
    pub source_url: Url,
}

impl NarStreamData {
    pub fn new(stream: NarStreamSource, source_url: Url) -> Self {
        Self { stream, source_url }
    }
}

pub struct NarStreamSource {
    pub headers: NarStreamHeaders,
    pub inner: Pin<Box<dyn Stream<Item = Result<Bytes, AnyhowError>> + Send>>,
}

impl NarStreamSource {
    pub fn new(
        headers: NarStreamHeaders,
        inner: Pin<Box<dyn Stream<Item = Result<Bytes, AnyhowError>> + Send>>,
    ) -> Self {
        Self { headers, inner }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NarStreamHeaders {
    pub content_length: Option<u64>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
}
