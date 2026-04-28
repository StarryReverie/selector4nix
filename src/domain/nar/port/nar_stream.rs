use std::pin::Pin;

use anyhow::{Error as AnyhowError, Result as AnyhowResult};
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;

use crate::domain::substituter::model::Url;

pub struct NarStream {
    pub inner: Pin<Box<dyn Stream<Item = Result<Bytes, AnyhowError>> + Send>>,
}

pub enum NarStreamOutcome {
    Found { stream: NarStream, source_url: Url },
    NotFound,
}

#[async_trait]
pub trait NarStreamProvider: Send + Sync {
    async fn stream_nar(&self, urls: &[Url]) -> AnyhowResult<NarStreamOutcome>;
}
