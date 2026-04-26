use axum::http::{StatusCode, header};
use axum::response::IntoResponse;

pub async fn get_nix_cache_info() -> impl IntoResponse {
    let body = "StoreDir: /nix/store\nWantMassQuery: 1\nPriority: 40\n";
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/x-nix-cache-info")],
        body,
    )
}
