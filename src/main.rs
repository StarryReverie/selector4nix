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
use selector4nix::infrastructure::index::substituter_availability::SubstituterAvailabilityIndexActor;
use selector4nix::infrastructure::registry::{NarActorRegistry, SubstituterActorRegistry};
use selector4nix::infrastructure::upstream::nar_info::ReqwestNarInfoProvider;
use selector4nix::usecase::nar::NarUseCase;
use selector4nix::usecase::substituter::SubstituterUseCase;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let substituters = vec![SubstituterMeta::new(
        Url::new("https://cache.nixos.org").unwrap(),
        Priority::new(40).unwrap(),
    )];

    let (index_pre, index_view) = SubstituterAvailabilityIndexActor::new(substituters.clone());
    let publisher = index_pre.address().erased();
    index_pre.run(substituters.clone());

    let mut senders = HashMap::new();
    for meta in &substituters {
        let actor = SubstituterActor::new(publisher.clone());
        let substituter = Substituter::new(meta.clone(), Availability::Normal);
        senders.insert(meta.url().clone(), actor.run(substituter));
    }

    let substituter_registry = Arc::new(SubstituterActorRegistry::new(senders));
    let nar_info_provider = Arc::new(ReqwestNarInfoProvider::new(Client::new()));
    let nar_registry = Arc::new(NarActorRegistry::new(1000, Duration::from_secs(300)));

    let substituter_usecase = SubstituterUseCase::new(Arc::new(index_view.clone()));

    let nar_usecase = NarUseCase::new(
        nar_registry,
        substituter_registry,
        Arc::new(index_view),
        nar_info_provider,
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
