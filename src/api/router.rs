use std::sync::Arc;

use axum::Router;
use axum::routing::get;

use crate::api::handlers::cache_info::get_nix_cache_info;
use crate::api::handlers::nar::get_nar;
use crate::api::handlers::nar_info::get_nar_info;
use crate::api::handlers::substituter::get_available_substituters;
use crate::api::state::AppContext;

pub fn build_router(ctx: Arc<AppContext>) -> Router {
    Router::new()
        .route("/substituters/available", get(get_available_substituters))
        .route("/nix-cache-info", get(get_nix_cache_info))
        .route("/nar/{path}", get(get_nar))
        .route("/{filename}", get(get_nar_info))
        .with_state(ctx)
}
