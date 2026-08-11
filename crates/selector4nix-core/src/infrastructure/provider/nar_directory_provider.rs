use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use http::{StatusCode, header};
use reqwest::Client;
use tokio::task::JoinSet;

use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_info::model::StorePathHash;
use crate::domain::nar_info::port::{
    ListDirectoryAttempt, ListDirectoryData, NarDirectoryProvider,
};
use crate::domain::substituter::model::SubstituterMeta;
use crate::infrastructure::config::AppCredential;

pub struct ReqwestNarDirectoryProvider {
    client: Client,
    credentials: Arc<AppCredential>,
}

impl ReqwestNarDirectoryProvider {
    pub fn new(client: Client, credentials: Arc<AppCredential>) -> Self {
        Self {
            client,
            credentials,
        }
    }
}

#[async_trait]
impl NarDirectoryProvider for ReqwestNarDirectoryProvider {
    async fn list(
        &self,
        substituters: &[SubstituterMeta],
        store_path_hash: &StorePathHash,
        headers: &PassthroughHeaders,
    ) -> (
        AnyhowResult<Option<ListDirectoryData>>,
        Vec<ListDirectoryAttempt>,
    ) {
        tracing::debug!(substituter_urls = ?substituters.iter().map(|s| s.url().to_string()).collect::<Vec<_>>(), hash = ?store_path_hash, "listing directory of store path from substituters");

        let mut set = JoinSet::new();
        for substituter in substituters {
            let url = store_path_hash.on_substituter_listing(substituter);

            let request = self.client.get(url.value()).headers(headers.to_headers());
            let request = if let Some(credential) = self.credentials.lookup(&url) {
                request.basic_auth(credential.login.clone(), credential.secret.clone())
            } else {
                request
            };

            let substituter_url = substituter.url().clone();
            set.spawn(async move {
                let res = request.send().await;
                (res, substituter_url)
            });
        }

        let mut not_found_count = 0;
        let mut attempts = Vec::new();

        while let Some(res) = set.join_next().await {
            let Ok((res, substituter_url)) = res else {
                continue;
            };

            let response = match res {
                Ok(response) => response,
                Err(err) => {
                    if err.is_timeout() || err.is_connect() || err.is_request() {
                        attempts.push(ListDirectoryAttempt::Offline { substituter_url });
                    } else {
                        attempts.push(ListDirectoryAttempt::ServiceError { substituter_url });
                    }
                    continue;
                }
            };

            match response.status() {
                StatusCode::OK => {
                    let content_type = response
                        .headers()
                        .get(header::CONTENT_TYPE)
                        .and_then(|h| h.to_str().ok().map(ToOwned::to_owned));
                    let content_encoding = response
                        .headers()
                        .get(header::CONTENT_ENCODING)
                        .and_then(|h| h.to_str().ok().map(ToOwned::to_owned));

                    let Ok(content) = response.bytes().await else {
                        attempts.push(ListDirectoryAttempt::ServiceError { substituter_url });
                        continue;
                    };

                    tracing::debug!(%substituter_url, hash = %store_path_hash.value(), "fetched entry list in directory of store path");

                    attempts.push(ListDirectoryAttempt::Successful { substituter_url });
                    let data = ListDirectoryData {
                        content,
                        content_type,
                        content_encoding,
                    };
                    return (Ok(Some(data)), attempts);
                }
                StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => {
                    attempts.push(ListDirectoryAttempt::Successful { substituter_url });
                    not_found_count += 1;
                }
                _ => {
                    attempts.push(ListDirectoryAttempt::ServiceError { substituter_url });
                }
            }
        }

        if not_found_count == 0 {
            tracing::debug!(hash = %store_path_hash.value(), "tried listing non-existent directory of store path");
            (Ok(None), attempts)
        } else {
            tracing::debug!(hash = %store_path_hash.value(), "failed to list directory of store path");
            let err = Err(anyhow::anyhow!(
                "could not send list directory request for store path hash {}",
                store_path_hash.value()
            ));
            (err, attempts)
        }
    }
}
