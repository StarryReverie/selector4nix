use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use tokio::net::TcpListener;

use selector4nix::api::{AppContext, build_router};
use selector4nix::domain::substituter::actor::SubstituterActor;
use selector4nix::domain::substituter::model::{
    Availability, Priority, Substituter, SubstituterMeta, Url,
};
use selector4nix::infrastructure::index::*;
use selector4nix::infrastructure::registry::*;
use selector4nix::infrastructure::upstream::*;
use selector4nix::usecase::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let substituters = vec![
        SubstituterMeta::new(
            Url::new("https://cache.nixos.org/").unwrap(),
            Priority::new(40).unwrap(),
        ),
        SubstituterMeta::new(
            Url::new("https://mirrors.tuna.tsinghua.edu.cn/nix-channels/store/").unwrap(),
            Priority::new(40).unwrap(),
        ),
        SubstituterMeta::new(
            Url::new("https://mirrors.ustc.edu.cn/nix-channels/store/").unwrap(),
            Priority::new(40).unwrap(),
        ),
        SubstituterMeta::new(
            Url::new("https://mirror.sjtu.edu.cn/nix-channels/store/").unwrap(),
            Priority::new(40).unwrap(),
        ),
    ];

    let (availability_index_pre, availability_index_view) =
        SubstituterAvailabilityIndexActor::new(substituters.clone());
    let availability_publisher = availability_index_pre.address().erased();
    availability_index_pre.run();

    let (nar_file_index_pre, nar_file_index_view) = NarFileIndexActor::new(10000);
    let nar_file_index_pub = nar_file_index_pre.address().erased();
    nar_file_index_pre.run();

    let mut senders = HashMap::new();
    for meta in &substituters {
        let substituter = Substituter::new(meta.clone(), Availability::Normal);
        let actor = SubstituterActor::new(substituter, availability_publisher.clone());
        senders.insert(meta.url().clone(), actor.run());
    }
    let substituter_registry = Arc::new(SubstituterActorRegistry::new(senders));

    let nar_registry = Arc::new(NarActorRegistry::new(1000, Duration::from_secs(300)));

    let http_client = Client::new();
    let nar_info_provider = Arc::new(ReqwestNarInfoProvider::new(http_client.clone()));
    let nar_stream_provider = Arc::new(ReqwestNarStreamProvider::new(http_client));

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

    let ctx = AppContext::new(substituter_usecase, nar_usecase);

    let app = build_router(ctx);
    let listener = TcpListener::bind("0.0.0.0:5496").await.unwrap();

    tracing::info!("listening on 0.0.0.0:5496");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.unwrap();
}
