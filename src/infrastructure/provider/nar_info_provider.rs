use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use snafu::ResultExt;
use tokio::sync::Semaphore;

use crate::domain::nar::model::NarInfoData;
use crate::domain::nar::port::error_ctx::{OfflineSnafu, ServiceSnafu};
use crate::domain::nar::port::{NarInfoProvider, NarInfoQueryData, QueryNarInfoError};
use crate::domain::substituter::model::Url;

pub struct ReqwestNarInfoProvider {
    client: Client,
    default_timeout: Duration,
    concurrency: Arc<Semaphore>,
}

impl ReqwestNarInfoProvider {
    pub fn new(client: Client, default_timeout: Duration, concurrency: Arc<Semaphore>) -> Self {
        Self {
            client,
            default_timeout,
            concurrency,
        }
    }
}

#[async_trait]
impl NarInfoProvider for ReqwestNarInfoProvider {
    async fn query_nar_info(
        &self,
        url: &Url,
        timeout: Option<Duration>,
    ) -> Result<Option<NarInfoQueryData>, QueryNarInfoError> {
        tracing::debug!(%url, "fetching nar info from substituter");

        let _permit = self.concurrency.acquire().await.unwrap();

        let timeout = timeout.unwrap_or(self.default_timeout);
        let request = self.client.get(url.value()).timeout(timeout);

        let start = Instant::now();
        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                tracing::debug!(%url, is_timeout = %err.is_timeout(), "failed to send nar info query request");
                if err.is_timeout() || err.is_connect() || err.is_request() {
                    return Err(AnyhowError::new(err)).context(OfflineSnafu);
                } else {
                    return Err(AnyhowError::new(err)).context(ServiceSnafu);
                }
            }
        };

        match response.status() {
            StatusCode::OK => {
                let text = (response.text().await)
                    .map_err(|err| AnyhowError::new(err))
                    .map_err(|err| err.context(format!("failed to read nar info body from {url}")))
                    .context(ServiceSnafu)
                    .inspect_err(|_| tracing::debug!(%url, "failed to read nar info body"))?;
                let latency = start.elapsed();
                let original_data = NarInfoData::original(text)
                    .map_err(|err| AnyhowError::new(err))
                    .map_err(|err| err.context(format!("invalid nar info from {url}")))
                    .context(ServiceSnafu)
                    .inspect_err(|_| tracing::debug!(%url, "failed to parse nar info body"))?;
                tracing::debug!(%url, "fetched nar info from substituter");
                Ok(Some(NarInfoQueryData::new(original_data, latency)))
            }
            StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => Ok(None),
            status => Err(anyhow::anyhow!("unexpected status {} from {}", status, url))
                .context(ServiceSnafu)
                .inspect_err(|_| tracing::debug!(%url, "encountered bad nar info response status")),
        }
    }
}
