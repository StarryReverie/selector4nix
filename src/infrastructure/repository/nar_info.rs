use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use selector4nix_db::cache_kv::{CacheKv, UnixTimestampArg};

use crate::domain::nar_info::NarInfoRepository;
use crate::domain::nar_info::model::{NarInfo, StorePathHash};

pub struct CacheKvNarInfoRepository {
    db: Arc<CacheKv>,
}

impl CacheKvNarInfoRepository {
    pub fn new(db: Arc<CacheKv>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl NarInfoRepository for CacheKvNarInfoRepository {
    async fn get(&self, hash: &StorePathHash) -> AnyhowResult<Option<NarInfo>> {
        let db = Arc::clone(&self.db);
        let key = postcard::to_stdvec(hash).expect("serialize `hash` to bytes should not fail");

        tokio::task::spawn_blocking(move || {
            let value = db.get(&key, UnixTimestampArg::SystemNow)?;
            let entity = value.and_then(|(_, value)| {
                postcard::from_bytes::<NarInfo>(&value)
                    .inspect_err(|_| tracing::warn!("encountered data of invalid schema, ignore"))
                    .ok()
            });
            Ok(entity)
        })
        .await?
    }

    async fn save(&self, nar_info: NarInfo) -> AnyhowResult<()> {
        let Some(expire_at) = nar_info.expire_at() else {
            return Ok(());
        };

        let db = Arc::clone(&self.db);
        let key = postcard::to_stdvec(nar_info.hash())
            .expect("serialize `nar_info.hash()` to bytes should not fail");
        let value =
            postcard::to_stdvec(&nar_info).expect("serialize `nar_info` to bytes should not fail");

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
