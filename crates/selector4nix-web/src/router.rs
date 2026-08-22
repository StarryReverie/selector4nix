use std::sync::Arc;

use axum::Router;
use axum::extract::Path;
use axum::response::Redirect;
use axum::routing::get;
use selector4nix_core::AppContext;
use selector4nix_core::AppError;

use crate::api::*;
use crate::dashboard::*;

pub fn build_router(ctx: Arc<AppContext>) -> Router {
    let router = Router::new()
        .route("/health", get(get_health))
        .route("/nix-cache-info", get(get_nix_cache_info))
        .route("/log/{filename}", get(get_log))
        .route("/nar/{filename}", get(get_nar))
        .route(
            "/{filename}",
            get(async move |state, filename: Path<String>, headers| {
                if filename.0.ends_with(".narinfo") {
                    get_nar_info(state, filename, headers).await
                } else if filename.0.ends_with(".ls") {
                    get_ls(state, filename, headers).await
                } else {
                    Err(AppError::not_found("path not found").into())
                }
            }),
        );

    let router = router
        .route("/", get(async move || Redirect::permanent("/dashboard/")))
        .route("/dashboard/", get(get_overview_page))
        .route("/dashboard/transferring", get(get_transferring_page))
        .route("/dashboard/cache", get(get_cache_page))
        .route("/dashboard/configuration", get(get_configuration_page))
        .route("/dashboard/static/{path}", get(get_static_asset));

    router.with_state(ctx)
}
