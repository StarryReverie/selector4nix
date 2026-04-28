use async_trait::async_trait;

use crate::domain::substituter::model::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NarFileEvent {
    Registered {
        nar_file: String,
        storage_prefix: Url,
    },
    Evicted {
        nar_file: String,
    },
}

#[async_trait]
pub trait NarFileIndex: Send + Sync {
    async fn get_storage_prefix(&self, nar_file: &str) -> Option<Url>;
}
