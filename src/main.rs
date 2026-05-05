mod bootstrap;
mod cli;

use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use clap::Parser;
use tokio::net::TcpListener;

use selector4nix::api::{AppContext, build_router};
use selector4nix::infrastructure::config::AppConfiguration;

use crate::cli::{Cli, Commands};

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    let cli = Cli::parse();
    bootstrap::init_logger(cli.log_level);

    let config = if let Some(path) = &cli.config_file {
        AppConfiguration::load_from(path)?
    } else {
        AppConfiguration::load()?
    };

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => {
            let context = bootstrap::init_context(&config)?;
            serve(config, context).await
        }
        Commands::Check => {
            eprintln!("Configuration OK");
            Ok(())
        }
    }
}

async fn serve(config: AppConfiguration, context: Arc<AppContext>) -> AnyhowResult<()> {
    let router = build_router(context);
    let listen_addr = config.server.listen_addr();
    let listener = TcpListener::bind(listen_addr).await?;

    tracing::info!("listening on {listen_addr}");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.unwrap();
}
