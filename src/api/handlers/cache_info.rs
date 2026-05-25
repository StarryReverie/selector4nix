use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{Response, header};
use serde::Deserialize;

use crate::api::state::AppContext;
use crate::application::AppError;
use crate::domain::substituter::model::Priority;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct NixCacheInfoQuery {
    priority: Option<u32>,
}

pub async fn get_nix_cache_info(
    Query(query): Query<NixCacheInfoQuery>,
    State(ctx): State<Arc<AppContext>>,
) -> Result<Response<Body>, AppError> {
    let priority = query
        .priority
        .map(|priority| Priority::new(priority))
        .transpose()?;

    let cache_info = ctx.cache_info();
    let body = format!(
        "StoreDir: {}\nWantMassQuery: {}\nPriority: {}\n",
        cache_info.store_dir,
        if cache_info.want_mass_query { 1 } else { 0 },
        priority.unwrap_or(cache_info.priority).value(),
    );

    let response = Response::builder()
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::new(body))
        .unwrap();
    Ok(response)
}
