pub mod api;

mod error;
mod router;

pub use error::WebAppError;
pub use router::build_router;
