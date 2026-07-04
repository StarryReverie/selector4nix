use std::sync::Arc;

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
}

pub struct RangeNarServer {
    pub base_url: String,
    handle: tokio::task::JoinHandle<()>,
}

impl RangeNarServer {
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
        let state = ServerState {
            data: Arc::new(data),
            accept_ranges,
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
