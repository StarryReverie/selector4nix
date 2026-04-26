use axum::body::Body;
use axum::http::{Response, header};
use axum::response::IntoResponse;

pub async fn get_nix_cache_info() -> impl IntoResponse {
    let body = "StoreDir: /nix/store\nWantMassQuery: 1\nPriority: 40\n";
    Response::builder()
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::new(body.to_string()))
        .unwrap()
}
