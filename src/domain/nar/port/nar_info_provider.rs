use std::time::Duration;

use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use snafu::Snafu;

use crate::domain::nar::model::NarInfoData;
use crate::domain::substituter::model::Url;

#[async_trait]
pub trait NarInfoProvider: Send + Sync {
    async fn query_nar_info(
        &self,
        url: &Url,
        timeout: Option<Duration>,
    ) -> Result<Option<NarInfoQueryData>, QueryNarInfoError>;
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

#[derive(Snafu, Debug)]
#[non_exhaustive]
#[snafu(visibility(pub))]
pub enum QueryNarInfoError {
    #[snafu(display("could not query nar info from offline substituter"))]
    Offline { source: AnyhowError },
    #[snafu(display("query nar info got service error from substituter"))]
    Service { source: AnyhowError },
}

pub mod error_ctx {
    pub use super::{OfflineSnafu, ServiceSnafu};
}
