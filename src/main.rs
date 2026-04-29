use std::collections::HashMap;
use std::sync::Arc;

use reqwest::Client;
use tokio::net::TcpListener;

use selector4nix::api::{AppContext, build_router};
use selector4nix::domain::substituter::actor::SubstituterActor;
use selector4nix::domain::substituter::model::{Availability, Substituter, SubstituterMeta};
use selector4nix::infrastructure::config::*;
use selector4nix::infrastructure::index::*;
use selector4nix::infrastructure::registry::*;
use selector4nix::infrastructure::upstream::*;
use selector4nix::usecase::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = AppConfiguration::load().expect("could not load configuration");

    let substituters = config
        .substituters
        .iter()
        .map(|c| {
            let meta = SubstituterMeta::new(c.url.clone(), c.priority);
            Substituter::new(meta, Availability::Normal)
        })
        .collect::<Vec<_>>();

    let (availability_index_pre, availability_index_view) =
        SubstituterAvailabilityIndexActor::new(substituters.clone());
    let availability_publisher = availability_index_pre.address().erased();
    availability_index_pre.run();

    let (nar_file_index_pre, nar_file_index_view) =
        NarFileIndexActor::new(config.cache.nar_location_capacity as u64);
    let nar_file_index_pub = nar_file_index_pre.address().erased();
    nar_file_index_pre.run();

    let mut senders = HashMap::new();
    for sub in &substituters {
        let actor = SubstituterActor::new(sub.clone(), availability_publisher.clone());
        senders.insert(sub.url().clone(), actor.run());
    }
    let substituter_registry = Arc::new(SubstituterActorRegistry::new(senders));

    let nar_registry = Arc::new(NarActorRegistry::new(
        config.cache.nar_info_lookup_capacity as u64,
        config.cache.nar_info_lookup_ttl,
    ));

    let http_client = Client::new();
    let nar_info_provider = Arc::new(ReqwestNarInfoProvider::new(
        http_client.clone(),
        config.network.nar_info_timeout,
    ));
    let nar_stream_provider = Arc::new(ReqwestNarStreamProvider::new(
        http_client,
        config.network.nar_timeout,
    ));

    let substituter_usecase = SubstituterUseCase::new(Arc::new(availability_index_view.clone()));

    let nar_usecase = NarUseCase::new(
        nar_registry,
        substituter_registry,
        Arc::new(availability_index_view),
        nar_info_provider,
        nar_stream_provider,
        Arc::new(nar_file_index_view),
        nar_file_index_pub,
    );

    let ctx = AppContext::new(substituter_usecase, nar_usecase, config.cache_info);

    let app = build_router(ctx);
    let listen_addr = config.server.listen_addr();
    let listener = TcpListener::bind(listen_addr).await.unwrap();

    tracing::info!("listening on {listen_addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.unwrap();
}
