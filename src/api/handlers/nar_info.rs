use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response, StatusCode, header};
use axum::response::IntoResponse;

use crate::api::state::AppContext;
use crate::domain::nar::model::StorePathHash;
use crate::domain::nar::service::ResolveNarInfoError;

pub async fn get_nar_info(
    State(ctx): State<Arc<AppContext>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let hash = match filename.strip_suffix(".narinfo") {
        None => return StatusCode::NOT_FOUND.into_response(),
        Some(hash) => match StorePathHash::new(hash.into()) {
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
            Ok(hash) => hash,
        },
    };

    match ctx.nar_usecase().get_nar_info(hash).await {
        Ok(data) => Response::builder()
            .header(header::CONTENT_TYPE, "text/x-nix-narinfo")
            .body(Body::from(data.content().to_string()))
            .unwrap(),
        Err(ResolveNarInfoError::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(ResolveNarInfoError::Fetch) => StatusCode::BAD_GATEWAY.into_response(),
    }
}
