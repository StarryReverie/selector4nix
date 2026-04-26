use std::time::{Duration, Instant};

use anyhow::{Context, Result as AnyhowResult};
use async_trait::async_trait;
use reqwest::Client;
use reqwest::StatusCode;

use crate::domain::nar::model::{NarInfoData, NarInfoQueryOutcome};
use crate::domain::nar::port::NarInfoProvider;
use crate::domain::substituter::model::Url;

// TODO: make timeout configurable
const NARINFO_TIMEOUT: Duration = Duration::from_secs(5);

pub struct ReqwestNarInfoProvider {
    client: Client,
}

impl ReqwestNarInfoProvider {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl NarInfoProvider for ReqwestNarInfoProvider {
    async fn provide_nar_info(&self, url: &Url) -> AnyhowResult<NarInfoQueryOutcome> {
        let request = self.client.get(url.value()).timeout(NARINFO_TIMEOUT);

        let start = Instant::now();
        let response = (request.send().await)
            .with_context(|| format!("failed to fetch narinfo from {}", url))?;

        match response.status() {
            StatusCode::OK => {
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
