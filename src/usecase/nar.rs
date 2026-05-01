use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use selector4nix_actor::actor::AnyAddress;
use tokio::sync::oneshot;

use crate::domain::nar::actor::{NarActorEffect, NarRequest, ResolveNarInfoError};
use crate::domain::nar::index::{NarFileEvent, NarFileIndex};
use crate::domain::nar::model::{NarFileName, NarInfoData, StorePathHash};
use crate::domain::nar::port::{NarStreamOutcome, NarStreamProvider};
use crate::domain::substituter::actor::SubstituterRequest;
use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::domain::substituter::model::Url;
use crate::infrastructure::registry::NarActorRegistry;
use crate::infrastructure::registry::SubstituterActorRegistry;

pub struct NarUseCase {
    nar_registry: Arc<NarActorRegistry>,
    substituter_registry: Arc<SubstituterActorRegistry>,
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
    nar_stream_provider: Arc<dyn NarStreamProvider>,
    nar_file_index: Arc<dyn NarFileIndex>,
    nar_file_index_pub: AnyAddress<NarFileEvent>,
}

impl NarUseCase {
    pub fn new(
        nar_registry: Arc<NarActorRegistry>,
        substituter_registry: Arc<SubstituterActorRegistry>,
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
        nar_stream_provider: Arc<dyn NarStreamProvider>,
        nar_file_index: Arc<dyn NarFileIndex>,
        nar_file_index_pub: AnyAddress<NarFileEvent>,
    ) -> Self {
        Self {
            nar_registry,
            substituter_registry,
            substituter_availability_index,
            nar_stream_provider,
            nar_file_index,
            nar_file_index_pub,
        }
    }

    pub async fn get_nar_info(
        &self,
        hash: StorePathHash,
    ) -> Result<NarInfoData, ResolveNarInfoError> {
        tracing::info!(hash = %hash.value(), "resolving nar info");

        let address = self.nar_registry.get(&hash).await;

        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = address.tell(NarRequest::ResolveNarInfo(reply_tx)).await;
        let response = reply_rx.await.expect("nar actor shouldn't be dropped");

        match &response.result {
            Ok(data) => {
                tracing::info!(hash = %hash.value(), nar_file = %data.nar_file().value(), "resolved nar info");
            }
            Err(ResolveNarInfoError::NotFound) => {
                tracing::info!(hash = %hash.value(), "failed to find nar info")
            }
            Err(ResolveNarInfoError::Fetch) => {
                tracing::warn!(hash = %hash.value(), "failed to resolve nar info")
            }
        }

        self.exec_effects(response.effects).await;
        response.result
    }

    pub async fn stream_nar(&self, nar_file: &NarFileName) -> AnyhowResult<NarStreamOutcome> {
        tracing::info!(nar_file = %nar_file.value(), "acquiring nar stream from substituter");

        if let Some(prefix) = &self.nar_file_index.get_storage_prefix(nar_file).await {
            tracing::info!(nar_file = %nar_file.value(), "use cached nar file location");

            let urls = [nar_file.with_storage_prefix(prefix)];
            let outcome = self.nar_stream_provider.stream_nar(&urls).await;

            if let s @ Ok(NarStreamOutcome::Found { .. }) = outcome {
                return s;
            } else {
                tracing::warn!(nar_file = %nar_file.value(), "fallback to query all substituters for nar file location")
            }
        } else {
            tracing::info!(nar_file = %nar_file.value(), "query all substituters for nar file location");
        }

        self.stream_nar_from_all(nar_file).await
    }

    async fn stream_nar_from_all(&self, nar_file: &NarFileName) -> AnyhowResult<NarStreamOutcome> {
        let urls = self.build_fallback_urls(nar_file);
        let outcome = self.nar_stream_provider.stream_nar(&urls).await;

        match &outcome {
            Ok(NarStreamOutcome::Found { source_url, .. }) => {
                tracing::info!(nar_file = %nar_file.value(), source_url = %source_url, "streamed nar from substituter");
                let request = NarFileEvent::Registered {
                    nar_file: nar_file.clone(),
                    storage_prefix: source_url.get_dir(),
                };
                let _ = self.nar_file_index_pub.tell(request).await;
            }
            Ok(NarStreamOutcome::NotFound) => {
                tracing::info!(nar_file = %nar_file.value(), "failed to find nar file in any substituter");
            }
            Err(_) => {
                tracing::warn!(nar_file = %nar_file.value(), "failed to stream nar");
            }
        }

        outcome
    }

    fn build_fallback_urls(&self, nar_file: &NarFileName) -> Vec<Url> {
        self.substituter_availability_index
            .query_all()
            .iter()
            .map(|substituter| {
                let prefix = substituter.target().storage_url();
                nar_file.with_storage_prefix(prefix)
            })
            .collect()
    }

    async fn exec_effects(&self, effects: Vec<NarActorEffect>) {
        for effect in effects {
            self.exec_effect(effect).await;
        }
    }

    async fn exec_effect(&self, effect: NarActorEffect) {
        match effect {
            NarActorEffect::ReportSubstituterSuccess(url) => {
                let sender = self.substituter_registry.get(&url).await;
                let _ = sender.tell(SubstituterRequest::ServiceSuccessful).await;
            }
            NarActorEffect::ReportSubstituterFailure(url) => {
                let sender = self.substituter_registry.get(&url).await;
                let _ = sender.tell(SubstituterRequest::ServiceFailed).await;
            }
        }
    }
}
