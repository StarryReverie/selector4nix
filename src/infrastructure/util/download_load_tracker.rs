use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Default)]
pub struct DownloadLoadTracker(Arc<AtomicUsize>);

impl DownloadLoadTracker {
    pub fn new() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }

    pub fn current(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }

    pub fn enter(&self) -> LoadGuard {
        self.0.fetch_add(1, Ordering::Relaxed);
        LoadGuard(Arc::clone(&self.0))
    }
}

pub struct LoadGuard(Arc<AtomicUsize>);

impl Drop for LoadGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_and_drop_updates_current_load() {
        let tracker = DownloadLoadTracker::new();
        assert_eq!(tracker.current(), 0);

        let guard = tracker.enter();
        assert_eq!(tracker.current(), 1);

        let guard2 = tracker.enter();
        assert_eq!(tracker.current(), 2);

        drop(guard);
        assert_eq!(tracker.current(), 1);

        drop(guard2);
        assert_eq!(tracker.current(), 0);
    }
}
