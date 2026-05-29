mod bootstrap;
mod cli;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use clap::Parser;
use tokio::net::TcpListener;

use selector4nix::api::{AppContext, build_router};
use selector4nix::infrastructure::config::{AppConfiguration, AppCredential};

use crate::cli::{Cli, Commands};

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    let cli = Cli::parse();
    bootstrap::init_logger(cli.log_file, cli.log_level, cli.no_log_timestamp)?;

    let config = if let Some(path) = cli.config_file {
        AppConfiguration::load_from(&path)?
    } else {
        AppConfiguration::load()?
    };

    let credentials = if let Some(path) = cli.credential_file {
        let credential = AppCredential::load_from(&path)?;
        Arc::new(credential)
    } else {
        let credential = AppCredential::load()
            .transpose()?
            .unwrap_or(AppCredential::empty());
        Arc::new(credential)
    };

    let cache_dir = if let Some(path) = cli.cache_dir {
        tracing::info!(path = %path.display(), "use cache directory from CLI options");
        Some(path)
    } else if let Some(path) = std::env::var_os("SELECTOR4NIX_CACHE_DIR") {
        tracing::info!(path = %path.display(), "use cache directory from environment variable");
        Some(PathBuf::from(path))
    } else {
        tracing::warn!("no cache directory is specified, use in-memory cache database");
        None
    };

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => {
            let context = bootstrap::init_context(&config, credentials, cache_dir).await?;
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
    tracing::info!("visit http://{listen_addr}/ for the welcome page");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.unwrap();
}
