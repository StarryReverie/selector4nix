use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use http::{HeaderMap, Response, header};

use crate::api::state::AppContext;
use crate::application::{AppError, AppErrorKind};
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_info::model::StorePathHash;

pub async fn get_nar_info(
    State(ctx): State<Arc<AppContext>>,
    Path(filename): Path<String>,
    headers: HeaderMap,
) -> Result<Response<Body>, AppError> {
    let hash = match filename.strip_suffix(".narinfo") {
        Some(hash) => StorePathHash::new(hash.into())?,
        None => {
            return Err(AppError::message(
                AppErrorKind::Input,
                "missing nar info file",
            ));
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
