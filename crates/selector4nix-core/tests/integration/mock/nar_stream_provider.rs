use std::collections::HashSet;
use std::sync::Mutex;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::common::url::Url;
use selector4nix_core::domain::nar_file::model::NarFileLocation;
use selector4nix_core::domain::nar_file::port::{
    NarStreamData, NarStreamHeaders, NarStreamProvider,
};

pub struct MockNarStreamProvider {
    success_urls: HashSet<Url>,
    contacted: Mutex<HashSet<Url>>,
}

impl MockNarStreamProvider {
    pub fn new<I>(success_urls: I) -> Self
    where
        I: IntoIterator<Item = Url>,
    {
        Self {
            success_urls: success_urls.into_iter().collect(),
            contacted: Mutex::new(HashSet::new()),
        }
    }

    pub fn has_contacted_url(&self, url: &Url) -> bool {
        self.contacted.lock().unwrap().contains(url)
    }
}

#[async_trait]
impl NarStreamProvider for MockNarStreamProvider {
    async fn stream_nar(
        &self,
        locations: &[NarFileLocation],
        _headers: &PassthroughHeaders,
    ) -> AnyhowResult<Option<NarStreamData>> {
        {
            let mut contacted = self.contacted.lock().unwrap();
            for loc in locations {
                contacted.insert(loc.source_url().clone());
            }
        }

        for loc in locations {
            if self.success_urls.contains(loc.source_url()) {
                let data = NarStreamData::new(
                    NarStreamHeaders {
                        content_length: None,
                        content_type: None,
                        content_encoding: None,
                    },
                    Box::pin(futures::stream::empty()),
                    loc.source_url().clone(),
                );
                return Ok(Some(data));
            }
        }
        Ok(None)
    }
}
