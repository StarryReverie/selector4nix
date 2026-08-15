use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use futures::StreamExt;
use http::{HeaderMap, Response, header};
use selector4nix_core::AppContext;
use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::common::query_parameters::QueryParameters;
use selector4nix_core::domain::nar_file::model::NarFileKey;
use selector4nix_core::domain::nar_file::port::NarStreamData;
use selector4nix_core::domain::nar_info::model::{NarFileName, ProxyNarInfoData};

use crate::WebAppError;

pub async fn get_nar(
    State(ctx): State<Arc<AppContext>>,
    Path(path): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response<Body>, WebAppError> {
    let nar_file = NarFileName::new(path)?;
    let key = NarFileKey::from_file_name(&nar_file);

    let upstream_query = query
        .get(ProxyNarInfoData::UPSTREAM_QUERY_KEY)
        .map(|qp| QueryParameters::decode(qp))
        .transpose()?;

    let headers = PassthroughHeaders::extract(headers).proxyed();
    let data = ctx
        .stream_nar_file_usecase
        .run(key, upstream_query, headers)
        .await?;
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
