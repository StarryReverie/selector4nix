mod assertions;
mod cli;
mod context;

use std::collections::HashSet;

use anyhow::{Context, Result as AnyhowResult};
use fastrand::Rng;
use reqwest::{Client, Response};
use selector4nix_system_test_common::nix_store::{generate_hash, generate_random_bytes};
use selector4nix_system_test_common::selector4nix::Selector4NixInstance;
use url::Url;

use crate::assertions::*;
use crate::context::TestContext;

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    let config = cli::resolve()?;
    let count = config.count;
    let seed = config.seed;
    let repeat = config.repeat;

    let mut rng = Rng::with_seed(seed);
    let contents: Vec<Vec<u8>> = (0..count)
        .map(|_| generate_random_bytes(rng.usize(1..100_000), &mut rng))
        .collect();
    let context = TestContext::init(&contents, &config).await?;
    eprintln!("shared context ready. populated {count} files (seed=`{seed}`, repeat=`{repeat}`)");

    let mut seed_index = 0usize;

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `valid_hash_succeeds` [{r}] (seed=`{testcase_seed}`)");
        let proxy = context.start_proxy().await?;
        valid_hash_succeeds(&context, &proxy).await?;
        seed_index += 1;
    }

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `nonexistent_hash_returns_404` [{r}] (seed=`{testcase_seed}`)");
        let proxy = context.start_proxy().await?;
        let mut rng = Rng::with_seed(testcase_seed);
        nonexistent_hash_returns_404(&context, &proxy, &mut rng).await?;
        seed_index += 1;
    }

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `same_hash_succeeds_idempotently` [{r}] (seed=`{testcase_seed}`)");
        let proxy = context.start_proxy().await?;
        let mut rng = Rng::with_seed(testcase_seed);
        same_hash_succeeds_idempotently(&context, &proxy, &mut rng).await?;
        seed_index += 1;
    }

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `concurrent_fetches_succeeds` [{r}] (seed=`{testcase_seed}`)");
        let proxy = context.start_proxy().await?;
        concurrent_fetches_succeeds(&context, &proxy).await?;
        seed_index += 1;
    }

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `mixed_concurrent_fetches_succeeds` [{r}] (seed=`{testcase_seed}`)");
        let proxy = context.start_proxy().await?;
        let mut rng = Rng::with_seed(testcase_seed);
        mixed_concurrent_fetches_succeeds(&context, &proxy, &mut rng).await?;
        seed_index += 1;
    }

    eprintln!("all testcases passed");
    Ok(())
}

fn derive_seed(base: u64, index: usize) -> u64 {
    Rng::with_seed(base).u64(..).wrapping_add(index as u64)
}

async fn valid_hash_succeeds(
    context: &TestContext,
    proxy: &Selector4NixInstance,
) -> AnyhowResult<()> {
    for hash in context.valid_hashes() {
        let response = fetch_nar_info(context.client(), proxy.base_url(), hash).await?;
        assert_nar_info_ok(response, hash)
            .await
            .with_context(|| format!("`hash={hash}`"))?;
    }
    Ok(())
}

async fn nonexistent_hash_returns_404(
    context: &TestContext,
    proxy: &Selector4NixInstance,
    rng: &mut Rng,
) -> AnyhowResult<()> {
    let valid_set: HashSet<&str> = context.valid_hashes().into_iter().collect();

    let mut generated = 0;
    while generated < 100 {
        let hash = generate_hash(rng);
        if valid_set.contains(hash.as_str()) {
            continue;
        }
        let response = fetch_nar_info(context.client(), proxy.base_url(), &hash).await?;
        assert_nar_info_not_found(response, &hash).await?;
        generated += 1;
    }
    Ok(())
}

async fn same_hash_succeeds_idempotently(
    context: &TestContext,
    proxy: &Selector4NixInstance,
    rng: &mut Rng,
) -> AnyhowResult<()> {
    let hashes = context.valid_hashes();
    let sampled = sample_hashes(&hashes, 30, rng);

    for hash in sampled {
        let response1 = fetch_nar_info(context.client(), proxy.base_url(), hash).await?;
        let body1 = assert_nar_info_ok_and_get_body(response1, hash).await?;

        let response2 = fetch_nar_info(context.client(), proxy.base_url(), hash).await?;
        let body2 = assert_nar_info_ok_and_get_body(response2, hash).await?;

        let response3 = fetch_nar_info(context.client(), proxy.base_url(), hash).await?;
        let body3 = assert_nar_info_ok_and_get_body(response3, hash).await?;

        if body1 != body2 || body2 != body3 {
            anyhow::bail!(
                "idempotency violation for `hash={hash}`: responses differ across 3 requests"
            );
        }
    }
    Ok(())
}

async fn concurrent_fetches_succeeds(
    context: &TestContext,
    proxy: &Selector4NixInstance,
) -> AnyhowResult<()> {
    let hashes = context.valid_hashes();
    let mut tasks = Vec::with_capacity(hashes.len());

    for hash in &hashes {
        let client = context.client().clone();
        let base_url = proxy.base_url().clone();
        let hash = hash.to_string();
        tasks.push(tokio::spawn(async move {
            let response = fetch_nar_info(&client, &base_url, &hash).await?;
            assert_nar_info_ok(response, &hash).await
        }));
    }

    for task in tasks {
        task.await
            .context("batch task panicked")?
            .context("`concurrent_fetches_succeeds` failed")?;
    }
    Ok(())
}

async fn mixed_concurrent_fetches_succeeds(
    context: &TestContext,
    proxy: &Selector4NixInstance,
    rng: &mut Rng,
) -> AnyhowResult<()> {
    let valid_hashes = context.valid_hashes();
    let valid_set: HashSet<&str> = valid_hashes.iter().copied().collect();
    let total = 200;
    let mut tasks = Vec::with_capacity(total);

    for _ in 0..total {
        let is_valid = rng.bool();
        let client = context.client().clone();
        let base_url = proxy.base_url().clone();

        if is_valid {
            let hash = valid_hashes[rng.usize(..valid_hashes.len())].to_string();
            tasks.push(tokio::spawn(async move {
                let response = fetch_nar_info(&client, &base_url, &hash).await?;
                assert_nar_info_ok(response, &hash).await
            }));
        } else {
            let mut hash = generate_hash(rng);
            while valid_set.contains(hash.as_str()) {
                hash = generate_hash(rng);
            }
            tasks.push(tokio::spawn(async move {
                let response = fetch_nar_info(&client, &base_url, &hash).await?;
                assert_nar_info_not_found(response, &hash).await
            }));
        }
    }

    for task in tasks {
        task.await
            .context("mixed task panicked")?
            .context("`mixed_concurrent_fetches_succeeds` failed")?;
    }
    Ok(())
}

fn sample_hashes<'a>(pool: &[&'a str], n: usize, rng: &mut Rng) -> Vec<&'a str> {
    let n = n.min(pool.len());
    let mut indices: Vec<usize> = (0..pool.len()).collect();
    let len = indices.len();
    for i in (1..len).rev() {
        let j = rng.usize(..=i);
        indices.swap(i, j);
    }
    indices[..n].iter().map(|&i| pool[i]).collect()
}

pub async fn fetch_nar_info(client: &Client, base_url: &Url, hash: &str) -> AnyhowResult<Response> {
    let url = base_url
        .join(&format!("{hash}.narinfo"))
        .with_context(|| format!("failed to construct URL for `{hash}`"))?;
    client
        .get(url)
        .send()
        .await
        .with_context(|| format!("HTTP request failed for `{hash}`"))
}
