use std::time::Duration;

use anyhow::{Context, Result as AnyhowResult};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Client, StatusCode};
use tokio::task::JoinSet;

use crate::domain::nar::port::{NarStream, NarStreamOutcome, NarStreamProvider};
use crate::domain::substituter::model::Url;

pub struct ReqwestNarStreamProvider {
    client: Client,
    timeout: Duration,
}

impl ReqwestNarStreamProvider {
    pub fn new(client: Client, timeout: Duration) -> Self {
        Self { client, timeout }
    }
}

#[async_trait]
impl NarStreamProvider for ReqwestNarStreamProvider {
    async fn stream_nar(&self, urls: &[Url]) -> AnyhowResult<NarStreamOutcome> {
        if urls.is_empty() {
            return Ok(NarStreamOutcome::NotFound);
        }

        let mut set = JoinSet::new();
        for url in urls {
            let client = self.client.clone();
            let timeout = self.timeout;
            let url = url.clone();
            set.spawn(async move {
                let request = client.get(url.value()).timeout(timeout);
                let response = request.send().await;
                (url, response)
            });
        }

        let mut not_found_count = 0;

        while let Some(result) = set.join_next().await {
            let Ok((url, response)) = result else {
                continue;
            };

            match response {
                Ok(resp) => match resp.status() {
                    StatusCode::OK => {
                        let stream = resp
                            .bytes_stream()
                            .map(|chunk| chunk.with_context(|| "failed to read nar stream"));
                        return Ok(NarStreamOutcome::Found {
                            stream: NarStream {
                                inner: Box::pin(stream),
                            },
                            source_url: url,
                        });
                    }
                    StatusCode::NOT_FOUND => {
                        not_found_count += 1;
                    }
                    status => {
                        tracing::debug!(%url, %status, "received unexpected status from substituter");
                    }
                },
                Err(e) => {
                    tracing::debug!(%url, error = %e, "failed to request nar from substituter");
                }
            }
        }

        if not_found_count == urls.len() {
            Ok(NarStreamOutcome::NotFound)
        } else {
            Err(anyhow::anyhow!("could not fetch nar from any substituter"))
        }
    }
}
