use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::common::url::Url;
use selector4nix_core::domain::nar_info::port::error_ctx::{OfflineSnafu, ServiceSnafu};
use selector4nix_core::domain::nar_info::port::{
    NarInfoProvider, NarInfoQueryData, QueryNarInfoError,
};
use snafu::ResultExt;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct MockNarInfoProvider {
    queries: HashMap<Url, Result<NarInfoQueryData, String>>,
    queried_urls: Mutex<Vec<Url>>,
}

impl MockNarInfoProvider {
    pub fn new<I>(queries: I) -> Self
    where
        I: IntoIterator<Item = (Url, Result<NarInfoQueryData, String>)>,
    {
        Self {
            queries: queries.into_iter().collect(),
            queried_urls: Mutex::new(Vec::new()),
        }
    }

    pub async fn queried_urls(&self) -> Vec<Url> {
        self.queried_urls.lock().await.clone()
    }
}

#[async_trait]
impl NarInfoProvider for MockNarInfoProvider {
    async fn query_nar_info(
        &self,
        url: &Url,
        _headers: &PassthroughHeaders,
        timeout: Option<Duration>,
    ) -> Result<Option<NarInfoQueryData>, QueryNarInfoError> {
        self.queried_urls.lock().await.push(url.clone());

        let Some(data) = self.queries.get(url) else {
            return Ok(None);
        };

        match (data, timeout) {
            (Ok(data), Some(timeout)) if data.latency > timeout => {
                tokio::time::sleep(timeout).await;
                Err(anyhow::anyhow!("timeout")).context(OfflineSnafu)
            }
            (Ok(data), _) => {
                tokio::time::sleep(data.latency).await;
                Ok(Some(data.clone()))
            }
            (Err(err), _) => Err(anyhow::anyhow!("{err}")).context(ServiceSnafu),
        }
    }
}
