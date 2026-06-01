mod assertions;
mod cli;
mod context;
mod fixture;

use std::collections::HashSet;

use anyhow::{Context, Result as AnyhowResult};
use selector4nix_system_test_common::selector4nix::Selector4NixInstance;

use assertions::*;
use context::SharedContext;
use fastrand::Rng;
use fixture::TestFixtures;

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    let paths = cli::resolve()?;
    let count = paths.count;
    let seed = paths.seed;
    let repeat = paths.repeat;

    let fixtures = TestFixtures::new(count, seed);
    let shared = SharedContext::init(&fixtures, &paths).await?;
    eprintln!("shared context ready. populated {count} files (seed=`{seed}`, repeat=`{repeat}`)");

    let mut seed_index = 0usize;

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `valid_hash_returns_narinfo` [{r}] (seed=`{testcase_seed}`)");
        let proxy = shared.start_proxy().await?;
        valid_hash_returns_narinfo(&shared, &proxy).await?;
        seed_index += 1;
    }

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `invalid_hash_returns_404` [{r}] (seed=`{testcase_seed}`)");
        let proxy = shared.start_proxy().await?;
        let mut rng = Rng::with_seed(testcase_seed);
        invalid_hash_returns_404(&shared, &proxy, &mut rng).await?;
        seed_index += 1;
    }

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `same_hash_idempotent` [{r}] (seed=`{testcase_seed}`)");
        let proxy = shared.start_proxy().await?;
        let mut rng = Rng::with_seed(testcase_seed);
        same_hash_idempotent(&shared, &proxy, &mut rng).await?;
        seed_index += 1;
    }

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `concurrent_fetches_correct` [{r}] (seed=`{testcase_seed}`)");
        let proxy = shared.start_proxy().await?;
        concurrent_fetches_correct(&shared, &proxy).await?;
        seed_index += 1;
    }

    for r in 0..repeat {
        let testcase_seed = derive_seed(seed, seed_index);
        eprintln!("testcase `mixed_valid_invalid` [{r}] (seed=`{testcase_seed}`)");
        let proxy = shared.start_proxy().await?;
        let mut rng = Rng::with_seed(testcase_seed);
        mixed_valid_invalid(&shared, &proxy, &mut rng).await?;
        seed_index += 1;
    }

    eprintln!("all testcases passed");
    Ok(())
}

fn derive_seed(base: u64, index: usize) -> u64 {
    Rng::with_seed(base).u64(..).wrapping_add(index as u64)
}

async fn valid_hash_returns_narinfo(
    shared: &SharedContext,
    proxy: &Selector4NixInstance,
) -> AnyhowResult<()> {
    for hash in shared.valid_hashes() {
        let response = fetch_nar_info(shared.client(), proxy.base_url(), hash).await?;
        assert_nar_info_ok(response, hash)
            .await
            .with_context(|| format!("`hash={hash}`"))?;
    }
    Ok(())
}

async fn invalid_hash_returns_404(
    shared: &SharedContext,
    proxy: &Selector4NixInstance,
    rng: &mut Rng,
) -> AnyhowResult<()> {
    let valid_set: HashSet<&str> = shared.valid_hashes().into_iter().collect();

    let mut generated = 0;
    while generated < 100 {
        let hash = fixture::generate_invalid_hash(rng);
        if valid_set.contains(hash.as_str()) {
            continue;
        }
        let response = fetch_nar_info(shared.client(), proxy.base_url(), &hash).await?;
        assert_nar_info_not_found(response, &hash).await?;
        generated += 1;
    }
    Ok(())
}

async fn same_hash_idempotent(
    shared: &SharedContext,
    proxy: &Selector4NixInstance,
    rng: &mut Rng,
) -> AnyhowResult<()> {
    let hashes = shared.valid_hashes();
    let sampled = sample_hashes(&hashes, 30, rng);

    for hash in sampled {
        let response1 = fetch_nar_info(shared.client(), proxy.base_url(), hash).await?;
        let body1 = assert_nar_info_ok_get_body(response1, hash).await?;

        let response2 = fetch_nar_info(shared.client(), proxy.base_url(), hash).await?;
        let body2 = assert_nar_info_ok_get_body(response2, hash).await?;

        let response3 = fetch_nar_info(shared.client(), proxy.base_url(), hash).await?;
        let body3 = assert_nar_info_ok_get_body(response3, hash).await?;

        if body1 != body2 || body2 != body3 {
            anyhow::bail!(
                "idempotency violation for `hash={hash}`: responses differ across 3 requests"
            );
        }
    }
    Ok(())
}

async fn concurrent_fetches_correct(
    shared: &SharedContext,
    proxy: &Selector4NixInstance,
) -> AnyhowResult<()> {
    let hashes = shared.valid_hashes();
    let mut tasks = Vec::with_capacity(hashes.len());

    for hash in &hashes {
        let client = shared.client().clone();
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
            .context("`concurrent_fetches_correct` failed")?;
    }
    Ok(())
}

async fn mixed_valid_invalid(
    shared: &SharedContext,
    proxy: &Selector4NixInstance,
    rng: &mut Rng,
) -> AnyhowResult<()> {
    let valid_hashes = shared.valid_hashes();
    let valid_set: HashSet<&str> = valid_hashes.iter().copied().collect();
    let total = 200;
    let mut tasks = Vec::with_capacity(total);

    for _ in 0..total {
        let is_valid = rng.bool();
        let client = shared.client().clone();
        let base_url = proxy.base_url().clone();

        if is_valid {
            let hash = valid_hashes[rng.usize(..valid_hashes.len())].to_string();
            tasks.push(tokio::spawn(async move {
                let response = fetch_nar_info(&client, &base_url, &hash).await?;
                assert_nar_info_ok(response, &hash).await
            }));
        } else {
            let mut hash = fixture::generate_invalid_hash(rng);
            while valid_set.contains(hash.as_str()) {
                hash = fixture::generate_invalid_hash(rng);
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
            .context("`mixed_valid_invalid` failed")?;
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
