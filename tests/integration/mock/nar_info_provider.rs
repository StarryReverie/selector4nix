use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use selector4nix::domain::nar::port::{NarInfoProvider, NarInfoQueryData};
use selector4nix::domain::substituter::model::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockNarInfoProvider {
    queries: HashMap<Url, Result<NarInfoQueryData, String>>,
}

impl MockNarInfoProvider {
    pub fn new<I>(queries: I) -> Self
    where
        I: IntoIterator<Item = (Url, Result<NarInfoQueryData, String>)>,
    {
        Self {
            queries: queries.into_iter().collect(),
        }
    }
}

#[async_trait]
impl NarInfoProvider for MockNarInfoProvider {
    async fn query_nar_info(
        &self,
        url: &Url,
        timeout: Option<Duration>,
    ) -> AnyhowResult<Option<NarInfoQueryData>> {
        let Some(data) = self.queries.get(url) else {
            return Ok(None);
        };

        match (data, timeout) {
            (Ok(data), Some(timeout)) if data.latency > timeout => {
                tokio::time::sleep(timeout).await;
                Err(anyhow::anyhow!("timeout"))
            }
            (Ok(data), _) => {
                tokio::time::sleep(data.latency).await;
                Ok(Some(data.clone()))
            }
            (Err(err), _) => Err(anyhow::anyhow!("{err}")),
        }
    }
}
