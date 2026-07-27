use std::sync::Arc;

use crate::domain::substituter::SubstituterRepository;
use crate::domain::substituter::model::SubstituterMeta;

pub struct SubstituterQueryUseCase {
    substituter_repository: Arc<dyn SubstituterRepository>,
}

impl SubstituterQueryUseCase {
    pub fn new(substituter_repository: Arc<dyn SubstituterRepository>) -> Self {
        Self {
            substituter_repository,
        }
    }

    pub async fn get_available(&self) -> Vec<SubstituterMeta> {
        let result = self.substituter_repository.query_all_available().await;
        tracing::info!(count = result.len(), "queried available substituters");
        result.iter().map(|s| s.meta().clone()).collect()
    }
}
