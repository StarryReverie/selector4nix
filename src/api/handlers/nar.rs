use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response, StatusCode, header};
use axum::response::IntoResponse;
use futures::StreamExt;

use crate::api::state::AppContext;
use crate::domain::nar::model::NarFileName;
use crate::domain::nar::port::NarStreamOutcome;

pub async fn get_nar(
    State(ctx): State<Arc<AppContext>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let nar_file = match NarFileName::new(path) {
        Ok(name) => name,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match ctx.nar_usecase().stream_nar(&nar_file).await {
        Ok(NarStreamOutcome::Found { stream, .. }) => {
            let stream = stream
                .inner
                .map(|result| result.map_err(|e| e.into_boxed_dyn_error()));
            Response::builder()
                .header(header::CONTENT_TYPE, "application/x-nix-nar")
                .body(Body::from_stream(stream))
                .unwrap()
        }
        Ok(NarStreamOutcome::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::BAD_GATEWAY.into_response(),
    }
}
