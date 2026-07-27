mod adapter;
mod per_host;

pub use adapter::ThrottlerAdapter;
pub use per_host::PerHostHttpThrottler;

use tokio::sync::OwnedSemaphorePermit;

#[expect(dead_code)]
pub struct ThrottlerPermit(OwnedSemaphorePermit);
