use std::sync::Arc;

use serde::Serialize;

use crate::application::actor::nar_info::NarInfoActorRegistry;
use crate::domain::common::url::Url;
use crate::domain::substituter::SubstituterRepository;
use crate::domain::substituter::model::{Availability, Priority};
use crate::infrastructure::config::AppCredential;
use crate::infrastructure::metric::NarTransferMetric;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct OverviewData {
    summary: OverviewSummaryData,
    substituters: Vec<OverviewSubstituterItemData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct OverviewSummaryData {
    available_substituters: usize,
    total_substituters: usize,
    transferring_nar_files: usize,
    nar_info_cache_size: usize,
    nar_info_cache_capacity: usize,
    cache_mode: CacheMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CacheMode {
    Persistent,
    InMemory,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct OverviewSubstituterItemData {
    url: Url,
    storage_url: Url,
    priority: Priority,
    has_credential: bool,
    status: SubstituterStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum SubstituterStatus {
    Normal,
    Offline,
    ServiceError,
    MaybeReady,
}

pub struct GetDashboardOverviewUseCase {
    substituter_repository: Arc<dyn SubstituterRepository>,
    nar_info_registry: Arc<NarInfoActorRegistry>,
    nar_transfer_metric: Arc<NarTransferMetric>,
    credentials: Arc<AppCredential>,
    nar_info_cache_capacity: usize,
    cache_mode: CacheMode,
}

impl GetDashboardOverviewUseCase {
    pub fn new(
        substituter_repository: Arc<dyn SubstituterRepository>,
        nar_info_registry: Arc<NarInfoActorRegistry>,
        nar_transfer_metric: Arc<NarTransferMetric>,
        credentials: Arc<AppCredential>,
        nar_info_cache_capacity: usize,
        has_persistent_cache: bool,
    ) -> Self {
        Self {
            substituter_repository,
            nar_info_registry,
            nar_transfer_metric,
            credentials,
            nar_info_cache_capacity,
            cache_mode: if has_persistent_cache {
                CacheMode::Persistent
            } else {
                CacheMode::InMemory
            },
        }
    }

    pub async fn run(&self) -> OverviewData {
        let substituters = self.substituter_repository.query_all().await;

        OverviewData {
            summary: OverviewSummaryData {
                available_substituters: substituters.iter().filter(|s| !s.is_unavailable()).count(),
                total_substituters: substituters.len(),
                transferring_nar_files: self.nar_transfer_metric.transferring_count(),
                nar_info_cache_size: self.nar_info_registry.entry_count().await,
                nar_info_cache_capacity: self.nar_info_cache_capacity,
                cache_mode: self.cache_mode,
            },
            substituters: substituters
                .iter()
                .map(|s| OverviewSubstituterItemData {
                    url: s.url().clone(),
                    storage_url: s.target().storage_url().clone(),
                    priority: s.priority(),
                    has_credential: self.credentials.lookup(s.url()).is_some(),
                    status: match s.availability() {
                        Availability::Normal => SubstituterStatus::Normal,
                        Availability::Offline { .. } => SubstituterStatus::Offline,
                        Availability::ServiceError { .. } => SubstituterStatus::ServiceError,
                        Availability::MaybeReady { .. } => SubstituterStatus::MaybeReady,
                    },
                })
                .collect(),
        }
    }
}
