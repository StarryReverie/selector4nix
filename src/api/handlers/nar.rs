use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response, header};
use futures::StreamExt;

use crate::api::state::AppContext;
use crate::application::AppError;
use crate::domain::nar::model::NarFileName;
use crate::domain::nar::port::NarStreamData;

pub async fn get_nar(
    State(ctx): State<Arc<AppContext>>,
    Path(path): Path<String>,
) -> Result<Response<Body>, AppError> {
    let nar_file = NarFileName::new(path)?;

    let data = ctx.nar_usecase().stream_nar(&nar_file).await?;
    Ok(build_response(data))
}

fn build_response(stream: NarStreamData) -> Response<Body> {
    let builder = Response::builder();
    let builder = match stream.headers.content_length {
        Some(value) => builder.header(header::CONTENT_LENGTH, value),
        None => builder,
    };
    let builder = match stream.headers.content_type {
        Some(value) => builder.header(header::CONTENT_TYPE, value),
        None => builder.header(header::CONTENT_TYPE, "application/x-nix-nar"),
    };
    let builder = match stream.headers.content_encoding {
        Some(value) => builder.header(header::CONTENT_ENCODING, value),
        None => builder,
    };

    let stream = stream
        .inner
        .map(|res| res.map_err(|e| e.into_boxed_dyn_error()));
    builder.body(Body::from_stream(stream)).unwrap()
}
