use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Response, header};
use axum::response::IntoResponse;

use crate::api::state::AppContext;

pub async fn get_nix_cache_info(State(ctx): State<Arc<AppContext>>) -> impl IntoResponse {
    let cache_info = ctx.cache_info();
    let body = format!(
        "StoreDir: {}\nWantMassQuery: {}\nPriority: {}\n",
        cache_info.store_dir,
        if cache_info.want_mass_query { 1 } else { 0 },
        cache_info.priority.value(),
    );
    Response::builder()
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::new(body))
        .unwrap()
}
