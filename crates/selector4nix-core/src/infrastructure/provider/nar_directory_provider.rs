use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use http::{StatusCode, header};
use reqwest::{Client, RequestBuilder};
use tokio::task::JoinSet;

use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::common::url::Url;
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
        tracing::debug!(substituter_urls = ?substituters.iter().map(|s| s.url().to_string()).collect::<Vec<_>>(), hash = %store_path_hash.value(), "listing directory of store path from substituters");

        let mut pending = JoinSet::new();
        for substituter in substituters {
            let url = store_path_hash.on_substituter_listing(substituter);

            let request = self.client.get(url.value()).headers(headers.to_headers());
            let request = if let Some(credential) = self.credentials.lookup(&url) {
                request.basic_auth(credential.login.clone(), credential.secret.clone())
            } else {
                request
            };

            let substituter_url = substituter.url().clone();
            pending.spawn(get_response(request, substituter_url));
        }

        let mut has_error = false;
        let mut attempts = Vec::new();
        while let Some(res) = pending.join_next().await {
            let Ok((res, attempt)) = res else {
                continue;
            };

            has_error |= res.is_err();
            let attempt = attempts.push_mut(attempt);
            if let Ok(Some(data)) = res {
                tracing::debug!(substituter_url = %attempt.substituter_url(), hash = %store_path_hash.value(), "fetched entry list in directory of store path");
                return (Ok(Some(data)), attempts);
            }
        }

        if !has_error {
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

async fn get_response(
    request: RequestBuilder,
    substituter_url: Url,
) -> (Result<Option<ListDirectoryData>, ()>, ListDirectoryAttempt) {
    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => {
            if err.is_timeout() || err.is_connect() || err.is_request() {
                let attempt = ListDirectoryAttempt::Offline { substituter_url };
                return (Ok(None), attempt);
            } else {
                let attempt = ListDirectoryAttempt::ServiceError { substituter_url };
                return (Err(()), attempt);
            };
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
                let attempt = ListDirectoryAttempt::ServiceError { substituter_url };
                return (Err(()), attempt);
            };

            let data = ListDirectoryData {
                content,
                content_type,
                content_encoding,
            };
            let attempt = ListDirectoryAttempt::Successful { substituter_url };
            (Ok(Some(data)), attempt)
        }
        StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => {
            let attempt = ListDirectoryAttempt::Successful { substituter_url };
            (Ok(None), attempt)
        }
        _ => {
            let attempt = ListDirectoryAttempt::ServiceError { substituter_url };
            (Err(()), attempt)
        }
    }
}
