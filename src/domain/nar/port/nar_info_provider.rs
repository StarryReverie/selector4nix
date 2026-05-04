use std::time::Duration;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;

use crate::domain::nar::model::NarInfoData;
use crate::domain::substituter::model::Url;

#[async_trait]
pub trait NarInfoProvider: Send + Sync {
    async fn provide_nar_info(&self, url: &Url) -> AnyhowResult<NarInfoQueryOutcome>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NarInfoQueryOutcome {
    Found {
        original_data: NarInfoData,
        latency: Duration,
    },
    NotFound,
}

impl NarInfoQueryOutcome {
    pub fn unwrap_found(self) -> Option<(NarInfoData, Duration)> {
        match self {
            Self::Found {
                original_data,
                latency,
            } => Some((original_data, latency)),
            Self::NotFound => None,
        }
    }
}
