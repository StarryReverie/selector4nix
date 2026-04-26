use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::domain::substituter::model::SubstituterMeta;

pub struct SubstituterUseCase {
    substituter_availability_index: Box<dyn SubstituterAvailabilityIndex>,
}

impl SubstituterUseCase {
    pub fn new(substituter_availability_index: Box<dyn SubstituterAvailabilityIndex>) -> Self {
        Self {
            substituter_availability_index,
        }
    }

    pub fn get_available(&self) -> Vec<SubstituterMeta> {
        self.substituter_availability_index.query_all().to_vec()
    }
}
