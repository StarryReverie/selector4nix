use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{Semaphore, TryAcquireError};

use crate::throttler::ThrottlerPermit;

pub struct PerHostHttpThrottler {
    max_concurrent_requests: usize,
    semaphores: DashMap<String, Arc<Semaphore>>,
}

impl PerHostHttpThrottler {
    pub fn new(max_concurrent_requests: usize) -> Self {
        Self {
            max_concurrent_requests,
            semaphores: DashMap::new(),
        }
    }

    pub async fn acquire(&self, host: &str) -> ThrottlerPermit {
        let semaphore = self.ensure_semaphore(host);
        let permit = semaphore
            .acquire_owned()
            .await
            .expect("the semaphore should not be closed");
        ThrottlerPermit(permit)
    }

    pub fn try_acquire(&self, host: &str) -> Option<ThrottlerPermit> {
        let semaphore = self.ensure_semaphore(host);
        match semaphore.try_acquire_owned() {
            Ok(permit) => Some(ThrottlerPermit(permit)),
            Err(TryAcquireError::NoPermits) => None,
            Err(TryAcquireError::Closed) => unreachable!("the semaphore should not be closed"),
        }
    }

    fn ensure_semaphore(&self, host: &str) -> Arc<Semaphore> {
        if let Some(semaphore) = self.semaphores.get(host) {
            Arc::clone(semaphore.value())
        } else {
            let entry = self.semaphores.entry(host.into());
            // Use `or_insert_with()` to prevent duplicated insertion.
            let entry =
                entry.or_insert_with(|| Arc::new(Semaphore::new(self.max_concurrent_requests)));
            Arc::clone(entry.value())
        }
    }
}
