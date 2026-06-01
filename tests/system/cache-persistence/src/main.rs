mod cli;

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result as AnyhowResult, bail};
use fastrand::Rng;
use reqwest::Client;
use selector4nix_system_test_common::nix_serve::NixServeInstance;
use selector4nix_system_test_common::nix_store::{NixStore, generate_random_bytes};
use selector4nix_system_test_common::selector4nix::Selector4NixInstance;
use tempfile::TempDir;
use url::Url;

use crate::cli::TestConfig;

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    let config = cli::resolve()?;

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to build HTTP client")?;

    let mut rng = Rng::with_seed(config.seed);
    let contents: Vec<Vec<u8>> = (0..config.count)
        .map(|_| generate_random_bytes(rng.usize(1..100_000), &mut rng))
        .collect();

    let mut store = NixStore::create(config.nix_bin.clone())?;
    for (i, content) in contents.iter().enumerate() {
        store.add_file(&format!("input-{i}"), content)?;
    }

    let nix_serve =
        NixServeInstance::start(&config.nix_serve_bin, store.path(), client.clone()).await?;
    let upstream_url = Url::parse(&format!("http://127.0.0.1:{}/", nix_serve.port())).unwrap();
    let hashes: Vec<&str> = store.entries().map(|e| e.hash.as_str()).collect();

    eprintln!(
        "test environment ready. populated {} files (seed=`{}`)",
        config.count, config.seed
    );

    eprintln!("testcase `nar_info_cached_after_restart`");
    nar_info_cached_after_restart(&config, &client, &upstream_url, &hashes).await?;

    eprintln!("testcase `nar_streamed_from_cached_location`");
    nar_streamed_from_cached_location(&config, &client, &upstream_url, &hashes).await?;

    eprintln!("all testcases passed");
    Ok(())
}

async fn nar_info_cached_after_restart(
    config: &TestConfig,
    client: &Client,
    upstream_url: &Url,
    hashes: &[&str],
) -> AnyhowResult<()> {
    let cache_dir = TempDir::new().context("failed to create cache dir")?;

    let instance_a = Selector4NixInstance::builder(config.selector4nix_bin.clone(), client.clone())
        .substituter(upstream_url.clone())
        .cache_dir(cache_dir.path().to_path_buf())
        .start()
        .await?;
    eprintln!("  selector4nix instance A: started");

    let mut expected: HashMap<&str, String> = HashMap::new();
    for hash in hashes {
        let body = fetch_nar_info(client, instance_a.base_url(), hash).await?;
        expected.insert(hash, body);
    }
    eprintln!(
        "  selector4nix instance A: warmed {} nar info entries",
        hashes.len()
    );

    drop(instance_a);
    eprintln!("  selector4nix instance A: stopped");

    let instance_b = Selector4NixInstance::builder(config.selector4nix_bin.clone(), client.clone())
        .substituter(upstream_url.clone())
        .cache_dir(cache_dir.path().to_path_buf())
        .start()
        .await?;
    eprintln!("  selector4nix instance B: started");

    for hash in hashes {
        let body = fetch_nar_info(client, instance_b.base_url(), hash).await?;
        let expected_body = expected.get(hash).context("missing expected body")?;
        if body != *expected_body {
            bail!(
                "nar info body mismatch for `{hash}` across restarts\n\
                 expected:\n{expected_body}\n\
                 got:\n{body}"
            );
        }
    }
    eprintln!(
        "  selector4nix instance B: all {} nar info entries match",
        hashes.len()
    );

    drop(instance_b);
    eprintln!("  selector4nix instance B: stopped");
    Ok(())
}

async fn nar_streamed_from_cached_location(
    config: &TestConfig,
    client: &Client,
    upstream_url: &Url,
    hashes: &[&str],
) -> AnyhowResult<()> {
    let cache_dir = TempDir::new().context("failed to create cache dir")?;

    let instance_a = Selector4NixInstance::builder(config.selector4nix_bin.clone(), client.clone())
        .substituter(upstream_url.clone())
        .cache_dir(cache_dir.path().to_path_buf())
        .start()
        .await?;
    eprintln!("  selector4nix instance A: started");

    for hash in hashes {
        let body = fetch_nar_info(client, instance_a.base_url(), hash).await?;
        let nar_path = extract_nar_url(&body)?;
        let size = fetch_nar_size(client, instance_a.base_url(), &nar_path).await?;
        if size == 0 {
            bail!("nar file for `{hash}` has zero size via selector4nix instance A");
        }
    }
    eprintln!(
        "  selector4nix instance A: warmed {} nar entries",
        hashes.len()
    );

    drop(instance_a);
    eprintln!("  selector4nix instance A: stopped");

    let instance_b = Selector4NixInstance::builder(config.selector4nix_bin.clone(), client.clone())
        .substituter(upstream_url.clone())
        .cache_dir(cache_dir.path().to_path_buf())
        .start()
        .await?;
    eprintln!("  selector4nix instance B: started");

    for hash in hashes {
        let body = fetch_nar_info(client, instance_b.base_url(), hash).await?;
        let nar_path = extract_nar_url(&body)?;
        let size = fetch_nar_size(client, instance_b.base_url(), &nar_path).await?;
        if size == 0 {
            bail!("nar file for `{hash}` has zero size via selector4nix instance B");
        }
    }
    eprintln!(
        "  selector4nix instance B: all {} nar entries streamed with nonzero size",
        hashes.len()
    );

    drop(instance_b);
    eprintln!("  selector4nix instance B: stopped");
    Ok(())
}

async fn fetch_nar_info(client: &Client, base_url: &Url, hash: &str) -> AnyhowResult<String> {
    let url = base_url.join(&format!("{hash}.narinfo")).unwrap();
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("nar info request failed for `{hash}`"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("expected 200 for nar info `{hash}`, got {status}: {body}");
    }
    response
        .text()
        .await
        .context("failed to read nar info body")
}

fn extract_nar_url(nar_info_body: &str) -> AnyhowResult<String> {
    nar_info_body
        .lines()
        .find(|line| line.starts_with("URL:"))
        .map(|line| line.trim_start_matches("URL:").trim().to_string())
        .context("nar info body missing `URL:` line")
}

async fn fetch_nar_size(client: &Client, base_url: &Url, nar_path: &str) -> AnyhowResult<usize> {
    let url = base_url.join(nar_path).unwrap();
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("nar request failed for `{nar_path}`"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("expected 200 for nar `{nar_path}`, got {status}: {body}");
    }
    let bytes = response.bytes().await.context("failed to read nar body")?;
    Ok(bytes.len())
}
