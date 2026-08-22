use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use http::{HeaderMap, Response, header};
use selector4nix_core::AppContext;
use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::derivation::model::DerivingPath;

use crate::WebAppError;

pub async fn get_log(
    State(ctx): State<Arc<AppContext>>,
    Path(filename): Path<String>,
    headers: HeaderMap,
) -> Result<Response<Body>, WebAppError> {
    let deriving_path = DerivingPath::new(filename)?;
    let headers = PassthroughHeaders::extract(headers).proxyed();

    let data = ctx
        .get_derivation_log_usecase
        .run(deriving_path, headers)
        .await?;

    let response = Response::builder()
        .header(header::CONTENT_TYPE, "text/plain")
        .header(
            header::CONTENT_ENCODING,
            data.content_encoding.unwrap_or("identity".into()),
        )
        .body(Body::from(data.content))
        .unwrap();
    Ok(response)
}
