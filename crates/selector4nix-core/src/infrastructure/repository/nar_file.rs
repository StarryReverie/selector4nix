use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use selector4nix_db::cache_kv::{CacheKv, UnixTimestampArg};

use crate::domain::nar_file::NarFileRepository;
use crate::domain::nar_file::model::{NarFile, NarFileKey};

pub struct CacheKvNarFileRepository {
    db: Arc<CacheKv>,
}

impl CacheKvNarFileRepository {
    pub fn new(db: Arc<CacheKv>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl NarFileRepository for CacheKvNarFileRepository {
    async fn get(&self, key: &NarFileKey) -> AnyhowResult<Option<NarFile>> {
        let db = Arc::clone(&self.db);
        let key = postcard::to_stdvec(key).expect("serialize `key` to bytes should not fail");

        tokio::task::spawn_blocking(move || {
            let value = db.get(&key, UnixTimestampArg::SystemNow)?;
            let entity = value.and_then(|(_, value)| {
                postcard::from_bytes::<NarFile>(&value)
                    .inspect_err(|_| tracing::warn!("encountered data of invalid schema, ignore"))
                    .ok()
            });
            Ok(entity)
        })
        .await?
    }

    async fn save(&self, nar_file: NarFile) -> AnyhowResult<()> {
        let Some(expire_at) = nar_file.expire_at() else {
            return Ok(());
        };

        let db = Arc::clone(&self.db);
        let key = postcard::to_stdvec(nar_file.key())
            .expect("serialize `nar_file.key()` to bytes should not fail");
        let value =
            postcard::to_stdvec(&nar_file).expect("serialize `nar_file` to bytes should not fail");

        tokio::task::spawn_blocking(move || {
            let expire_at = expire_at.unix_timpstamp();
            db.save(&key, &value, expire_at, UnixTimestampArg::SystemNow)
        })
        .await?
    }

    async fn entry_count(&self) -> AnyhowResult<usize> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || db.len()).await?
    }
}
