use std::time::Duration;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;

use crate::domain::nar::model::NarInfoData;
use crate::domain::substituter::model::Url;

#[async_trait]
pub trait NarInfoProvider: Send + Sync {
    async fn query_nar_info(
        &self,
        url: &Url,
        timeout: Option<Duration>,
    ) -> AnyhowResult<Option<NarInfoQueryData>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NarInfoQueryData {
    pub original_data: NarInfoData,
    pub latency: Duration,
}

impl NarInfoQueryData {
    pub fn new(original_data: NarInfoData, latency: Duration) -> Self {
        Self {
            original_data,
            latency,
        }
    }
}
