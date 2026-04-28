use std::sync::Arc;

use crate::domain::substituter::model::{Substituter, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubstituterAvailabilityEvent {
    BecameAvailable(Substituter),
    BecameUnavailable(Url),
}

pub trait SubstituterAvailabilityIndex: Send + Sync {
    fn query_all(&self) -> Arc<Vec<Substituter>>;
}
