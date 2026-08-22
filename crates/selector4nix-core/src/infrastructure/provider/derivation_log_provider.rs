use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use http::{StatusCode, header};
use reqwest::{Client, RequestBuilder};
use tokio::task::JoinSet;

use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::common::url::Url;
use crate::domain::derivation::model::DerivingPath;
use crate::domain::derivation::port::{
    DerivationLogProvider, GetDerivationLogAttempt, GetDerivationLogData,
};
use crate::domain::substituter::model::SubstituterMeta;
use crate::infrastructure::config::AppCredential;

pub struct ReqwestDerivationLogProvider {
    client: Client,
    credentials: Arc<AppCredential>,
}

impl ReqwestDerivationLogProvider {
    pub fn new(client: Client, credentials: Arc<AppCredential>) -> Self {
        Self {
            client,
            credentials,
        }
    }
}

#[async_trait]
impl DerivationLogProvider for ReqwestDerivationLogProvider {
    async fn get_derivation_log(
        &self,
        substituters: &[SubstituterMeta],
        deriving_path: &DerivingPath,
        headers: &PassthroughHeaders,
    ) -> (
        AnyhowResult<Option<GetDerivationLogData>>,
        Vec<GetDerivationLogAttempt>,
    ) {
        tracing::debug!(substituter_urls = ?substituters.iter().map(|s| s.url().to_string()).collect::<Vec<_>>(), %deriving_path, "getting build log of derivation from upstream substituters");

        let mut pending = JoinSet::new();
        for substituter in substituters {
            let url = deriving_path.on_substituter_log(substituter);

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
                tracing::debug!(substituter_url = %attempt.substituter_url(), %deriving_path, "fetched build log of derivation from upstream substituter");
                return (Ok(Some(data)), attempts);
            }
        }

        if !has_error {
            tracing::debug!(%deriving_path, "tried getting non-existent build log of derivation from upstream substituters");
            (Ok(None), attempts)
        } else {
            tracing::debug!(%deriving_path, "failed to get build log of derivation from upstream substituters");
            let err = Err(anyhow::anyhow!(
                "could not send get build log request for derivation {deriving_path}",
            ));
            (err, attempts)
        }
    }
}

async fn get_response(
    request: RequestBuilder,
    substituter_url: Url,
) -> (
    Result<Option<GetDerivationLogData>, ()>,
    GetDerivationLogAttempt,
) {
    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => {
            if err.is_timeout() || err.is_connect() || err.is_request() {
                let attempt = GetDerivationLogAttempt::Offline { substituter_url };
                return (Ok(None), attempt);
            } else {
                let attempt = GetDerivationLogAttempt::ServiceError { substituter_url };
                return (Err(()), attempt);
            };
        }
    };

    match response.status() {
        StatusCode::OK => {
            let content_encoding = response
                .headers()
                .get(header::CONTENT_ENCODING)
                .and_then(|h| h.to_str().ok().map(ToOwned::to_owned));

            let Ok(content) = response.bytes().await else {
                let attempt = GetDerivationLogAttempt::ServiceError { substituter_url };
                return (Err(()), attempt);
            };

            let data = GetDerivationLogData {
                content,
                content_encoding,
            };
            let attempt = GetDerivationLogAttempt::Successful { substituter_url };
            (Ok(Some(data)), attempt)
        }
        StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => {
            let attempt = GetDerivationLogAttempt::Successful { substituter_url };
            (Ok(None), attempt)
        }
        _ => {
            let attempt = GetDerivationLogAttempt::ServiceError { substituter_url };
            (Err(()), attempt)
        }
    }
}
