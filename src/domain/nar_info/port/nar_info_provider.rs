use std::time::Duration;

use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use snafu::Snafu;

use crate::domain::common::url::Url;
use crate::domain::nar_info::model::UpstreamNarInfoData;

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
    pub upstream_data: UpstreamNarInfoData,
    pub latency: Duration,
}

impl NarInfoQueryData {
    pub fn new(original_data: UpstreamNarInfoData, latency: Duration) -> Self {
        Self {
            upstream_data: original_data,
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
