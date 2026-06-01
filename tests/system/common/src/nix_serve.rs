use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result as AnyhowResult, bail};
use reqwest::Client;

use crate::net::allocate_port;
use crate::subprocess::SubprocessGuard;

const READINESS_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub struct NixServeInstance {
    _guard: SubprocessGuard,
    port: u16,
}

impl NixServeInstance {
    pub async fn start(
        nix_serve_bin: &Path,
        cache_dir: &Path,
        client: Client,
    ) -> AnyhowResult<Self> {
        let port = allocate_port();

        let store_uri = format!("file://{}", cache_dir.display());
        let child = Command::new(nix_serve_bin)
            .args(["--store", &store_uri, "--port", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn `nix-serve`")?;

        let guard = SubprocessGuard::new(child);

        wait_ready(&client, port).await?;

        Ok(Self {
            _guard: guard,
            port,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

async fn wait_ready(client: &Client, port: u16) -> AnyhowResult<()> {
    let probe_url = format!("http://127.0.0.1:{port}/nix-cache-info");
    let start = Instant::now();
    loop {
        match client.get(&probe_url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => {
                if start.elapsed() > READINESS_TIMEOUT {
                    bail!("`nix-serve` did not become ready within {READINESS_TIMEOUT:?}");
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }
}
