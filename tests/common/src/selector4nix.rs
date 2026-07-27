use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result as AnyhowResult, bail};
use reqwest::Client;
use tempfile::NamedTempFile;
use url::Url;

use crate::net::allocate_port;
use crate::subprocess::SubprocessGuard;

const READINESS_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub struct Selector4NixInstance {
    _guard: SubprocessGuard,
    _config_file: NamedTempFile,
    base_url: Url,
}

impl Selector4NixInstance {
    pub fn builder(bin: PathBuf, client: Client) -> Selector4NixInstanceBuilder {
        Selector4NixInstanceBuilder {
            bin,
            client,
            substituters: Vec::new(),
            cache_dir: None,
        }
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }
}

pub struct Selector4NixInstanceBuilder {
    bin: PathBuf,
    client: Client,
    substituters: Vec<Url>,
    cache_dir: Option<PathBuf>,
}

impl Selector4NixInstanceBuilder {
    pub fn substituter(mut self, url: Url) -> Self {
        self.substituters.push(url);
        self
    }

    pub fn cache_dir(mut self, path: PathBuf) -> Self {
        self.cache_dir = Some(path);
        self
    }

    pub async fn start(self) -> AnyhowResult<Selector4NixInstance> {
        let port = allocate_port();

        let mut substituter_toml = String::new();
        for url in &self.substituters {
            substituter_toml.push_str(&format!("[[substituters]]\nurl = \"{url}\"\n\n"));
        }

        let config_content = format!(
            "[server]\nip = \"127.0.0.1\"\nport = {port}\n\n[network]\nperiodic_probing = false\n\n{substituter_toml}"
        );

        let mut config_file = NamedTempFile::new().context("failed to create temp config file")?;
        config_file
            .write_all(config_content.as_bytes())
            .context("failed to write config file")?;
        config_file.flush().context("failed to flush config file")?;

        let child = Command::new(&self.bin)
            .args(["--config-file", &config_file.path().to_string_lossy()])
            .args(
                self.cache_dir
                    .iter()
                    .flat_map(|p| ["--cache-dir".to_string(), p.to_string_lossy().into_owned()]),
            )
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to spawn `{}`", self.bin.display()))?;

        let base_url = Url::parse(&format!("http://127.0.0.1:{port}"))
            .context("failed to construct base URL")?;

        wait_ready(&self.client, &base_url).await?;

        Ok(Selector4NixInstance {
            _guard: SubprocessGuard::new(child),
            _config_file: config_file,
            base_url,
        })
    }
}

async fn wait_ready(client: &Client, base_url: &Url) -> AnyhowResult<()> {
    let url = base_url
        .join("nix-cache-info")
        .context("failed to construct readiness URL")?;
    let start = Instant::now();
    loop {
        match client.get(url.as_str()).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            _ => {
                if start.elapsed() > READINESS_TIMEOUT {
                    bail!("`selector4nix` did not become ready within {READINESS_TIMEOUT:?}");
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }
}
