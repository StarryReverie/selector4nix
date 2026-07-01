use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use redb::Database;
use redb::backends::InMemoryBackend;
use selector4nix::api::AppContext;
use selector4nix::api::handlers::status::get_status;
use selector4nix::application::nar_file::actor::{NarFileActor, NarFileActorRegistry};
use selector4nix::application::nar_file::usecase::NarFileStreamingUseCase;
use selector4nix::application::nar_info::actor::{NarInfoActor, NarInfoActorRegistry};
use selector4nix::application::nar_info::usecase::NarInfoResolutionUseCase;
use selector4nix::application::status::usecase::{
    CacheMode, StatusQueryUseCase, StatusRuntimeInfo,
};
use selector4nix::application::substituter::actor::{SubstituterActor, SubstituterActorRegistry};
use selector4nix::application::substituter::usecase::SubstituterQueryUseCase;
use selector4nix::domain::common::url::Url;
use selector4nix::domain::substituter::SubstituterRepository;
use selector4nix::infrastructure::config::AppConfiguration;
use selector4nix::infrastructure::repository::{
    CacheKvNarFileRepository, CacheKvNarInfoRepository, InMemorySubstituterRepository,
};
use selector4nix_actor::actor::Address;
use selector4nix_actor::registry::{AsyncFactory, RegistryBuilder};
use selector4nix_db::cache_kv::CacheKv;

use crate::fixture::config::make_config_string_minimal;
use crate::fixture::substituter::{make_substituter_maybe_ready, make_substituter_normal};

fn nar_info_registry() -> Arc<NarInfoActorRegistry> {
    Arc::new(
        RegistryBuilder::new()
            .factory(AsyncFactory::new(|_| async {
                Address::<NarInfoActor>::mock().0
            }))
            .build(),
    )
}

fn nar_file_registry() -> Arc<NarFileActorRegistry> {
    Arc::new(
        RegistryBuilder::new()
            .factory(AsyncFactory::new(|_| async {
                Address::<NarFileActor>::mock().0
            }))
            .build(),
    )
}

fn substituter_registry() -> Arc<SubstituterActorRegistry> {
    Arc::new(
        RegistryBuilder::new()
            .factory(AsyncFactory::new(|_| async {
                Address::<SubstituterActor>::mock().0
            }))
            .build(),
    )
}

#[tokio::test]
async fn status_endpoint_returns_runtime_config_and_substituters() {
    let config = Arc::new(AppConfiguration::deserialize(&make_config_string_minimal()).unwrap());
    let cache_database = Arc::new(
        Database::builder()
            .create_with_backend(InMemoryBackend::new())
            .unwrap(),
    );
    let nar_info_repository = Arc::new(CacheKvNarInfoRepository::new(Arc::new(CacheKv::new(
        cache_database.clone(),
        "nar_info".into(),
    ))));
    let nar_file_repository = Arc::new(CacheKvNarFileRepository::new(Arc::new(CacheKv::new(
        cache_database,
        "nar_file".into(),
    ))));

    let substituter_repository = Arc::new(InMemorySubstituterRepository::new());
    let cache_url = Url::new("https://cache.nixos.org/").unwrap();
    let private_url = Url::new("https://private.example.com/cache/").unwrap();
    substituter_repository
        .save(make_substituter_normal(&cache_url, 40))
        .await;
    substituter_repository
        .save(make_substituter_maybe_ready(&private_url, 10))
        .await;

    let nar_info_registry = nar_info_registry();
    let nar_file_registry = nar_file_registry();
    let status_query_usecase = StatusQueryUseCase::new(
        substituter_repository.clone(),
        Arc::new(StatusRuntimeInfo {
            version: "0.0.0-test",
            cache_mode: CacheMode::InMemory,
            config: config.clone(),
            authenticated_substituter_urls: [private_url].into_iter().collect(),
        }),
        nar_info_registry.clone(),
        nar_file_registry.clone(),
        nar_info_repository,
        nar_file_repository,
    );

    let ctx = AppContext::new(
        SubstituterQueryUseCase::new(substituter_repository),
        NarInfoResolutionUseCase::new(
            nar_info_registry,
            substituter_registry(),
            nar_file_registry.clone(),
        ),
        NarFileStreamingUseCase::new(nar_file_registry),
        status_query_usecase,
        config.cache_info.clone(),
    );

    let Json(response) = get_status(State(ctx)).await;
    let response = serde_json::to_value(response).unwrap();

    assert_eq!(response["version"], "0.0.0-test");
    assert_eq!(response["cache_mode"], "in_memory");
    assert_eq!(response["network"]["periodic_probing"], true);
    assert_eq!(response["network"]["tolerance_msecs"], 50);
    assert_eq!(response["network"]["nar_info_timeout_secs"], 30);
    assert_eq!(response["network"]["nar_timeout_secs"], 30);
    assert_eq!(response["network"]["max_concurrent_requests"], 12);
    assert_eq!(response["network"]["ignore_nar_info_error"], false);
    assert_eq!(response["proxy"]["rewrite_to_target"], "self");

    assert_eq!(response["substituters"]["total"], 2);
    assert_eq!(response["substituters"]["available"], 2);
    assert_eq!(
        response["substituters"]["items"][0]["url"],
        "https://private.example.com/cache/"
    );
    assert_eq!(response["substituters"]["items"][0]["priority"], 10);
    assert_eq!(
        response["substituters"]["items"][0]["status"],
        "maybe_ready"
    );
    assert_eq!(response["substituters"]["items"][0]["has_credential"], true);
    assert_eq!(
        response["substituters"]["items"][1]["url"],
        "https://cache.nixos.org/"
    );
    assert_eq!(response["substituters"]["items"][1]["priority"], 40);
    assert_eq!(response["substituters"]["items"][1]["status"], "normal");
    assert_eq!(
        response["substituters"]["items"][1]["has_credential"],
        false
    );

    assert_eq!(response["cache_stats"]["nar_info_lookup"]["entries"], 0);
    assert_eq!(response["cache_stats"]["nar_info_lookup"]["capacity"], 4096);
    assert_eq!(
        response["cache_stats"]["nar_info_lookup"]["ttl_secs"],
        14400
    );
    assert_eq!(response["cache_stats"]["nar_location"]["entries"], 0);
    assert_eq!(response["cache_stats"]["nar_location"]["capacity"], 4096);
    assert_eq!(response["cache_stats"]["nar_location"]["ttl_secs"], 14400);
    assert_eq!(response["cache_stats"]["nar_info_store"]["entries"], 0);
    assert_eq!(response["cache_stats"]["nar_file_store"]["entries"], 0);
}
