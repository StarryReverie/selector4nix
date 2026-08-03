use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result as AnyhowResult};
use redb::Database;
use redb::backends::InMemoryBackend;
use reqwest::{Client, ClientBuilder};
use selector4nix_actor::actor::Address;
use selector4nix_actor::registry::{
    AsyncFactory, CapacityOption, ExpirationOption, RegistryBuilder,
};
use selector4nix_core::AppContext;
use selector4nix_core::application::actor::nar_file::NarFileActor;
use selector4nix_core::application::actor::nar_info::NarInfoActor;
use selector4nix_core::application::actor::substituter::SubstituterActor;
use selector4nix_core::application::usecase::dashboard::{
    GetDashboardCacheStatsUseCase, GetDashboardConfigSummaryUseCase,
};
use selector4nix_core::application::usecase::dashboard::{
    GetDashboardOverviewUseCase, GetDashboardTransferringUseCase,
};
use selector4nix_core::application::usecase::nar_file::StreamNarFileUseCase;
use selector4nix_core::application::usecase::nar_info::ResolveNarInfoUseCase;
use selector4nix_core::domain::common::passthrough_headers::SELF_USER_AGENT;
use selector4nix_core::domain::nar_file::NarFileService;
use selector4nix_core::domain::nar_file::model::NarFileKey;
use selector4nix_core::domain::nar_info::NarInfoService;
use selector4nix_core::domain::nar_info::model::StorePathHash;
use selector4nix_core::domain::substituter::model::{Availability, Substituter, SubstituterMeta};
use selector4nix_core::domain::substituter::{SubstituterRepository, SubstituterService};
use selector4nix_core::infrastructure::config::{AppConfiguration, AppCredential};
use selector4nix_core::infrastructure::metric::NarTransferMetric;
use selector4nix_core::infrastructure::provider::*;
use selector4nix_core::infrastructure::repository::*;
use selector4nix_db::cache_kv::CacheKv;
use selector4nix_streaming::StreamingClient;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

use crate::cli::LogLevel;

pub fn init_logger(
    log_file: Option<PathBuf>,
    log_level: Option<LogLevel>,
    no_timestamp: bool,
) -> AnyhowResult<()> {
    let registry = tracing_subscriber::registry();

    let writer = if let Some(file) = log_file {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .with_context(|| format!("could not open log file: {}", file.display()))?;
        Some(Arc::new(file))
    } else {
        None
    };

    let filter = if let Some(level) = log_level {
        EnvFilter::new(level.to_string())
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    let registry = {
        let fmt_layer: Box<dyn Layer<Registry> + Send + Sync> = match (no_timestamp, writer) {
            (true, Some(writer)) => Box::new(
                tracing_subscriber::fmt::layer()
                    .without_time()
                    .with_writer(writer)
                    .with_ansi(false)
                    .with_filter(filter.clone()),
            ),
            (false, Some(writer)) => Box::new(
                tracing_subscriber::fmt::layer()
                    .with_writer(writer)
                    .with_ansi(false)
                    .with_filter(filter.clone()),
            ),
            (true, None) => Box::new(
                tracing_subscriber::fmt::layer()
                    .without_time()
                    .with_filter(filter.clone()),
            ),
            (false, None) => Box::new(tracing_subscriber::fmt::layer().with_filter(filter.clone())),
        };
        registry.with(fmt_layer)
    };

    #[cfg(all(target_vendor = "apple", not(debug_assertions)))]
    let registry = {
        use tracing_oslog::OsLogger;
        let oslog_layer =
            OsLogger::new("cc.starryreverie.selector4nix", "default").with_filter(filter);
        registry.with(oslog_layer)
    };

    registry.init();
    Ok(())
}

pub async fn init_context(
    config: &Arc<AppConfiguration>,
    credentials: Arc<AppCredential>,
    cache_dir: Option<PathBuf>,
) -> AnyhowResult<Arc<AppContext>> {
    let has_persistent_cache = cache_dir.is_some();

    let database = match cache_dir {
        Some(cache_dir) => {
            if !cache_dir.is_dir() {
                return Err(anyhow::anyhow!(
                    "could not use `{}` as a cache directory",
                    cache_dir.display(),
                ));
            }
            let database_path = cache_dir.join(Path::new("main.redb"));
            let database = Database::builder().create(database_path)?;
            Arc::new(database)
        }
        None => {
            let database = Database::builder().create_with_backend(InMemoryBackend::new())?;
            Arc::new(database)
        }
    };

    let http_client = http_client_builder_factory(config)
        .build()
        .context("could not build HTTP client")?;

    let streaming_http_client = Arc::new(StreamingClient::new(
        http_client_builder_factory(config),
        config.network.max_concurrent_requests,
        config.network.chunked_streaming,
        config.network.streaming_chunk_max_len,
        config.network.streaming_window_max_len,
    ));

    let substituter_probing_provider = Arc::new(ReqwestSubstituterProbingProvider::new(
        http_client.clone(),
        config.network.nar_info_timeout,
        credentials.clone(),
    ));

    let nar_info_provider = Arc::new(ReqwestNarInfoProvider::new(
        http_client.clone(),
        config.network.nar_info_timeout,
        credentials.clone(),
    ));

    let nar_stream_provider = Arc::new(ReqwestNarStreamProvider::new(
        streaming_http_client,
        credentials.clone(),
    ));

    let nar_transfer_metric = Arc::new(NarTransferMetric::new());

    let substituters = config
        .substituters
        .iter()
        .map(|sub_config| {
            let meta = SubstituterMeta::new(sub_config.url.clone(), sub_config.priority)
                .with_nar_info_timeout(sub_config.nar_info_timeout)
                .with_nar_timeout(sub_config.nar_timeout);
            let meta = match sub_config.storage_url.clone() {
                Some(storage_url) => meta.with_storage_url(storage_url),
                None => meta,
            };
            Substituter::new(meta, Availability::Normal)
        })
        .collect::<Vec<_>>();

    let substituter_repository = Arc::new({
        let substituter_repository = InMemorySubstituterRepository::new();
        for sub in &substituters {
            substituter_repository.save(sub.clone()).await;
        }
        substituter_repository
    });

    let nar_info_repository = {
        let cache_kv = Arc::new(CacheKv::new(database.clone(), "nar_info".into()));
        cache_kv.spawn_cleanup_task();
        Arc::new(CacheKvNarInfoRepository::new(cache_kv))
    };

    let nar_file_repository = {
        let cache_kv = Arc::new(CacheKv::new(database, "nar_file".into()));
        cache_kv.spawn_cleanup_task();
        Arc::new(CacheKvNarFileRepository::new(cache_kv))
    };

    let substituter_service = Arc::new(SubstituterService::new(config.network.periodic_probing));

    let nar_info_service = Arc::new(NarInfoService::new(
        nar_info_provider,
        substituter_repository.clone(),
        config.proxy.rewrite_nar_url,
        config.network.tolerance,
        config.network.ignore_nar_info_error,
    ));

    let nar_file_service = Arc::new(NarFileService::new(
        nar_stream_provider,
        substituter_repository.clone(),
        config.cache.nar_file_ttl,
    ));

    let substituter_registry = Arc::new({
        let registry = RegistryBuilder::new()
            .factory(AsyncFactory::new(|_| async {
                // No additional actor will be loaded here as those actors are created eagerly
                Address::mock().0
            }))
            .build();
        for sub in &substituters {
            let substituter_service = substituter_service.clone();
            let sub_probing_provider = substituter_probing_provider.clone();
            let repo = substituter_repository.clone();
            let addr = SubstituterActor::new(
                Some(sub.clone()),
                substituter_service,
                sub_probing_provider,
                repo,
            )
            .run();
            registry.insert(sub.url().clone(), addr).await;
        }
        registry
    });

    let nar_info_registry = Arc::new(
        RegistryBuilder::new()
            .capacity(CapacityOption::Lru(config.cache.nar_info_cache_capacity))
            .expiration(ExpirationOption::Ttl(config.cache.nar_info_ttl))
            .factory(AsyncFactory::new({
                let nar_info_service = nar_info_service.clone();
                let nar_info_repository = nar_info_repository.clone();
                let nar_info_ttl = config.cache.nar_info_ttl;
                move |hash: &StorePathHash| {
                    let addr = NarInfoActor::new(
                        hash.clone(),
                        nar_info_service.clone(),
                        nar_info_repository.clone(),
                        nar_info_ttl,
                    )
                    .run();
                    async move { addr }
                }
            }))
            .build(),
    );

    let nar_file_registry = Arc::new(
        RegistryBuilder::new()
            .capacity(CapacityOption::Lru(config.cache.nar_file_cache_capacity))
            .expiration(ExpirationOption::Ttl(config.cache.nar_file_ttl))
            .factory(AsyncFactory::new({
                let nar_file_servicee = nar_file_service.clone();
                let nar_file_repository = nar_file_repository.clone();
                let nar_file_ttl = config.cache.nar_file_ttl;
                move |key: &NarFileKey| {
                    let addr = NarFileActor::new(
                        key.clone(),
                        nar_file_servicee.clone(),
                        nar_file_repository.clone(),
                        nar_file_ttl,
                    )
                    .run();
                    async move { addr }
                }
            }))
            .build(),
    );

    let stream_nar_file_usecase = StreamNarFileUseCase::new(
        nar_file_registry.clone(),
        nar_info_repository.clone(),
        nar_transfer_metric.clone(),
    );

    let resolve_nar_info_usecase = ResolveNarInfoUseCase::new(
        nar_info_registry.clone(),
        substituter_registry,
        nar_file_registry.clone(),
    );

    let get_dashboard_overview_usecase = GetDashboardOverviewUseCase::new(
        substituter_repository,
        nar_info_registry.clone(),
        nar_transfer_metric.clone(),
        credentials,
        config.cache.nar_info_cache_capacity,
        has_persistent_cache,
    );

    let get_dashboard_transferring_usecase =
        GetDashboardTransferringUseCase::new(nar_transfer_metric);

    let get_dashboard_cache_stats_usecase = GetDashboardCacheStatsUseCase::new(
        nar_info_registry,
        nar_file_registry,
        nar_info_repository,
        nar_file_repository,
        config.cache.clone(),
        has_persistent_cache,
    );

    let get_dashboard_config_summary_usecase =
        GetDashboardConfigSummaryUseCase::new(Arc::clone(&config));

    Ok(Arc::new(AppContext {
        resolve_nar_info_usecase,
        stream_nar_file_usecase,
        get_dashboard_overview_usecase,
        get_dashboard_transferring_usecase,
        get_dashboard_cache_stats_usecase,
        get_dashboard_config_summary_usecase,
        cache_info: config.cache_info.clone(),
    }))
}

fn http_client_builder_factory(config: &AppConfiguration) -> ClientBuilder {
    Client::builder()
        .user_agent(SELF_USER_AGENT.as_str())
        .connect_timeout(config.network.nar_timeout)
}
