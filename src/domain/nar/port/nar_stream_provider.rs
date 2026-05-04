use std::pin::Pin;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;

use crate::domain::substituter::model::Url;

#[async_trait]
pub trait NarStreamProvider: Send + Sync {
    async fn stream_nar(&self, urls: &[Url]) -> AnyhowResult<Option<NarStreamData>>;
}

pub struct NarStreamData {
    pub headers: NarStreamHeaders,
    pub inner: Pin<Box<dyn Stream<Item = AnyhowResult<Bytes>> + Send>>,
    pub source_url: Url,
}

impl NarStreamData {
    pub fn new(
        headers: NarStreamHeaders,
        inner: Pin<Box<dyn Stream<Item = AnyhowResult<Bytes>> + Send>>,
        source_url: Url,
    ) -> Self {
        Self {
            headers,
            inner,
            source_url,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NarStreamHeaders {
    pub content_length: Option<u64>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
}
