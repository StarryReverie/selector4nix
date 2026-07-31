use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Query, State};
use http::{Response, header};
use selector4nix_core::AppContext;
use selector4nix_core::domain::substituter::model::Priority;
use serde::Deserialize;

use crate::WebAppError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct NixCacheInfoQuery {
    priority: Option<u32>,
}

pub async fn get_nix_cache_info(
    Query(query): Query<NixCacheInfoQuery>,
    State(ctx): State<Arc<AppContext>>,
) -> Result<Response<Body>, WebAppError> {
    let priority = query.priority.map(Priority::new).transpose()?;

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
