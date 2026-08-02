use std::sync::Arc;

use serde::Serialize;

use crate::application::actor::nar_file::NarFileActorRegistry;
use crate::application::actor::nar_info::NarInfoActorRegistry;
use crate::domain::nar_file::NarFileRepository;
use crate::domain::nar_info::NarInfoRepository;
use crate::infrastructure::config::CacheConfiguration;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CacheStatsData {
    pub cache: CacheStatsCacheData,
    pub store: CacheStatsStoreData,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CacheStatsCacheData {
    pub nar_info: CacheTypeStats,
    pub nar_file: CacheTypeStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CacheStatsStoreData {
    pub nar_info: StoreTypeStats,
    pub nar_file: StoreTypeStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CacheTypeStats {
    pub size: usize,
    pub capacity: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct StoreTypeStats {
    pub size: Option<usize>,
    pub ttl_secs: u64,
    pub cache_mode: CacheMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CacheMode {
    Persistent,
    InMemory,
}

pub struct GetDashboardCacheStatsUseCase {
    nar_info_registry: Arc<NarInfoActorRegistry>,
    nar_file_registry: Arc<NarFileActorRegistry>,
    nar_info_repository: Arc<dyn NarInfoRepository>,
    nar_file_repository: Arc<dyn NarFileRepository>,
    cache_config: CacheConfiguration,
    cache_mode: CacheMode,
}

impl GetDashboardCacheStatsUseCase {
    pub fn new(
        nar_info_registry: Arc<NarInfoActorRegistry>,
        nar_file_registry: Arc<NarFileActorRegistry>,
        nar_info_repository: Arc<dyn NarInfoRepository>,
        nar_file_repository: Arc<dyn NarFileRepository>,
        cache_config: CacheConfiguration,
        has_persistent_cache: bool,
    ) -> Self {
        Self {
            nar_info_registry,
            nar_file_registry,
            nar_info_repository,
            nar_file_repository,
            cache_config,
            cache_mode: if has_persistent_cache {
                CacheMode::Persistent
            } else {
                CacheMode::InMemory
            },
        }
    }

    pub async fn run(&self) -> CacheStatsData {
        CacheStatsData {
            cache: CacheStatsCacheData {
                nar_info: CacheTypeStats {
                    size: self.nar_info_registry.entry_count().await,
                    capacity: self.cache_config.nar_info_lookup_capacity,
                },
                nar_file: CacheTypeStats {
                    size: self.nar_file_registry.entry_count().await,
                    capacity: self.cache_config.nar_location_capacity,
                },
            },
            store: CacheStatsStoreData {
                nar_info: StoreTypeStats {
                    size: self.nar_info_repository.entry_count().await.ok(),
                    ttl_secs: self.cache_config.nar_info_lookup_ttl.as_secs(),
                    cache_mode: self.cache_mode,
                },
                nar_file: StoreTypeStats {
                    size: self.nar_file_repository.entry_count().await.ok(),
                    ttl_secs: self.cache_config.nar_location_ttl.as_secs(),
                    cache_mode: self.cache_mode,
                },
            },
        }
    }
}
