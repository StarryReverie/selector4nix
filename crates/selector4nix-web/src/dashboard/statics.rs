use axum::body::Body;
use axum::extract::Path;
use http::{Response, header};
use rust_embed::Embed;
use selector4nix_core::AppError;

use crate::WebAppError;

#[derive(Embed)]
#[folder = "../../frontend/dist"]
struct StaticAssets;

pub async fn get_static_asset(Path(path): Path<String>) -> Result<Response<Body>, WebAppError> {
    let Some(file) = StaticAssets::get(&path) else {
        return Err(AppError::not_found("static resource not found").into());
    };

    let response = Response::builder()
        .header(header::CONTENT_TYPE, file.metadata.mimetype())
        .body(file.data.into())
        .unwrap();
    Ok(response)
}
