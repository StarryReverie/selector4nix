use std::sync::Arc;

use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::domain::substituter::model::SubstituterMeta;

pub struct SubstituterQueryUseCase {
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
}

impl SubstituterQueryUseCase {
    pub fn new(substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>) -> Self {
        Self {
            substituter_availability_index,
        }
    }

    pub fn get_available(&self) -> Vec<SubstituterMeta> {
        let result = self.substituter_availability_index.query_all();
        tracing::info!(count = result.len(), "queried available substituters");
        result.iter().map(|s| s.meta().clone()).collect()
    }
}
