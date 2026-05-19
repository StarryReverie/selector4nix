use std::sync::Arc;

use crate::application::nar::actor::{NarActorRegistry, NarRequest};
use crate::application::substituter::actor::{SubstituterActorRegistry, SubstituterRequest};
use crate::application::{AppErrorKind, AppOptionExt, AppResult, AppResultExt};
use crate::domain::nar::model::{NarInfoData, StorePathHash};
use crate::domain::nar::service::{NarResolutionEvent, ResolveNarInfoError};

pub struct NarResolutionUseCase {
    nar_registry: Arc<NarActorRegistry>,
    substituter_registry: Arc<SubstituterActorRegistry>,
}

impl NarResolutionUseCase {
    pub fn new(
        nar_registry: Arc<NarActorRegistry>,
        substituter_registry: Arc<SubstituterActorRegistry>,
    ) -> Self {
        Self {
            nar_registry,
            substituter_registry,
        }
    }

    pub async fn get_nar_info(&self, hash: StorePathHash) -> AppResult<NarInfoData> {
        tracing::info!(hash = %hash.value(), "resolving nar info");

        let address = self.nar_registry.get(&hash).await;

        let response = address
            .ask(|reply_to| NarRequest::ResolveNarInfo(reply_to))
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

    async fn exec_events(&self, events: Vec<NarResolutionEvent>) {
        for event in events {
            self.exec_event(event).await;
        }
    }

    async fn exec_event(&self, event: NarResolutionEvent) {
        match event {
            NarResolutionEvent::SubstituterSucceeded(url) => {
                let sender = self.substituter_registry.get(&url).await;
                let _ = sender.tell(SubstituterRequest::ServiceSuccessful).await;
            }
            NarResolutionEvent::SubstituterFailed(url) => {
                let sender = self.substituter_registry.get(&url).await;
                let _ = sender.tell(SubstituterRequest::ServiceError).await;
            }
        }
    }
}
