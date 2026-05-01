use std::sync::Arc;

use anyhow::{Context, Result as AnyhowResult};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Client, Response, StatusCode};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::domain::nar::port::{NarStream, NarStreamHeaders, NarStreamOutcome, NarStreamProvider};
use crate::domain::substituter::model::Url;

pub struct ReqwestNarStreamProvider {
    client: Client,
    concurrency: Arc<Semaphore>,
}

impl ReqwestNarStreamProvider {
    pub fn new(client: Client, concurrency: Arc<Semaphore>) -> Self {
        Self {
            client,
            concurrency,
        }
    }

    fn wrap_ok_response(url: Url, response: Response) -> AnyhowResult<NarStreamOutcome> {
        let headers = NarStreamHeaders {
            content_length: response.content_length(),
            content_type: response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string),
            content_encoding: response
                .headers()
                .get(reqwest::header::CONTENT_ENCODING)
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string),
        };

        let stream = response
            .bytes_stream()
            .map(|chunk| chunk.with_context(|| "failed to read nar stream"));
        Ok(NarStreamOutcome::Found {
            stream: NarStream::new(headers, Box::pin(stream)),
            source_url: url,
        })
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
            let url = url.clone();
            let concurrency = self.concurrency.clone();
            set.spawn(async move {
                let _permit = concurrency.acquire().await.unwrap();
                let response = client.get(url.value()).send().await;
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
                        return Self::wrap_ok_response(url, resp);
                    }
                    StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => {
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
