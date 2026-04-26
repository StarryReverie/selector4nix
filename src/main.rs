use tokio::net::TcpListener;

use selector4nix::api::router::build_router;
use selector4nix::api::state::AppState;
use selector4nix::domain::substituter::model::{Priority, SubstituterMeta, Url};
use selector4nix::infrastructure::index::substituter_availability::SubstituterAvailabilityIndexActor;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let substituters = vec![SubstituterMeta::new(
        Url::new("https://cache.nixos.org").unwrap(),
        Priority::new(40).unwrap(),
    )];

    let index_actor = SubstituterAvailabilityIndexActor::new();
    let index_view = index_actor.view();
    index_actor.run(substituters);

    let state = AppState { index_view };
    let app = build_router(state);
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
