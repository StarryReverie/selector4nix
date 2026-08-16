use std::sync::Arc;

use crate::application::actor::substituter::{SubstituterActorRegistry, SubstituterRequest};
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_info::model::StorePathHash;
use crate::domain::nar_info::port::{
    ListDirectoryAttempt, ListDirectoryData, NarDirectoryProvider,
};
use crate::domain::substituter::SubstituterRepository;
use crate::{AppError, AppResultExt};

pub struct ListNarInnerDirectoryUseCase {
    substituter_registry: Arc<SubstituterActorRegistry>,
    substituter_repository: Arc<dyn SubstituterRepository>,
    nar_directory_provider: Arc<dyn NarDirectoryProvider>,
}

impl ListNarInnerDirectoryUseCase {
    pub fn new(
        substituter_registry: Arc<SubstituterActorRegistry>,
        substituter_repository: Arc<dyn SubstituterRepository>,
        nar_directory_provider: Arc<dyn NarDirectoryProvider>,
    ) -> Self {
        Self {
            substituter_registry,
            substituter_repository,
            nar_directory_provider,
        }
    }

    pub async fn run(
        &self,
        hash: StorePathHash,
        headers: PassthroughHeaders,
    ) -> Result<ListDirectoryData, AppError> {
        tracing::info!(hash = %hash.value(), "listing inner directory in nar");

        let substituter = self.substituter_repository.query_all_available().await;
        let substituters = substituter
            .iter()
            .map(|s| s.meta().clone())
            .collect::<Vec<_>>();

        let (response, attempts) = self
            .nar_directory_provider
            .list(&substituters, &hash, &headers)
            .await;

        self.publish_substituter_status(attempts).await;

        match response {
            Ok(Some(data)) => {
                tracing::info!(hash = %hash.value(), "listed inner directory in nar");
                Ok(data)
            }
            Ok(None) => {
                tracing::info!(hash = %hash.value(), "tried to list inner directory in non-existent nar");
                Err(AppError::not_found(
                    "could not list directory in non-existent nar",
                ))
            }
            Err(err) => {
                tracing::warn!(hash = %hash.value(), %err, "failed to list inner directory in nar");
                Err(err).chain_infrastructure("could not list inner directory in nar")
            }
        }
    }

    async fn publish_substituter_status(&self, attempts: Vec<ListDirectoryAttempt>) {
        for attempt in attempts {
            let sender = self
                .substituter_registry
                .get(attempt.substituter_url())
                .await;
            match attempt {
                ListDirectoryAttempt::Successful { .. } => {
                    let _ = sender.tell(SubstituterRequest::ServiceSuccessful).await;
                }
                ListDirectoryAttempt::Offline { .. } => {
                    let _ = sender.tell(SubstituterRequest::ServiceOffline).await;
                }
                ListDirectoryAttempt::ServiceError { .. } => {
                    let _ = sender.tell(SubstituterRequest::ServiceError).await;
                }
            }
        }
    }
}
