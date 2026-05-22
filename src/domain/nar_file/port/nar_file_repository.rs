use async_trait::async_trait;

use crate::domain::nar_file::model::{NarFile, NarFileKey};

#[async_trait]
pub trait NarFileRepository: Send + Sync {
    async fn get(&self, key: &NarFileKey) -> Option<NarFile>;

    async fn put(&self, nar_file: &NarFile);
}
