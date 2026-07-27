use axum::response::{IntoResponse, Response};
use http::StatusCode;

use selector4nix_core::{AppError, AppErrorKind};

pub struct WebAppError(pub AppError);

impl<E> From<E> for WebAppError
where
    E: Into<AppError>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

impl IntoResponse for WebAppError {
    fn into_response(self) -> Response {
        let this = self.0;
        let status = match this.kind() {
            AppErrorKind::Input => StatusCode::BAD_REQUEST,
            AppErrorKind::NotFound => StatusCode::NOT_FOUND,
            AppErrorKind::Rule => StatusCode::UNPROCESSABLE_ENTITY,
            AppErrorKind::Infrastructure => StatusCode::BAD_GATEWAY,
            AppErrorKind::Catastrophic => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = match this.kind() {
            AppErrorKind::Input => format!("400 BAD REQUEST - {this}"),
            AppErrorKind::NotFound => format!("404 NOT FOUND - {this}"),
            AppErrorKind::Rule => format!("422 UNPROCESSABLE ENTITY - {this}"),
            AppErrorKind::Infrastructure => "502 BAD GATEWAY - infrastructure error".into(),
            AppErrorKind::Catastrophic => "500 INTERNAL SERVER ERROR - unknown error".into(),
        };
        (status, message).into_response()
    }
}
