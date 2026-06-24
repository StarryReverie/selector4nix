use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

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
        let semaphore = if let Some(semaphore) = self.semaphores.get(host) {
            Arc::clone(semaphore.value())
        } else {
            loop {
                // Dashmap is not async-aware, so directly calling `entry()` may cause deadlock if
                // another task locked the entry in the same thread.
                if let Some(entry) = self.semaphores.try_entry(host.into()) {
                    // Use `or_insert()` to prevent duplicated insertion.
                    let entry = entry
                        .or_insert_with(|| Arc::new(Semaphore::new(self.max_concurrent_requests)));
                    // Explicitly exit the write transaction and get the readonly entry later.
                    drop(entry);

                    let semaphore = self
                        .semaphores
                        .get(host)
                        .expect("the semaphore should have already been inserted");
                    break Arc::clone(semaphore.value());
                } else {
                    tokio::task::yield_now().await;
                }
            }
        };

        let permit = semaphore
            .acquire_owned()
            .await
            .expect("the semaphore should not be closed");
        ThrottlerPermit(permit)
    }
}

#[expect(dead_code)]
pub struct ThrottlerPermit(OwnedSemaphorePermit);
