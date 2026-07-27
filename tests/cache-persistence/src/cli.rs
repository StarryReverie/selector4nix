use std::path::PathBuf;

use anyhow::{Context, Result as AnyhowResult, bail};
use clap::Parser;

#[derive(Parser)]
#[command(name = "selector4nix-system-test-cache-persistence")]
struct Cli {
    #[arg(long = "selector4nix", env = "SELECTOR4NIX_BIN")]
    selector4nix: Option<PathBuf>,

    #[arg(long = "nix", env = "NIX_BIN")]
    nix: Option<PathBuf>,

    #[arg(long = "nix-serve", env = "NIX_SERVE_BIN")]
    nix_serve: Option<PathBuf>,

    #[arg(long = "count", default_value_t = 5)]
    count: usize,

    #[arg(long = "seed", default_value_t = 42)]
    seed: u64,
}

pub struct TestConfig {
    pub selector4nix_bin: PathBuf,
    pub nix_bin: PathBuf,
    pub nix_serve_bin: PathBuf,
    pub count: usize,
    pub seed: u64,
}

pub fn resolve() -> AnyhowResult<TestConfig> {
    let cli = Cli::parse();

    let selector4nix = resolve_binary(cli.selector4nix, "selector4nix")
        .context("failed to resolve `selector4nix` binary")?;
    let nix = resolve_binary(cli.nix, "nix").context("failed to resolve `nix` binary")?;
    let nix_serve = resolve_binary(cli.nix_serve, "nix-serve")
        .context("failed to resolve `nix-serve` binary")?;

    Ok(TestConfig {
        selector4nix_bin: selector4nix,
        nix_bin: nix,
        nix_serve_bin: nix_serve,
        count: cli.count,
        seed: cli.seed,
    })
}

fn resolve_binary(explicit: Option<PathBuf>, name: &str) -> AnyhowResult<PathBuf> {
    if let Some(path) = explicit {
        if path.exists() {
            return Ok(path);
        }
        bail!(
            "explicitly specified `{name}` binary not found: {}",
            path.display()
        );
    }
    which::which(name).with_context(|| {
        format!(
            "`{name}` not found on PATH; specify `--{name}` or set `{}` env var",
            env_var_for(name)
        )
    })
}

fn env_var_for(name: &str) -> &'static str {
    match name {
        "selector4nix" => "SELECTOR4NIX_BIN",
        "nix" => "NIX_BIN",
        "nix-serve" => "NIX_SERVE_BIN",
        _ => "UNKNOWN",
    }
}
