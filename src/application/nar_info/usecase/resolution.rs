use std::sync::Arc;

use crate::application::nar_file::actor::{NarFileActorRegistry, NarFileRequest};
use crate::application::nar_info::actor::{NarInfoActorRegistry, NarInfoRequest};
use crate::application::substituter::actor::{SubstituterActorRegistry, SubstituterRequest};
use crate::application::{AppErrorKind, AppOptionExt, AppResult, AppResultExt};
use crate::domain::nar_file::model::{NarFileKey, NarFileLocation};
use crate::domain::nar_info::model::{NarInfoData, StorePathHash};
use crate::domain::nar_info::service::{ResolveNarInfoError, ResolveNarInfoEvent};

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

    pub async fn get_nar_info(&self, hash: StorePathHash) -> AppResult<NarInfoData> {
        tracing::info!(hash = %hash.value(), "resolving nar info");

        let address = self.nar_info_registry.get(&hash).await;

        let response = address
            .ask(|reply_to| NarInfoRequest::ResolveNarInfo(reply_to))
            .await
            .map_err(|_| anyhow::anyhow!("nar actor terminated unexpectedly"))
            .wrap(AppErrorKind::Unknown)?;

        match &response.result {
            Ok(Some(data)) => {
                tracing::info!(hash = %hash.value(), nar_file = %data.nar_file().value(), "resolved nar info");
            }
            Ok(None) => {
                tracing::info!(hash = %hash.value(), "resolved nar info with not-found")
            }
            Err(ResolveNarInfoError::Fetch) => {
                tracing::warn!(hash = %hash.value(), "failed to resolve nar info")
            }
        }

        self.exec_events(response.events).await;
        response.result?.flat()
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
            } => {
                let nar_file_key = NarFileKey::from_file_name(&nar_file);
                let location = NarFileLocation::new(source_url, substituter.nar_timeout());
                let sender = self.nar_file_registry.get(&nar_file_key).await;
                let _ = sender.tell(NarFileRequest::SetLocation(location)).await;
            }
        }
    }
}
