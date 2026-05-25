use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result as AnyhowResult, bail};
use reqwest::Client;
use tempfile::TempDir;

use crate::cli::ResolvedPaths;
use crate::fixture::TestFixtures;

const READINESS_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

struct SubprocessGuard {
    child: Child,
}

impl Drop for SubprocessGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct SharedContext {
    _tempdir: TempDir,
    _nix_serve: SubprocessGuard,
    upstream_port: u16,
    client: Client,
    fixtures: TestFixtures,
    selector4nix_bin: std::path::PathBuf,
    proxy_counter: u64,
}

pub struct ProxyInstance {
    _guard: SubprocessGuard,
    proxy_base_url: String,
}

impl SharedContext {
    pub async fn init(mut fixtures: TestFixtures, paths: &ResolvedPaths) -> AnyhowResult<Self> {
        let tempdir = TempDir::new().context("failed to create temp directory")?;
        let cache_dir = tempdir.path().join("cache");
        std::fs::create_dir(&cache_dir).context("failed to create cache directory")?;

        populate_cache(&mut fixtures, &cache_dir, &paths.nix)?;

        let upstream_port = find_free_port();
        let _nix_serve = start_nix_serve(&paths.nix_serve, &cache_dir, upstream_port)?;

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("failed to build HTTP client")?;

        wait_nix_serve_ready(&client, &cache_dir, upstream_port).await?;

        Ok(Self {
            _tempdir: tempdir,
            _nix_serve,
            upstream_port,
            client,
            fixtures,
            selector4nix_bin: paths.selector4nix.clone(),
            proxy_counter: 0,
        })
    }

    pub async fn start_proxy(&mut self) -> AnyhowResult<ProxyInstance> {
        let proxy_port = find_free_port();
        let config_name = format!("selector4nix-{}.toml", self.proxy_counter);
        self.proxy_counter += 1;

        let config_path = self._tempdir.path().join(&config_name);
        let config_content = format!(
            r#"[server]
ip = "127.0.0.1"
port = {proxy_port}

[network]
periodic_probing = false

[[substituters]]
url = "http://127.0.0.1:{upstream_port}/"
"#,
            upstream_port = self.upstream_port,
        );
        std::fs::write(&config_path, &config_content).context("failed to write config")?;

        let child = Command::new(&self.selector4nix_bin)
            .args(["--config-file", &config_path.to_string_lossy()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn `selector4nix`")?;

        let proxy_base_url = format!("http://127.0.0.1:{proxy_port}");

        wait_ready(&self.client, &proxy_base_url).await?;

        Ok(ProxyInstance {
            _guard: SubprocessGuard { child },
            proxy_base_url,
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn fixtures(&self) -> &TestFixtures {
        &self.fixtures
    }
}

impl ProxyInstance {
    pub fn proxy_base_url(&self) -> &str {
        &self.proxy_base_url
    }
}

async fn wait_nix_serve_ready(client: &Client, cache_dir: &Path, port: u16) -> AnyhowResult<()> {
    let first_hash = std::fs::read_dir(cache_dir)
        .context("failed to read cache dir")?
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().is_some_and(|ext| ext == "narinfo"))
        .map(|e| e.path());

    let Some(narinfo_path) = first_hash else {
        bail!("no .narinfo files found in cache directory");
    };
    let stem = narinfo_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let probe_url = format!("http://127.0.0.1:{port}/{stem}.narinfo");
    let start = Instant::now();
    loop {
        match client.get(&probe_url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => {
                if start.elapsed() > READINESS_TIMEOUT {
                    bail!(
                        "`nix-serve` did not become ready within {READINESS_TIMEOUT:?} (probed `{stem}`)"
                    );
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }
}

async fn wait_ready(client: &Client, proxy_base_url: &str) -> AnyhowResult<()> {
    let start = Instant::now();
    let url = format!("{proxy_base_url}/nix-cache-info");
    loop {
        match client.get(&url).send().await {
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

fn populate_cache(
    fixtures: &mut TestFixtures,
    cache_dir: &Path,
    nix_bin: &Path,
) -> AnyhowResult<()> {
    let contents: Vec<Vec<u8>> = fixtures.contents().to_vec();
    for (i, content) in contents.into_iter().enumerate() {
        let file_path = cache_dir.join(format!("input-{i}"));
        std::fs::write(&file_path, content)
            .with_context(|| format!("failed to write test file `{i}`"))?;

        let store_uri = format!("file://{}?compression=none", cache_dir.display());
        let output = Command::new(nix_bin)
            .args(["store", "add-file", "--store", &store_uri])
            .arg(&file_path)
            .env("NIX_CONFIG", "experimental-features = nix-command")
            .output()
            .with_context(|| format!("failed to spawn `nix store add-file` for file `{i}`"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`nix store add-file` failed for file `{i}`: {stderr}");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let store_path = stdout.trim();
        let hash = store_path
            .strip_prefix("/nix/store/")
            .and_then(|s| s.split_once('-'))
            .map(|(h, _)| h)
            .context(format!(
                "unexpected `nix store add-file` output for file `{i}`: {store_path}"
            ))?;

        fixtures.add_populated(hash.to_string());
    }
    Ok(())
}

fn find_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind ephemeral port");
    listener
        .local_addr()
        .expect("failed to get local address")
        .port()
}

fn start_nix_serve(
    nix_serve_bin: &Path,
    cache_dir: &Path,
    port: u16,
) -> AnyhowResult<SubprocessGuard> {
    let store_uri = format!("file://{}", cache_dir.display());
    let child = Command::new(nix_serve_bin)
        .args(["--store", &store_uri, "--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn `nix-serve`")?;

    Ok(SubprocessGuard { child })
}
