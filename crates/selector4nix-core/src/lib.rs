#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]

pub mod api;
pub mod application;
pub mod domain;
pub mod infrastructure;

mod error;

pub use error::{AppError, AppErrorKind, AppResultExt};
