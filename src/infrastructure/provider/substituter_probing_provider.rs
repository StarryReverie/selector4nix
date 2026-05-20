use std::sync::Arc;
use std::time::Duration;

use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use snafu::ResultExt;
use tokio::sync::Semaphore;

use crate::domain::substituter::model::SubstituterMeta;
use crate::domain::substituter::port::error_ctx::{OfflineSnafu, ServiceSnafu};
use crate::domain::substituter::port::{ProbeSubstituterError, SubstituterProbingProvider};

pub struct ReqwestSubstituterProbingProvider {
    client: Client,
    default_timeout: Duration,
    concurrency: Arc<Semaphore>,
}

impl ReqwestSubstituterProbingProvider {
    pub fn new(client: Client, default_timeout: Duration, concurrency: Arc<Semaphore>) -> Self {
        Self {
            client,
            default_timeout,
            concurrency,
        }
    }
}

#[async_trait]
impl SubstituterProbingProvider for ReqwestSubstituterProbingProvider {
    async fn probe_substituter(
        &self,
        substituter: &SubstituterMeta,
    ) -> Result<(), ProbeSubstituterError> {
        tracing::debug!(substituter = %substituter.url(), "probing substituter's health status");

        let _permit = self.concurrency.acquire().await.unwrap();

        let url = substituter.url().as_dir().join("nix-cache-info").unwrap();
        let timeout = substituter
            .nar_info_timeout()
            .unwrap_or(self.default_timeout);
        let request = self.client.get(url.value()).timeout(timeout);

        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                tracing::debug!(%url, is_timeout = %err.is_timeout(), "failed to send probing request");
                if err.is_timeout() || err.is_connect() || err.is_request() {
                    return Err(AnyhowError::new(err)).context(OfflineSnafu);
                } else {
                    return Err(AnyhowError::new(err)).context(ServiceSnafu);
                }
            }
        };

        match response.status() {
            StatusCode::OK => {
                let _ = (response.text().await)
                    .map_err(|err| AnyhowError::new(err))
                    .map_err(|err| err.context(format!("failed to read nix-cache-info from {url}")))
                    .context(ServiceSnafu)
                    .inspect_err(|_| tracing::debug!(%url, "failed to read nix-cache-info"))?;
                tracing::debug!(%url, "probed substituter successfully");
                Ok(())
            }
            status => Err(anyhow::anyhow!("unexpected status {} from {}", status, url))
                .context(ServiceSnafu)
                .inspect_err(
                    |_| tracing::debug!(%url, "encountered bad nix-cache-info response status"),
                ),
        }
    }
}
