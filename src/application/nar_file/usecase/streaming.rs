use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Result as AnyhowResult;
use bytes::Bytes;
use futures::Stream;

use crate::application::nar_file::actor::{NarFileActorRegistry, NarFileRequest};
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_file::model::NarFileKey;
use crate::domain::nar_file::port::NarStreamData;
use crate::domain::nar_info::NarInfoRepository;
use crate::domain::nar_info::model::StorePathHash;
use crate::infrastructure::metric::{NarTransferHandle, NarTransferMeta, NarTransferMetric};
use crate::{AppError, AppResultExt};

pub struct NarFileStreamingUseCase {
    nar_file_registry: Arc<NarFileActorRegistry>,
    nar_info_repository: Arc<dyn NarInfoRepository>,
    nar_transfer_metric: Arc<NarTransferMetric>,
}

impl NarFileStreamingUseCase {
    pub fn new(
        nar_file_registry: Arc<NarFileActorRegistry>,
        nar_info_repository: Arc<dyn NarInfoRepository>,
        nar_transfer_metric: Arc<NarTransferMetric>,
    ) -> Self {
        Self {
            nar_file_registry,
            nar_info_repository,
            nar_transfer_metric,
        }
    }

    pub async fn stream_nar(
        &self,
        key: NarFileKey,
        headers: PassthroughHeaders,
    ) -> Result<NarStreamData, AppError> {
        tracing::info!(nar_file = %key.to_file_name().value(), "acquiring nar stream from substituter");

        let address = self.nar_file_registry.get(&key).await;

        let response = address
            .ask(|reply_to| NarFileRequest::StreamNarFile { reply_to, headers })
            .await
            .throw_catastrophic("`NarFileActor` terminated unexpectedly")?;

        let result = response
            .inspect(|result| tracing::info!(nar_file = %key.to_file_name().value(), source_url = %result.stream.source_url, substituter = %result.substituter.url(), "streamed nar from substituter"))
            .inspect_err(|err| tracing::warn!(nar_file = %key.to_file_name().value(), %err, "failed to stream nar"))?;

        Ok(instrument_stream(
            &self.nar_transfer_metric,
            NarTransferMeta {
                nar_file_name: key.to_file_name(),
                store_path: self.query_store_path(result.store_path_hash.as_ref()).await,
                substituter_url: result.substituter.url().clone(),
                source_url: result.stream.source_url.clone(),
                content_length: result.stream.headers.content_length,
            },
            result.stream,
        ))
    }

    async fn query_store_path(&self, hash: Option<&StorePathHash>) -> Option<String> {
        let nar_info = self.nar_info_repository.get(hash?).await.ok().flatten()?;
        nar_info
            .nar_info()
            .and_then(|data| data.store_path().map(ToString::to_string))
    }
}

fn instrument_stream(
    metric: &Arc<NarTransferMetric>,
    meta: NarTransferMeta,
    data: NarStreamData,
) -> NarStreamData {
    let stream = Box::pin(InstrumentedNarStream {
        inner: data.inner,
        handle: metric.begin(meta),
    });
    NarStreamData::new(data.headers, stream, data.source_url)
}

struct InstrumentedNarStream {
    inner: Pin<Box<dyn Stream<Item = AnyhowResult<Bytes>> + Send>>,
    handle: NarTransferHandle,
}

impl Stream for InstrumentedNarStream {
    type Item = AnyhowResult<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                self.handle.record_bytes(bytes.len() as u64);
                Poll::Ready(Some(Ok(bytes)))
            }
            other => other,
        }
    }
}
