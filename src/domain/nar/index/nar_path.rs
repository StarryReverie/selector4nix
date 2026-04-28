use async_trait::async_trait;

use crate::domain::substituter::model::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NarPathEvent {
    Registered {
        nar_path: String,
        storage_prefix: Url,
    },
    Evicted {
        nar_path: String,
    },
}

#[async_trait]
pub trait NarPathIndex: Send + Sync {
    async fn get_storage_prefix(&self, nar_path: &str) -> Option<Url>;
}
