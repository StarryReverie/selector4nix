use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result as AnyhowResult};
use redb::Database;

use crate::cache_kv::{CacheKvInner, UnixTimestamp};

const CLEANUP_LIMIT: usize = 20;
const CLEANUP_PERIOD: UnixTimestamp = 30;

#[derive(Debug, Clone, Copy)]
pub enum UnixTimestampArg {
    Pure(UnixTimestamp),
    SystemNow,
}

impl UnixTimestampArg {
    fn get(self) -> UnixTimestamp {
        match self {
            UnixTimestampArg::Pure(now) => now,
            UnixTimestampArg::SystemNow => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("`UNIX_EPOCH` should be earlier than any other system time")
                .as_secs(),
        }
    }
}

pub struct CacheKv {
    inner: CacheKvInner,
    last_cleanup: AtomicU64,
}

impl CacheKv {
    pub fn new(db: Arc<Database>, entity_name: String) -> Self {
        Self {
            inner: CacheKvInner::new(db, entity_name),
            last_cleanup: 0.into(),
        }
    }

    pub fn get(
        &self,
        key: &[u8],
        now: UnixTimestampArg,
    ) -> AnyhowResult<Option<(UnixTimestamp, Vec<u8>)>> {
        let now = now.get();
        self.auto_cleanup(now)?;
        self.inner
            .get(key, now)
            .with_context(|| format!("could not get entry of key `{key:?}`"))
    }

    pub fn save(
        &self,
        key: &[u8],
        value: &[u8],
        expire_at: UnixTimestamp,
        now: UnixTimestampArg,
    ) -> AnyhowResult<()> {
        self.auto_cleanup(now.get())?;
        self.inner
            .save(key, value, expire_at)
            .with_context(|| format!("could not save entry of key `{key:?}`"))
    }

    pub fn remove(&self, key: &[u8], now: UnixTimestampArg) -> AnyhowResult<()> {
        self.auto_cleanup(now.get())?;
        self.inner
            .remove(key)
            .with_context(|| format!("could not remove entry of key `{key:?}`"))
    }

    pub fn len(&self) -> AnyhowResult<usize> {
        self.inner.len()
    }

    pub fn is_empty(&self) -> AnyhowResult<bool> {
        self.len().map(|len| len == 0)
    }

    pub fn cleanup(&self, now: UnixTimestampArg) -> AnyhowResult<usize> {
        self.inner
            .cleanup(now.get(), CLEANUP_LIMIT)
            .context("could not cleanup expired entries`")
    }

    fn auto_cleanup(&self, now: UnixTimestamp) -> AnyhowResult<usize> {
        let mut last_cleanup = self.last_cleanup.load(Ordering::Acquire);
        if last_cleanup.saturating_add(CLEANUP_PERIOD) > now {
            return Ok(0);
        }

        let expired_cnt = self.cleanup(UnixTimestampArg::Pure(now))?;

        while let Err(new_last_cleanup) = self.last_cleanup.compare_exchange(
            last_cleanup,
            now,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            if new_last_cleanup > now {
                break;
            }
            last_cleanup = new_last_cleanup;
        }

        Ok(expired_cnt)
    }
}

#[cfg(test)]
mod tests {
    use redb::backends::InMemoryBackend;

    use super::*;

    fn make_cache() -> CacheKv {
        let db = Database::builder()
            .create_with_backend(InMemoryBackend::new())
            .unwrap();
        CacheKv::new(Arc::new(db), "test".to_string())
    }

    fn now(secs: UnixTimestamp) -> UnixTimestampArg {
        UnixTimestampArg::Pure(secs)
    }

    #[test]
    fn save_get_remove_lifecycle() {
        let cache = make_cache();

        cache.save(b"key1", b"value1", 200, now(100)).unwrap();

        let (expire_at, value) = cache.get(b"key1", now(100)).unwrap().unwrap();
        assert_eq!(expire_at, 200);
        assert_eq!(value, b"value1");

        assert_eq!(cache.len().unwrap(), 1);

        cache.remove(b"key1", now(100)).unwrap();
        assert!(cache.get(b"key1", now(100)).unwrap().is_none());
        assert_eq!(cache.len().unwrap(), 0);
    }

    #[test]
    fn expired_entry_lazily_removed_on_get() {
        let cache = make_cache();

        cache.save(b"key1", b"value1", 50, now(100)).unwrap();
        assert_eq!(cache.len().unwrap(), 1);

        assert!(cache.get(b"key1", now(100)).unwrap().is_none());
        assert_eq!(cache.len().unwrap(), 0);
    }

    #[test]
    fn cleanup_removes_expired_up_to_limit_and_preserves_unexpired() {
        let cache = make_cache();

        for i in 0..5u8 {
            let key = [i];
            cache.save(&key, b"expired", 100, now(0)).unwrap();
        }
        for i in 5..7u8 {
            let key = [i];
            cache.save(&key, b"alive", 300, now(0)).unwrap();
        }

        assert_eq!(cache.cleanup(now(200)).unwrap(), 5);
        assert_eq!(cache.len().unwrap(), 2);

        for i in 5..7u8 {
            let (expire_at, value) = cache.get(&[i], now(200)).unwrap().unwrap();
            assert_eq!(expire_at, 300);
            assert_eq!(value, b"alive");
        }
    }

    #[test]
    fn auto_cleanup_triggers_on_period_elapsed() {
        let cache = make_cache();

        cache.save(b"key1", b"expired", 50, now(0)).unwrap();
        assert_eq!(cache.len().unwrap(), 1);

        let _ = cache.get(b"_", now(100)).unwrap();
        assert_eq!(cache.len().unwrap(), 0);
    }

    #[test]
    fn overwrite_updates_value_and_expiry() {
        let cache = make_cache();

        cache.save(b"key1", b"v1", 100, now(0)).unwrap();
        cache.save(b"key1", b"v2", 200, now(0)).unwrap();

        let (expire_at, value) = cache.get(b"key1", now(150)).unwrap().unwrap();
        assert_eq!(expire_at, 200);
        assert_eq!(value, b"v2");

        assert_eq!(cache.len().unwrap(), 1);
    }

    #[test]
    fn len_reflects_actual_count() {
        let cache = make_cache();

        assert_eq!(cache.len().unwrap(), 0);

        for i in 0..3u8 {
            cache.save(&[i], b"v", 999, now(0)).unwrap();
        }
        assert_eq!(cache.len().unwrap(), 3);

        cache.remove(&[0], now(0)).unwrap();
        assert_eq!(cache.len().unwrap(), 2);
    }
}
