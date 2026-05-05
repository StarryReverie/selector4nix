use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response, header};

use crate::api::state::AppContext;
use crate::application::{AppError, AppErrorKind};
use crate::domain::nar::model::StorePathHash;

pub async fn get_nar_info(
    State(ctx): State<Arc<AppContext>>,
    Path(filename): Path<String>,
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

    let data = ctx.nar_usecase().get_nar_info(hash).await?;
    let response = Response::builder()
        .header(header::CONTENT_TYPE, "text/x-nix-narinfo")
        .body(Body::from(data.content().to_string()))
        .unwrap();
    Ok(response)
}
