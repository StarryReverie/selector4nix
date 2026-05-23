use crate::application::{AppError, AppErrorKind};
use crate::domain::nar_file::StreamNarFileError;

impl From<StreamNarFileError> for AppError {
    fn from(error: StreamNarFileError) -> Self {
        match error {
            StreamNarFileError::Infrastructure => Self::new(AppErrorKind::Infrastructure, error),
        }
    }
}
