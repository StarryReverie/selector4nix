use std::sync::Arc;

use bytes::Bytes;
use futures::StreamExt;
use selector4nix::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix::domain::common::url::Url;
use selector4nix::domain::nar_file::model::NarFileLocation;
use selector4nix::domain::nar_file::port::NarStreamProvider;
use selector4nix::infrastructure::config::{AppCredential, DownloadConfiguration};
use selector4nix::infrastructure::provider::ReqwestNarStreamProvider;
use selector4nix::infrastructure::util::{DownloadLoadTracker, PerHostHttpThrottler};

use super::mock::range_server::RangeNarServer;

fn segmented_download_config() -> DownloadConfiguration {
    DownloadConfiguration {
        segmented: true,
        segmented_min_file_bytes: 1024,
        segmented_max_connections: 4,
        segmented_load_threshold: 3,
        segmented_buffer_bytes: 64 * 1024,
    }
}

fn make_provider(
    config: DownloadConfiguration,
    load_tracker: DownloadLoadTracker,
) -> ReqwestNarStreamProvider {
    ReqwestNarStreamProvider::new(
        reqwest::Client::new(),
        Arc::new(PerHostHttpThrottler::new(8)),
        Arc::new(AppCredential::empty()),
        config,
        load_tracker,
    )
}

async fn collect_stream(provider: &ReqwestNarStreamProvider, url: &Url) -> Vec<u8> {
    let location = NarFileLocation::new(url.clone(), None);
    let data = provider
        .stream_nar(&[location], &PassthroughHeaders::empty())
        .await
        .unwrap()
        .expect("nar stream should be available");

    let mut body = Vec::new();
    let mut stream = data.inner;
    while let Some(chunk) = stream.next().await {
        body.extend_from_slice(&chunk.unwrap());
    }
    body
}

#[tokio::test]
async fn segmented_download_returns_full_object() {
    let payload: Vec<u8> = (0..16 * 1024).map(|i| (i % 251) as u8).collect();
    let server = RangeNarServer::start(Bytes::from(payload.clone())).await;
    let url = Url::new(&format!("{}/nar/test.nar.xz", server.base_url)).unwrap();
    let provider = make_provider(segmented_download_config(), DownloadLoadTracker::new());

    let body = collect_stream(&provider, &url).await;
    assert_eq!(body, payload);
}

#[tokio::test]
async fn segmented_download_handles_file_larger_than_buffer() {
    // The object is much larger than the reassembly buffer. Trailing segments
    // cannot be fully buffered ahead of the cursor, so the download only
    // completes if the leading segment keeps streaming to the client while the
    // rest is still being fetched.
    let payload: Vec<u8> = (0..256 * 1024).map(|i| (i % 251) as u8).collect();
    let server = RangeNarServer::start(Bytes::from(payload.clone())).await;
    let url = Url::new(&format!("{}/nar/test.nar.xz", server.base_url)).unwrap();
    let provider = make_provider(
        DownloadConfiguration {
            segmented_buffer_bytes: 16 * 1024,
            ..segmented_download_config()
        },
        DownloadLoadTracker::new(),
    );

    let body = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        collect_stream(&provider, &url),
    )
    .await
    .expect("segmented download must not deadlock on files larger than the buffer");
    assert_eq!(body, payload);
}

#[tokio::test]
async fn segmented_download_uses_multiple_range_requests() {
    let payload: Vec<u8> = (0..16 * 1024).map(|i| (i % 251) as u8).collect();
    let server = RangeNarServer::start(Bytes::from(payload.clone())).await;
    let url = Url::new(&format!("{}/nar/test.nar.xz", server.base_url)).unwrap();
    let provider = make_provider(segmented_download_config(), DownloadLoadTracker::new());

    let body = collect_stream(&provider, &url).await;
    assert_eq!(body, payload);
    assert_eq!(server.full_request_count(), 1);
    assert!(server.range_request_count() >= 2);
}

#[tokio::test]
async fn segmented_download_falls_back_when_feature_disabled() {
    let payload: Vec<u8> = (0..16 * 1024).map(|i| (i % 251) as u8).collect();
    let server = RangeNarServer::start(Bytes::from(payload.clone())).await;
    let url = Url::new(&format!("{}/nar/test.nar.xz", server.base_url)).unwrap();
    let provider = make_provider(
        DownloadConfiguration {
            segmented: false,
            ..segmented_download_config()
        },
        DownloadLoadTracker::new(),
    );

    let body = collect_stream(&provider, &url).await;
    assert_eq!(body, payload);
    assert_eq!(server.full_request_count(), 1);
    assert_eq!(server.range_request_count(), 0);
}

#[tokio::test]
async fn segmented_download_falls_back_for_small_files() {
    let payload = vec![1u8, 2, 3, 4, 5];
    let server = RangeNarServer::start(Bytes::from(payload.clone())).await;
    let url = Url::new(&format!("{}/nar/test.nar.xz", server.base_url)).unwrap();
    let provider = make_provider(segmented_download_config(), DownloadLoadTracker::new());

    let body = collect_stream(&provider, &url).await;
    assert_eq!(body, payload);
    assert_eq!(server.full_request_count(), 1);
    assert_eq!(server.range_request_count(), 0);
}

#[tokio::test]
async fn segmented_download_falls_back_without_accept_ranges() {
    let payload: Vec<u8> = (0..16 * 1024).map(|i| (i % 251) as u8).collect();
    let server = RangeNarServer::start_without_ranges(Bytes::from(payload.clone())).await;
    let url = Url::new(&format!("{}/nar/test.nar.xz", server.base_url)).unwrap();
    let provider = make_provider(segmented_download_config(), DownloadLoadTracker::new());

    let body = collect_stream(&provider, &url).await;
    assert_eq!(body, payload);
    assert_eq!(server.full_request_count(), 1);
    assert_eq!(server.range_request_count(), 0);
}

#[tokio::test]
async fn segmented_download_falls_back_when_load_is_high() {
    let payload: Vec<u8> = (0..16 * 1024).map(|i| (i % 251) as u8).collect();
    let server = RangeNarServer::start(Bytes::from(payload.clone())).await;
    let url = Url::new(&format!("{}/nar/test.nar.xz", server.base_url)).unwrap();

    let tracker = DownloadLoadTracker::new();
    let _in_flight = [
        tracker.enter(),
        tracker.enter(),
        tracker.enter(),
        tracker.enter(),
    ];

    let provider = make_provider(
        DownloadConfiguration {
            segmented_load_threshold: 3,
            ..segmented_download_config()
        },
        tracker,
    );

    let body = collect_stream(&provider, &url).await;
    assert_eq!(body, payload);
    assert_eq!(server.full_request_count(), 1);
    assert_eq!(server.range_request_count(), 0);
}

#[tokio::test]
async fn load_guard_tracks_in_flight_downloads() {
    let tracker = DownloadLoadTracker::new();
    assert_eq!(tracker.current(), 0);

    let guard = tracker.enter();
    assert_eq!(tracker.current(), 1);
    drop(guard);
    assert_eq!(tracker.current(), 0);
}
