use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use bytes::Bytes;

use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::common::url::Url;
use crate::domain::derivation::model::DerivingPath;
use crate::domain::substituter::model::SubstituterMeta;

#[async_trait]
pub trait DerivationLogProvider: Send + Sync {
    async fn get_derivation_log(
        &self,
        substituters: &[SubstituterMeta],
        deriving_path: &DerivingPath,
        headers: &PassthroughHeaders,
    ) -> (
        AnyhowResult<Option<GetDerivationLogData>>,
        Vec<GetDerivationLogAttempt>,
    );
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GetDerivationLogData {
    pub content: Bytes,
    pub content_encoding: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GetDerivationLogAttempt {
    Successful { substituter_url: Url },
    Offline { substituter_url: Url },
    ServiceError { substituter_url: Url },
}

impl GetDerivationLogAttempt {
    pub fn substituter_url(&self) -> &Url {
        match self {
            Self::Successful { substituter_url } => substituter_url,
            Self::Offline { substituter_url } => substituter_url,
            Self::ServiceError { substituter_url } => substituter_url,
        }
    }
}
