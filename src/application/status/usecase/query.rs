use std::collections::HashSet;
use std::sync::Arc;

use crate::application::nar_file::actor::NarFileActorRegistry;
use crate::application::nar_info::actor::NarInfoActorRegistry;
use crate::domain::common::url::Url;
use crate::domain::nar_file::NarFileRepository;
use crate::domain::nar_info::NarInfoRepository;
use crate::domain::substituter::SubstituterRepository;
use crate::domain::substituter::model::{Availability, Substituter};
use crate::infrastructure::config::AppConfiguration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheMode {
    Persistent,
    InMemory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRuntimeInfo {
    pub version: &'static str,
    pub cache_mode: CacheMode,
    pub config: Arc<AppConfiguration>,
    pub authenticated_substituter_urls: HashSet<Url>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub runtime: Arc<StatusRuntimeInfo>,
    pub substituters: Vec<Substituter>,
    pub nar_info_actor_entries: usize,
    pub nar_file_actor_entries: usize,
    pub nar_info_persistent_entries: usize,
    pub nar_file_persistent_entries: usize,
}

pub struct StatusQueryUseCase {
    substituter_repository: Arc<dyn SubstituterRepository>,
    runtime: Arc<StatusRuntimeInfo>,
    nar_info_registry: Arc<NarInfoActorRegistry>,
    nar_file_registry: Arc<NarFileActorRegistry>,
    nar_info_repository: Arc<dyn NarInfoRepository>,
    nar_file_repository: Arc<dyn NarFileRepository>,
}

impl StatusQueryUseCase {
    pub fn new(
        substituter_repository: Arc<dyn SubstituterRepository>,
        runtime: Arc<StatusRuntimeInfo>,
        nar_info_registry: Arc<NarInfoActorRegistry>,
        nar_file_registry: Arc<NarFileActorRegistry>,
        nar_info_repository: Arc<dyn NarInfoRepository>,
        nar_file_repository: Arc<dyn NarFileRepository>,
    ) -> Self {
        Self {
            substituter_repository,
            runtime,
            nar_info_registry,
            nar_file_registry,
            nar_info_repository,
            nar_file_repository,
        }
    }

    pub async fn snapshot(&self) -> StatusSnapshot {
        let substituters = self.substituter_repository.query_all().await;

        StatusSnapshot {
            runtime: self.runtime.clone(),
            substituters,
            nar_info_actor_entries: self
                .nar_info_registry
                .entry_count()
                .await
                .try_into()
                .unwrap_or(usize::MAX),
            nar_file_actor_entries: self
                .nar_file_registry
                .entry_count()
                .await
                .try_into()
                .unwrap_or(usize::MAX),
            nar_info_persistent_entries: self
                .nar_info_repository
                .entry_count()
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(%err, cache = "nar_info", "failed to get cache entry count");
                    0
                }),
            nar_file_persistent_entries: self
                .nar_file_repository
                .entry_count()
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(%err, cache = "nar_file", "failed to get cache entry count");
                    0
                }),
        }
    }
}

impl StatusSnapshot {
    pub fn available_substituter_count(&self) -> usize {
        self.substituters
            .iter()
            .filter(|sub| !sub.is_unavailable())
            .count()
    }
}

pub fn availability_status(availability: &Availability) -> &'static str {
    match availability {
        Availability::Normal => "normal",
        Availability::Offline { .. } => "offline",
        Availability::ServiceError { .. } => "service_error",
        Availability::MaybeReady { .. } => "maybe_ready",
    }
}
