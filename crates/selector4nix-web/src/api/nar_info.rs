use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use http::{HeaderMap, Response, header};

use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::nar_info::model::StorePathHash;
use selector4nix_core::{AppContext, AppError};

use crate::WebAppError;

pub async fn get_nar_info(
    State(ctx): State<Arc<AppContext>>,
    Path(filename): Path<String>,
    headers: HeaderMap,
) -> Result<Response<Body>, WebAppError> {
    let hash = match filename.strip_suffix(".narinfo") {
        Some(hash) => StorePathHash::new(hash.into())?,
        None => {
            return Err(AppError::input("missing nar info file").into());
        }
    };

    let headers = PassthroughHeaders::extract(headers).proxyed();
    let data = ctx
        .nar_info_resolution_usecase()
        .get_nar_info(hash, headers)
        .await?;
    let response = Response::builder()
        .header(header::CONTENT_TYPE, "text/x-nix-narinfo")
        .body(Body::from(data.content().to_string()))
        .unwrap();
    Ok(response)
}
