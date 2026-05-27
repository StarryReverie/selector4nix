use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::substituter::model::{Substituter, Url};

pub use crate::domain::substituter::index::SubstituterCandidate;

#[async_trait]
pub trait SubstituterRepository: Send + Sync {
    async fn get(&self, url: &Url) -> Option<Substituter>;

    async fn query_all_available(&self) -> Arc<Vec<SubstituterCandidate>>;

    async fn save(&self, substituter: Substituter);
}
