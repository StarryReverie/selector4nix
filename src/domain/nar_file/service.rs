use std::sync::Arc;
use std::time::{Duration, SystemTime};

use snafu::Snafu;

use crate::domain::common::expire_at::ExpireAt;
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_file::model::{NarFile, NarFileLocation};
use crate::domain::nar_file::port::{NarStreamData, NarStreamProvider};
use crate::domain::nar_info::model::NarFileName;
use crate::domain::substituter::SubstituterRepository;

pub struct NarFileService {
    nar_stream_provider: Arc<dyn NarStreamProvider>,
    substituter_repository: Arc<dyn SubstituterRepository>,
    nar_file_ttl: Duration,
}

impl NarFileService {
    pub fn new(
        nar_stream_provider: Arc<dyn NarStreamProvider>,
        substituter_repository: Arc<dyn SubstituterRepository>,
        nar_file_ttl: Duration,
    ) -> Self {
        Self {
            nar_stream_provider,
            substituter_repository,
            nar_file_ttl,
        }
    }

    pub async fn stream(
        &self,
        nar_file: NarFile,
        headers: PassthroughHeaders,
        now: SystemTime,
    ) -> (NarFile, Result<Option<NarStreamData>, StreamNarFileError>) {
        let nar_file_name = nar_file.key().to_file_name();

        if let Some(location) = nar_file.location() {
            tracing::trace!(nar_file = %nar_file_name.value(), source_url = %location.source_url(), "use cached nar file location");

            let locations = [location.clone()];
            let outcome = self
                .nar_stream_provider
                .stream_nar(&locations, &headers)
                .await;

            if let Ok(Some(data)) = outcome {
                return (nar_file, Ok(Some(data)));
            }

            tracing::trace!(nar_file = %nar_file_name.value(), "fallback to query all substituters for nar file location");
        } else {
            tracing::trace!(nar_file = %nar_file_name.value(), "query all substituters for nar file location");
        }

        let candidates = self.build_candidates_from_all(&nar_file_name).await;
        self.stream_from_all(nar_file, headers, candidates, now)
            .await
    }

    async fn stream_from_all(
        &self,
        nar_file: NarFile,
        headers: PassthroughHeaders,
        candidates: Vec<NarFileLocation>,
        now: SystemTime,
    ) -> (NarFile, Result<Option<NarStreamData>, StreamNarFileError>) {
        let outcome = self
            .nar_stream_provider
            .stream_nar(&candidates, &headers)
            .await;

        match outcome {
            Ok(Some(data)) => {
                let location = candidates
                    .into_iter()
                    .find(|loc| loc.source_url() == &data.source_url)
                    .expect("returned `source_url` should match a candidate");
                let nar_file = match nar_file.location() {
                    Some(_) => nar_file.on_relocated(location),
                    None => {
                        let expire_at = ExpireAt::since(now, self.nar_file_ttl);
                        nar_file.on_located(location, expire_at)
                    }
                };
                (nar_file, Ok(Some(data)))
            }
            Ok(None) => (nar_file, Ok(None)),
            Err(_) => (nar_file, Err(StreamNarFileError::Infrastructure)),
        }
    }

    async fn build_candidates_from_all(&self, nar_file_name: &NarFileName) -> Vec<NarFileLocation> {
        self.substituter_repository
            .query_all_available()
            .await
            .iter()
            .map(|sub| {
                let source_url = nar_file_name.with_storage_prefix(sub.meta().storage_url());
                let timeout = sub.meta().nar_timeout();
                NarFileLocation::new(source_url, timeout)
            })
            .collect()
    }
}

#[derive(Snafu, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StreamNarFileError {
    #[snafu(display("failed to stream nar file"))]
    Infrastructure,
}
