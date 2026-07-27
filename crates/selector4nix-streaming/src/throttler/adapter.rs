use std::sync::Arc;

use crate::stream::ChunkTrottler;
use crate::throttler::{PerHostHttpThrottler, ThrottlerPermit};

pub struct ThrottlerAdapter {
    inner: Arc<PerHostHttpThrottler>,
    host: String,
}

impl ThrottlerAdapter {
    pub fn new(inner: Arc<PerHostHttpThrottler>, host: String) -> Self {
        Self { inner, host }
    }
}

impl ChunkTrottler for ThrottlerAdapter {
    fn try_acquire(&self) -> Option<ThrottlerPermit> {
        self.inner.try_acquire(&self.host)
    }
}
