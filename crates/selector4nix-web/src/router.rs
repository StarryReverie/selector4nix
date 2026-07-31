use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use selector4nix_core::AppContext;

use crate::api::*;
use crate::dashboard::*;

pub fn build_router(ctx: Arc<AppContext>) -> Router {
    let router = Router::new()
        .route("/health", get(get_health))
        .route("/status", get(get_status))
        .route("/nix-cache-info", get(get_nix_cache_info))
        .route("/nar/{path}", get(get_nar))
        .route("/{filename}", get(get_nar_info));

    let router = router
        .route("/", get(get_index))
        .route("/dashboard/", get(get_overview_page))
        .route("/dashboard/static/{path}", get(get_static_asset));

    router.with_state(ctx)
}
