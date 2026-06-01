use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result as AnyhowResult};
use reqwest::Client;
use selector4nix_system_test_common::nix_serve::NixServeInstance;
use selector4nix_system_test_common::nix_store::NixStore;
use selector4nix_system_test_common::selector4nix::Selector4NixInstance;
use url::Url;

use crate::cli::ResolvedPaths;
use crate::fixture::TestFixtures;

pub struct SharedContext {
    store: NixStore,
    nix_serve: NixServeInstance,
    client: Client,
    selector4nix_bin: PathBuf,
}

impl SharedContext {
    pub async fn init(fixtures: &TestFixtures, paths: &ResolvedPaths) -> AnyhowResult<Self> {
        let mut store = NixStore::create(paths.nix.clone())?;
        for (i, content) in fixtures.contents().iter().enumerate() {
            store.add_file(&format!("input-{i}"), content)?;
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("failed to build HTTP client")?;

        let nix_serve =
            NixServeInstance::start(&paths.nix_serve, store.path(), client.clone()).await?;

        Ok(Self {
            store,
            nix_serve,
            client,
            selector4nix_bin: paths.selector4nix.clone(),
        })
    }

    pub async fn start_proxy(&self) -> AnyhowResult<Selector4NixInstance> {
        let upstream_url =
            Url::parse(&format!("http://127.0.0.1:{}/", self.nix_serve.port())).unwrap();
        Selector4NixInstance::builder(self.selector4nix_bin.clone(), self.client.clone())
            .substituter(upstream_url)
            .start()
            .await
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn valid_hashes(&self) -> Vec<&str> {
        self.store.entries().map(|e| e.hash.as_str()).collect()
    }
}
