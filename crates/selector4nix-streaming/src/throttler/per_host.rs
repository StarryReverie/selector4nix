use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{Semaphore, TryAcquireError};

use crate::throttler::ThrottlerPermit;

#[derive(Debug, Clone)]
pub struct ThrottlingOptions {
    pub default_max_concurrent_requests: NonZeroUsize,
    pub per_host_max_concurrent_requests: HashMap<String, NonZeroUsize>,
}

impl ThrottlingOptions {
    pub fn new(default_max_concurrent_requests: NonZeroUsize) -> Self {
        Self {
            default_max_concurrent_requests,
            per_host_max_concurrent_requests: HashMap::new(),
        }
    }
}

pub struct PerHostHttpThrottler {
    options: ThrottlingOptions,
    semaphores: DashMap<String, Arc<Semaphore>>,
}

impl PerHostHttpThrottler {
    pub fn new(options: ThrottlingOptions) -> Self {
        Self {
            options,
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
            let entry = entry.or_insert_with(|| Arc::new(Semaphore::new(self.limit_for(host))));
            Arc::clone(entry.value())
        }
    }

    fn limit_for(&self, host: &str) -> usize {
        self.options
            .per_host_max_concurrent_requests
            .get(host)
            .map_or(
                self.options.default_max_concurrent_requests.get(),
                |limit| limit.get(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limit_applies_to_unregistered_hosts() {
        let throttler =
            PerHostHttpThrottler::new(ThrottlingOptions::new(NonZeroUsize::new(2).unwrap()));

        // Hold the permits so they are not released by dropping temporaries.
        let permits: Vec<_> = (0..2)
            .map(|_| throttler.try_acquire("default.example.com").unwrap())
            .collect();
        assert!(throttler.try_acquire("default.example.com").is_none());
        drop(permits);
        assert!(throttler.try_acquire("default.example.com").is_some());
    }

    #[test]
    fn per_host_limit_overrides_default() {
        let options = ThrottlingOptions {
            default_max_concurrent_requests: NonZeroUsize::new(2).unwrap(),
            per_host_max_concurrent_requests: [(
                "limited.example.com".to_string(),
                NonZeroUsize::new(1).unwrap(),
            )]
            .into(),
        };
        let throttler = PerHostHttpThrottler::new(options);

        let limited_permit = throttler.try_acquire("limited.example.com").unwrap();
        assert!(throttler.try_acquire("limited.example.com").is_none());
        drop(limited_permit);
        assert!(throttler.try_acquire("limited.example.com").is_some());

        // Unregistered hosts keep the default limit.
        let permits: Vec<_> = (0..2)
            .map(|_| throttler.try_acquire("default.example.com").unwrap())
            .collect();
        assert!(throttler.try_acquire("default.example.com").is_none());
        drop(permits);
        assert!(throttler.try_acquire("default.example.com").is_some());
    }
}
