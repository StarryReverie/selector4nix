use std::sync::Arc;

use async_trait::async_trait;
use moka::future::Cache;

use crate::domain::nar::index::NarPathIndex;
use crate::domain::substituter::model::Url;

#[derive(Clone)]
pub struct NarPathIndexView {
    cache: Arc<Cache<String, Url>>,
}

impl NarPathIndexView {
    pub fn new(cache: Arc<Cache<String, Url>>) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl NarPathIndex for NarPathIndexView {
    async fn get_storage_prefix(&self, nar_path: &str) -> Option<Url> {
        self.cache.get(&nar_path.to_string()).await
    }
}
