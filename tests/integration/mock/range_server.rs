use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use bytes::Bytes;
use tokio::net::TcpListener;

#[derive(Clone)]
struct ServerState {
    data: Arc<Bytes>,
    accept_ranges: bool,
    full_requests: Arc<AtomicUsize>,
    range_requests: Arc<AtomicUsize>,
}

pub struct RangeNarServer {
    pub base_url: String,
    full_requests: Arc<AtomicUsize>,
    range_requests: Arc<AtomicUsize>,
    handle: tokio::task::JoinHandle<()>,
}

impl RangeNarServer {
    pub fn full_request_count(&self) -> usize {
        self.full_requests.load(Ordering::Relaxed)
    }

    pub fn range_request_count(&self) -> usize {
        self.range_requests.load(Ordering::Relaxed)
    }

    pub async fn start(data: Bytes) -> Self {
        Self::start_with_options(data, true).await
    }

    pub async fn start_without_ranges(data: Bytes) -> Self {
        Self::start_with_options(data, false).await
    }

    async fn start_with_options(data: Bytes, accept_ranges: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind range nar server");
        let addr = listener.local_addr().expect("failed to read local addr");
        let full_requests = Arc::new(AtomicUsize::new(0));
        let range_requests = Arc::new(AtomicUsize::new(0));
        let state = ServerState {
            data: Arc::new(data),
            accept_ranges,
            full_requests: Arc::clone(&full_requests),
            range_requests: Arc::clone(&range_requests),
        };
        let app = Router::new()
            .route("/nar/test.nar.xz", get(serve_nar))
            .with_state(state);

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("range nar server failed");
        });

        Self {
            base_url: format!("http://{addr}"),
            full_requests,
            range_requests,
            handle,
        }
    }
}

impl Drop for RangeNarServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn serve_nar(
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    let total = state.data.len();

    if state.accept_ranges {
        if let Some(range) = headers.get(header::RANGE).and_then(|value| value.to_str().ok()) {
            if let Some((start, end)) = parse_range_header(range, total) {
                state
                    .range_requests
                    .fetch_add(1, Ordering::Relaxed);
                let slice = state.data.slice(start..=end);
                let headers = [
                    (header::CONTENT_TYPE, "application/x-nix-nar".to_string()),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                    (
                        header::CONTENT_RANGE,
                        format!("bytes {start}-{end}/{total}"),
                    ),
                    (header::CONTENT_LENGTH, slice.len().to_string()),
                ];
                return (StatusCode::PARTIAL_CONTENT, headers, slice).into_response();
            }
        }
    }

    if state.accept_ranges {
        state.full_requests.fetch_add(1, Ordering::Relaxed);
        let headers = [
            (header::CONTENT_TYPE, "application/x-nix-nar".to_string()),
            (header::CONTENT_LENGTH, total.to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
        ];
        return (
            StatusCode::OK,
            headers,
            (*state.data).clone(),
        )
            .into_response();
    }

    let headers = [
        (header::CONTENT_TYPE, "application/x-nix-nar".to_string()),
        (header::CONTENT_LENGTH, total.to_string()),
    ];
    state.full_requests.fetch_add(1, Ordering::Relaxed);
    (
        StatusCode::OK,
        headers,
        (*state.data).clone(),
    )
        .into_response()
}

fn parse_range_header(range: &str, total: usize) -> Option<(usize, usize)> {
    let range = range.strip_prefix("bytes=")?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse::<usize>().ok()?;
    let end = if end.is_empty() {
        total.saturating_sub(1)
    } else {
        end.parse::<usize>().ok()?
    };
    if start > end || end >= total {
        return None;
    }
    Some((start, end))
}
