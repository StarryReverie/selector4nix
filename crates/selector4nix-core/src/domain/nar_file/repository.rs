use anyhow::Result as AnyhowResult;
use async_trait::async_trait;

use crate::domain::nar_file::model::{NarFile, NarFileKey};

#[async_trait]
pub trait NarFileRepository: Send + Sync {
    async fn get(&self, key: &NarFileKey) -> AnyhowResult<Option<NarFile>>;

    async fn save(&self, nar_file: NarFile) -> AnyhowResult<()>;

    async fn entry_count(&self) -> AnyhowResult<usize>;
}
