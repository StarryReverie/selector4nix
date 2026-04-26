use std::sync::Arc;

use tokio::sync::mpsc::Sender;

use crate::domain::substituter::model::{SubstituterMeta, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubstituterAvailabilityEvent {
    BecameAvailable(SubstituterMeta),
    BecameUnavailable(Url),
}

pub trait SubstituterAvailabilityIndex: Send + Sync {
    fn publisher(&self) -> Sender<SubstituterAvailabilityEvent>;

    fn query_all(&self) -> Arc<Vec<SubstituterMeta>>;
}
