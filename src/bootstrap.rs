use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result as AnyhowResult};
use reqwest::Client;
use selector4nix_actor::registry::{
    AsyncFactory, CapacityOption, ExpirationOption, RegistryBuilder,
};
use tokio::sync::Semaphore;
use tracing_subscriber::EnvFilter;

use selector4nix::api::AppContext;
use selector4nix::application::nar::actor::NarActor;
use selector4nix::application::nar::usecase::{NarResolutionUseCase, NarStreamingUseCase};
use selector4nix::application::substituter::actor::SubstituterActor;
use selector4nix::application::substituter::usecase::SubstituterQueryUseCase;
use selector4nix::domain::nar::model::{Nar, StorePathHash};
use selector4nix::domain::nar::service::NarResolutionService;
use selector4nix::domain::substituter::model::{Availability, Substituter, SubstituterMeta};
use selector4nix::domain::substituter::service::SubstituterLifecycleService;
use selector4nix::infrastructure::config::AppConfiguration;
use selector4nix::infrastructure::index::*;
use selector4nix::infrastructure::provider::*;

use crate::cli::LogLevel;

pub fn init_logger(log_level: Option<LogLevel>) {
    let filter = if let Some(level) = log_level {
        EnvFilter::new(level.to_string())
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

pub fn init_context(config: &AppConfiguration) -> AnyhowResult<Arc<AppContext>> {
    let http_client = Client::builder()
        .user_agent(format!(
            "curl/8.7.1 Nix/2.24.11 {}/{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))
        .connect_timeout(config.network.nar_timeout)
        .build()
        .context("could not build HTTP client")?;

    let concurrency = Arc::new(Semaphore::new(config.network.max_concurrent_requests));

    let nar_info_provider = Arc::new(ReqwestNarInfoProvider::new(
        http_client.clone(),
        config.network.nar_info_timeout,
        concurrency.clone(),
    ));

    let nar_stream_provider = Arc::new(ReqwestNarStreamProvider::new(http_client, concurrency));

    let substituters = config
        .substituters
        .iter()
        .map(|c| {
            let meta = SubstituterMeta::new(c.url.clone(), c.priority);
            let meta = match c.storage_url.clone() {
                Some(storage_url) => meta.with_storage_url(storage_url),
                None => meta,
            };
            Substituter::new(meta, Availability::Normal)
        })
        .collect::<Vec<_>>();

    let (substituter_availability_index_pre, substituter_availability_index_view) =
        SubstituterAvailabilityIndexActor::new(substituters.clone());
    let substituter_availability_pub = substituter_availability_index_pre.address().erased();
    substituter_availability_index_pre.run();
    let substituter_availability_index = Arc::new(substituter_availability_index_view);

    let (nar_file_index_pre, nar_file_index_view) =
        NarFileIndexActor::new(config.cache.nar_location_capacity as u64);
    let nar_file_index_pub = nar_file_index_pre.address().erased();
    nar_file_index_pre.run();
    let nar_file_index = Arc::new(nar_file_index_view);

    let substituter_lifecycle_service = Arc::new(SubstituterLifecycleService::new());

    let nar_info_query_service = Arc::new(NarResolutionService::new(
        nar_info_provider,
        substituter_availability_index.clone(),
        config.proxy.rewrite_nar_url,
        config.network.tolerance,
    ));

    let substituter_registry = Arc::new(
        RegistryBuilder::new()
            .factory(AsyncFactory::new({
                let sub_map = substituters
                    .iter()
                    .map(|s| (s.url().clone(), s.clone()))
                    .collect::<HashMap<_, _>>();
                let avail_pub = substituter_availability_pub.clone();
                let lifecycle_service = substituter_lifecycle_service.clone();
                move |url| {
                    let substituter = sub_map.get(url).cloned();
                    let avail_pub = avail_pub.clone();
                    let lifecycle_service = lifecycle_service.clone();
                    let addr =
                        SubstituterActor::new(substituter, lifecycle_service, avail_pub).run();
                    async move { addr }
                }
            }))
            .build(),
    );

    let nar_registry = Arc::new(
        RegistryBuilder::new()
            .capacity(CapacityOption::Lru(config.cache.nar_info_lookup_capacity))
            .expiration(ExpirationOption::Ttl(config.cache.nar_info_lookup_ttl))
            .factory(AsyncFactory::new({
                let nar_info_query_service = nar_info_query_service.clone();
                let nar_file_index_pub = nar_file_index_pub.clone();
                move |hash: &StorePathHash| {
                    let addr = NarActor::new(
                        Nar::new(hash.clone()),
                        nar_info_query_service.clone(),
                        nar_file_index_pub.clone(),
                    )
                    .run();
                    async move { addr }
                }
            }))
            .build(),
    );

    let substituter_query_usecase =
        SubstituterQueryUseCase::new(substituter_availability_index.clone());

    let nar_resolution_usecase =
        NarResolutionUseCase::new(nar_registry.clone(), substituter_registry);

    let nar_streaming_usecase = NarStreamingUseCase::new(
        substituter_availability_index,
        nar_stream_provider,
        nar_file_index,
        nar_file_index_pub,
    );

    Ok(AppContext::new(
        substituter_query_usecase,
        nar_resolution_usecase,
        nar_streaming_usecase,
        config.cache_info.clone(),
    ))
}
