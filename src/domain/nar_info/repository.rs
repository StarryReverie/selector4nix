use anyhow::Result as AnyhowResult;
use async_trait::async_trait;

use crate::domain::nar_info::model::{NarInfo, StorePathHash};

#[async_trait]
pub trait NarInfoRepository: Send + Sync {
    async fn get(&self, hash: &StorePathHash) -> AnyhowResult<Option<NarInfo>>;

    async fn save(&self, nar_info: NarInfo) -> AnyhowResult<()>;

    async fn entry_count(&self) -> AnyhowResult<usize>;
}
