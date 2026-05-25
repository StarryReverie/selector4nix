use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::application::{AppError, AppErrorKind};

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self.kind() {
            AppErrorKind::Input => StatusCode::BAD_REQUEST,
            AppErrorKind::NotFound => StatusCode::NOT_FOUND,
            AppErrorKind::Rule => StatusCode::UNPROCESSABLE_ENTITY,
            AppErrorKind::Infrastructure => StatusCode::BAD_GATEWAY,
            AppErrorKind::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = match self.kind() {
            AppErrorKind::Input => format!("400 BAD REQUEST - {self}"),
            AppErrorKind::NotFound => format!("404 NOT FOUND - {self}"),
            AppErrorKind::Rule => format!("422 UNPROCESSABLE ENTITY - {self}"),
            AppErrorKind::Infrastructure => "502 BAD GATEWAY - infrastructure error".into(),
            AppErrorKind::Unknown => "500 INTERNAL SERVER ERROR - unknown error".into(),
        };
        (status, message).into_response()
    }
}
