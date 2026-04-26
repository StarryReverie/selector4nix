use std::sync::Arc;

use tokio::sync::watch::Receiver;

use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Clone)]
pub struct SubstituterAvailabilityIndexView {
    snapshot: Receiver<Arc<Vec<SubstituterMeta>>>,
}

impl SubstituterAvailabilityIndexView {
    pub fn new(snapshot: Receiver<Arc<Vec<SubstituterMeta>>>) -> Self {
        Self { snapshot }
    }
}

impl SubstituterAvailabilityIndex for SubstituterAvailabilityIndexView {
    fn query_all(&self) -> Arc<Vec<SubstituterMeta>> {
        Arc::clone(&self.snapshot.borrow())
    }
}
