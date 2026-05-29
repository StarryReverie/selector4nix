use crate::application::{AppError, AppErrorKind};
use crate::domain::common::url::TryNewUrlError;
use crate::domain::substituter::model::TryNewPriorityError;

impl From<TryNewPriorityError> for AppError {
    fn from(error: TryNewPriorityError) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}

impl From<TryNewUrlError> for AppError {
    fn from(error: TryNewUrlError) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}
