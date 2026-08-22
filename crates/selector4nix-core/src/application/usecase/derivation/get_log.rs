use std::sync::Arc;

use crate::application::actor::substituter::{SubstituterActorRegistry, SubstituterRequest};
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::derivation::model::DerivingPath;
use crate::domain::derivation::port::{
    DerivationLogProvider, GetDerivationLogAttempt, GetDerivationLogData,
};
use crate::domain::substituter::SubstituterRepository;
use crate::{AppError, AppResultExt};

pub struct GetDerivationLogUseCase {
    substituter_registry: Arc<SubstituterActorRegistry>,
    substituter_repository: Arc<dyn SubstituterRepository>,
    derivation_log_provider: Arc<dyn DerivationLogProvider>,
}

impl GetDerivationLogUseCase {
    pub fn new(
        substituter_registry: Arc<SubstituterActorRegistry>,
        substituter_repository: Arc<dyn SubstituterRepository>,
        derivation_log_provider: Arc<dyn DerivationLogProvider>,
    ) -> Self {
        Self {
            substituter_registry,
            substituter_repository,
            derivation_log_provider,
        }
    }

    pub async fn run(
        &self,
        deriving_path: DerivingPath,
        headers: PassthroughHeaders,
    ) -> Result<GetDerivationLogData, AppError> {
        tracing::info!(%deriving_path, "getting build log of derivation");

        let substituter = self.substituter_repository.query_all_available().await;
        let substituters = substituter
            .iter()
            .map(|s| s.meta().clone())
            .collect::<Vec<_>>();

        let (response, attempts) = self
            .derivation_log_provider
            .get_derivation_log(&substituters, &deriving_path, &headers)
            .await;

        self.publish_substituter_status(attempts).await;

        match response {
            Ok(Some(data)) => {
                tracing::info!(%deriving_path, "got build log of derivation");
                Ok(data)
            }
            Ok(None) => {
                tracing::info!(%deriving_path, "tried to got non-existent build log of derivation");
                Err(AppError::not_found(
                    "could not get non-existent build log of derivation",
                ))
            }
            Err(err) => {
                tracing::warn!(%deriving_path, %err, "failed to get build log of derivation");
                Err(err).chain_infrastructure("could not get build log of derivation")
            }
        }
    }

    async fn publish_substituter_status(&self, attempts: Vec<GetDerivationLogAttempt>) {
        for attempt in attempts {
            let sender = self
                .substituter_registry
                .get(attempt.substituter_url())
                .await;
            match attempt {
                GetDerivationLogAttempt::Successful { .. } => {
                    let _ = sender.tell(SubstituterRequest::ServiceSuccessful).await;
                }
                GetDerivationLogAttempt::Offline { .. } => {
                    let _ = sender.tell(SubstituterRequest::ServiceOffline).await;
                }
                GetDerivationLogAttempt::ServiceError { .. } => {
                    let _ = sender.tell(SubstituterRequest::ServiceError).await;
                }
            }
        }
    }
}
