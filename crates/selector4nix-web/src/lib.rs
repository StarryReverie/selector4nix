pub mod api;
pub mod dashboard;

mod error;
mod router;

pub use error::WebAppError;
pub use router::build_router;
