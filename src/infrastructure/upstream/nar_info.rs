use std::time::{Duration, Instant};

use anyhow::{Context, Result as AnyhowResult};
use async_trait::async_trait;
use reqwest::Client;
use reqwest::StatusCode;

use crate::domain::nar::model::{NarInfoData, NarInfoQueryOutcome};
use crate::domain::nar::port::NarInfoProvider;
use crate::domain::substituter::model::Url;

pub struct ReqwestNarInfoProvider {
    client: Client,
    timeout: Duration,
}

impl ReqwestNarInfoProvider {
    pub fn new(client: Client, timeout: Duration) -> Self {
        Self { client, timeout }
    }
}

#[async_trait]
impl NarInfoProvider for ReqwestNarInfoProvider {
    async fn provide_nar_info(&self, url: &Url) -> AnyhowResult<NarInfoQueryOutcome> {
        tracing::debug!(%url, "fetching nar info from substituter");

        let request = self.client.get(url.value()).timeout(self.timeout);

        let start = Instant::now();
        let response = (request.send().await)
            .with_context(|| format!("failed to fetch narinfo from {}", url))?;

        match response.status() {
            StatusCode::OK => {
                tracing::debug!(%url, "fetched nar info from substituter");
                let text = (response.text().await)
                    .with_context(|| format!("failed to read narinfo body from {}", url))?;
                let latency = start.elapsed();
                let data = NarInfoData::new(text)
                    .with_context(|| format!("invalid narinfo from {}", url))?;
                Ok(NarInfoQueryOutcome::Found { data, latency })
            }
            StatusCode::NOT_FOUND => Ok(NarInfoQueryOutcome::NotFound),
            status => Err(anyhow::anyhow!("unexpected status {} from {}", status, url)),
        }
    }
}
