use std::sync::Arc;

use axum::Router;
use axum::routing::get;

use crate::api::state::AppContext;

pub fn build_router(ctx: Arc<AppContext>) -> Router {
    Router::new()
        .route(
            "/substituters/available",
            get(super::handlers::substituter::get_available_substituters),
        )
        .route(
            "/nix-cache-info",
            get(super::handlers::cache_info::get_nix_cache_info),
        )
        .with_state(ctx)
}
