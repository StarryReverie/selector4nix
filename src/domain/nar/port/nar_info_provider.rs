use anyhow::Result as AnyhowResult;
use async_trait::async_trait;

use crate::domain::nar::model::NarInfoQueryOutcome;
use crate::domain::substituter::model::Url;

#[async_trait]
pub trait NarInfoProvider: Send + Sync {
    async fn provide_nar_info(&self, url: &Url) -> AnyhowResult<NarInfoQueryOutcome>;
}
