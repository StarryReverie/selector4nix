use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use http::{HeaderMap, Response, header};
use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::nar_info::model::StorePathHash;
use selector4nix_core::{AppContext, AppError};

use crate::WebAppError;

pub async fn get_ls(
    State(ctx): State<Arc<AppContext>>,
    Path(filename): Path<String>,
    headers: HeaderMap,
) -> Result<Response<Body>, WebAppError> {
    let hash = match filename.strip_suffix(".ls") {
        Some(hash) => StorePathHash::new(hash.into())?,
        None => {
            return Err(AppError::input("expects `{storePathHash}.ls`").into());
        }
    };

    let headers = PassthroughHeaders::extract(headers).proxyed();

    let data = ctx
        .list_nar_inner_directory_usecase
        .run(hash, headers)
        .await?;

    let response = Response::builder()
        .header(
            header::CONTENT_TYPE,
            data.content_type.unwrap_or("application/json".into()),
        )
        .header(
            header::CONTENT_ENCODING,
            data.content_encoding.unwrap_or("identity".into()),
        )
        .body(Body::from(data.content))
        .unwrap();
    Ok(response)
}
