use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{Context as _, Result as AnyhowResult};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use http::{StatusCode, header};
use reqwest::{Client, Response};
use tokio::task::JoinSet;

use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::common::url::Url;
use crate::domain::nar_file::model::NarFileLocation;
use crate::domain::nar_file::port::{NarStreamData, NarStreamHeaders, NarStreamProvider};
use crate::infrastructure::config::{AppCredential, DownloadConfiguration};
use crate::infrastructure::provider::segmented::start_segmented_download;
use crate::infrastructure::util::{
    DownloadLoadTracker, LoadGuard, PerHostHttpThrottler, ThrottlerPermit,
};

pub struct ReqwestNarStreamProvider {
    client: Client,
    throttler: Arc<PerHostHttpThrottler>,
    credentials: Arc<AppCredential>,
    download_config: DownloadConfiguration,
    load_tracker: DownloadLoadTracker,
}

impl ReqwestNarStreamProvider {
    pub fn new(
        client: Client,
        throttler: Arc<PerHostHttpThrottler>,
        credentials: Arc<AppCredential>,
        download_config: DownloadConfiguration,
        load_tracker: DownloadLoadTracker,
    ) -> Self {
        Self {
            client,
            throttler,
            credentials,
            download_config,
            load_tracker,
        }
    }

    fn wrap_ok_response(
        url: Url,
        response: Response,
        permit: ThrottlerPermit,
        load_guard: LoadGuard,
    ) -> AnyhowResult<Option<NarStreamData>> {
        let headers = extract_stream_headers(&response);
        let stream = ThrottledStream {
            inner: response
                .bytes_stream()
                .map(|chunk| chunk.with_context(|| "failed to read nar stream")),
            _permit: permit,
            _load_guard: load_guard,
        };
        Ok(Some(NarStreamData::new(headers, Box::pin(stream), url)))
    }

    fn try_segmented_response(
        &self,
        url: Url,
        response: Response,
        permit: ThrottlerPermit,
        headers: PassthroughHeaders,
    ) -> AnyhowResult<Option<NarStreamData>> {
        let stream_headers = extract_stream_headers(&response);
        let Some(content_length) = stream_headers.content_length else {
            return Self::wrap_ok_response(
                url,
                response,
                permit,
                self.load_tracker.enter(),
            );
        };

        if !is_segmented_eligible(
            &self.download_config,
            &self.load_tracker,
            content_length,
            accepts_byte_ranges(&response),
            stream_headers.content_encoding.as_deref(),
        ) {
            return Self::wrap_ok_response(
                url,
                response,
                permit,
                self.load_tracker.enter(),
            );
        }

        tracing::debug!(
            %url,
            content_length,
            load = self.load_tracker.current(),
            "starting segmented nar download"
        );

        start_segmented_download(
            self.client.clone(),
            Arc::clone(&self.throttler),
            Arc::clone(&self.credentials),
            self.download_config.clone(),
            self.load_tracker.clone(),
            self.load_tracker.enter(),
            permit,
            url.clone(),
            headers,
            content_length,
            stream_headers,
            response,
        )
        .map(Some)
    }
}

#[async_trait]
impl NarStreamProvider for ReqwestNarStreamProvider {
    async fn stream_nar(
        &self,
        locations: &[NarFileLocation],
        headers: &PassthroughHeaders,
    ) -> AnyhowResult<Option<NarStreamData>> {
        if locations.is_empty() {
            return Ok(None);
        }

        let mut set = JoinSet::new();
        for location in locations {
            let location = location.clone();
            let headers = headers.clone();

            let client = self.client.clone();
            let throttler = Arc::clone(&self.throttler);
            let credentials = Arc::clone(&self.credentials);

            set.spawn(async move {
                let permit = throttler.acquire(location.source_url().host()).await;

                let mut request = client
                    .get(location.source_url().value())
                    .headers(headers.to_headers());

                if let Some(credential) = credentials.lookup(location.source_url()) {
                    request =
                        request.basic_auth(credential.login.clone(), credential.secret.clone());
                }

                let response = if let Some(timeout) = location.timeout() {
                    tokio::time::timeout(timeout, request.send()).await
                } else {
                    Ok(request.send().await)
                };
                (location.clone(), response, permit)
            });
        }

        let mut not_found_count = 0;

        while let Some(result) = set.join_next().await {
            let Ok((location, response, permit)) = result else {
                continue;
            };
            let url = location.source_url();

            match response {
                Ok(Ok(response)) => match response.status() {
                    StatusCode::OK => {
                        return self.try_segmented_response(
                            url.clone(),
                            response,
                            permit,
                            headers.clone(),
                        );
                    }
                    StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => {
                        not_found_count += 1;
                    }
                    status => {
                        tracing::debug!(%url, %status, "received unexpected status from substituter");
                    }
                },
                Ok(Err(e)) => {
                    tracing::debug!(%url, error = %e, "failed to request nar from substituter");
                }
                Err(_) => {
                    if let Some(timeout) = location.timeout() {
                        tracing::debug!(%url, timeout_secs = %timeout.as_secs(), "timeout for requesting nar from substituter elapsed");
                    }
                }
            }
        }

        if not_found_count == locations.len() {
            Ok(None)
        } else {
            Err(anyhow::anyhow!("could not fetch nar from any substituter"))
        }
    }
}

fn extract_stream_headers(response: &Response) -> NarStreamHeaders {
    NarStreamHeaders {
        content_length: response.content_length(),
        content_type: response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string),
        content_encoding: response
            .headers()
            .get(header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string),
    }
}

fn accepts_byte_ranges(response: &Response) -> bool {
    response
        .headers()
        .get(header::ACCEPT_RANGES)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("bytes"))
}

fn is_segmented_eligible(
    config: &DownloadConfiguration,
    load_tracker: &DownloadLoadTracker,
    content_length: u64,
    accept_ranges: bool,
    content_encoding: Option<&str>,
) -> bool {
    config.segmented
        && load_tracker.current() <= config.segmented_load_threshold
        && content_length >= config.segmented_min_file_bytes
        && accept_ranges
        && content_encoding.is_none()
}

struct ThrottledStream<S> {
    inner: S,
    _permit: ThrottlerPermit,
    _load_guard: LoadGuard,
}

impl<S> Stream for ThrottledStream<S>
where
    S: Stream + Unpin,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().inner.poll_next_unpin(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmented_eligibility_requires_all_conditions() {
        let config = DownloadConfiguration {
            segmented: true,
            segmented_min_file_bytes: 1024,
            segmented_max_connections: 4,
            segmented_load_threshold: 3,
            segmented_buffer_bytes: 4096,
        };
        let tracker = DownloadLoadTracker::new();

        assert!(is_segmented_eligible(
            &config,
            &tracker,
            2048,
            true,
            None
        ));
        assert!(!is_segmented_eligible(
            &config,
            &tracker,
            512,
            true,
            None
        ));
        assert!(!is_segmented_eligible(
            &config,
            &tracker,
            2048,
            false,
            None
        ));
        assert!(!is_segmented_eligible(
            &config,
            &tracker,
            2048,
            true,
            Some("gzip")
        ));
    }
}
