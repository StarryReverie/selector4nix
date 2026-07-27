#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]

pub mod application;
pub mod domain;
pub mod infrastructure;

mod context;
mod error;

pub use context::AppContext;
pub use error::{AppError, AppErrorKind, AppResultExt};
