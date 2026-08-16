use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use bytes::Bytes;

use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::common::url::Url;
use crate::domain::nar_info::model::StorePathHash;
use crate::domain::substituter::model::SubstituterMeta;

#[async_trait]
pub trait NarDirectoryProvider: Send + Sync {
    async fn list(
        &self,
        substituters: &[SubstituterMeta],
        store_path_hash: &StorePathHash,
        headers: &PassthroughHeaders,
    ) -> (
        AnyhowResult<Option<ListDirectoryData>>,
        Vec<ListDirectoryAttempt>,
    );
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListDirectoryData {
    pub content: Bytes,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ListDirectoryAttempt {
    Successful { substituter_url: Url },
    Offline { substituter_url: Url },
    ServiceError { substituter_url: Url },
}

impl ListDirectoryAttempt {
    pub fn substituter_url(&self) -> &Url {
        match self {
            Self::Successful { substituter_url } => substituter_url,
            Self::Offline { substituter_url } => substituter_url,
            Self::ServiceError { substituter_url } => substituter_url,
        }
    }
}
