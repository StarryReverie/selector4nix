use std::sync::Arc;

use crate::application::nar_file::actor::{NarFileActorRegistry, NarFileRequest};
use crate::application::nar_info::actor::{NarInfoActorRegistry, NarInfoRequest};
use crate::application::substituter::actor::{SubstituterActorRegistry, SubstituterRequest};
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_file::model::{NarFileKey, NarFileLocation};
use crate::domain::nar_info::ResolveNarInfoEvent;
use crate::domain::nar_info::model::{ProxyNarInfoData, StorePathHash};
use crate::{AppError, AppResultExt};

pub struct NarInfoResolutionUseCase {
    nar_info_registry: Arc<NarInfoActorRegistry>,
    substituter_registry: Arc<SubstituterActorRegistry>,
    nar_file_registry: Arc<NarFileActorRegistry>,
}

impl NarInfoResolutionUseCase {
    pub fn new(
        nar_info_registry: Arc<NarInfoActorRegistry>,
        substituter_registry: Arc<SubstituterActorRegistry>,
        nar_file_registry: Arc<NarFileActorRegistry>,
    ) -> Self {
        Self {
            nar_info_registry,
            substituter_registry,
            nar_file_registry,
        }
    }

    pub async fn get_nar_info(
        &self,
        hash: StorePathHash,
        headers: PassthroughHeaders,
    ) -> Result<ProxyNarInfoData, AppError> {
        tracing::info!(hash = %hash.value(), "resolving nar info");

        let address = self.nar_info_registry.get(&hash).await;

        let response = address
            .ask(|reply_to| NarInfoRequest::ResolveNarInfo { reply_to, headers })
            .await
            .throw_catastrophic("`NarInfoActor` terminated unexpectedly")?;

        self.exec_events(response.events).await;

        match response.result {
            Ok(Some(data)) => {
                tracing::info!(hash = %hash.value(), nar_file = %data.nar_file().value(), "resolved nar info");
                Ok(data)
            }
            Ok(None) => {
                tracing::info!(hash = %hash.value(), "resolved nar info with not-found");
                Err(AppError::not_found(
                    "could not resolve non-existent nar info",
                ))
            }
            Err(err) => {
                tracing::warn!(hash = %hash.value(), %err, "failed to resolve nar info");
                Err(err)
            }
        }
    }

    async fn exec_events(&self, events: Vec<ResolveNarInfoEvent>) {
        for event in events {
            self.exec_event(event).await;
        }
    }

    async fn exec_event(&self, event: ResolveNarInfoEvent) {
        match event {
            ResolveNarInfoEvent::SubstituterSucceeded(url) => {
                let sender = self.substituter_registry.get(&url).await;
                let _ = sender.tell(SubstituterRequest::ServiceSuccessful).await;
            }
            ResolveNarInfoEvent::SubstituterOffline(url) => {
                let sender = self.substituter_registry.get(&url).await;
                let _ = sender.tell(SubstituterRequest::ServiceOffline).await;
            }
            ResolveNarInfoEvent::SubstituterError(url) => {
                let sender = self.substituter_registry.get(&url).await;
                let _ = sender.tell(SubstituterRequest::ServiceError).await;
            }
            ResolveNarInfoEvent::NarFileLocated {
                nar_file,
                substituter,
                source_url,
                store_path_hash,
            } => {
                let nar_file_key = NarFileKey::from_file_name(&nar_file);
                let location = NarFileLocation::new(
                    source_url,
                    substituter.clone(),
                    substituter.nar_timeout(),
                );
                let sender = self.nar_file_registry.get(&nar_file_key).await;
                let _ = sender
                    .tell(NarFileRequest::SetLocation {
                        location,
                        store_path_hash,
                    })
                    .await;
            }
        }
    }
}
