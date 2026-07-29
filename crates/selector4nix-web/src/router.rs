use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use selector4nix_core::AppContext;

use crate::api::*;

pub fn build_router(ctx: Arc<AppContext>) -> Router {
    Router::new()
        .route("/", get(get_index))
        .route("/health", get(get_health))
        .route("/status", get(get_status))
        .route("/substituters/available", get(get_available_substituters))
        .route("/nix-cache-info", get(get_nix_cache_info))
        .route("/nar/{path}", get(get_nar))
        .route("/{filename}", get(get_nar_info))
        .with_state(ctx)
}
