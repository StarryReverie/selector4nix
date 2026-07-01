pub mod nar_file;
pub mod nar_info;
pub mod status;
pub mod substituter;

mod error;

pub use error::{AppError, AppErrorKind, AppOptionExt, AppResult, AppResultExt};
