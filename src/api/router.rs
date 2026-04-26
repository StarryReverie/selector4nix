use axum::Router;
use axum::routing::get;

use crate::api::handlers::{cache_info, substituter};
use crate::api::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/substituters/available", get(substituter::get_available))
        .route("/nix-cache-info", get(cache_info::get_nix_cache_info))
        .with_state(state)
}
