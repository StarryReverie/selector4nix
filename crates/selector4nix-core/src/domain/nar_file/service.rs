use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::domain::common::expire_at::ExpireAt;
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_file::model::{NarFile, NarFileLocation};
use crate::domain::nar_file::port::{NarStreamData, NarStreamProvider};
use crate::domain::nar_info::model::NarFileName;
use crate::domain::substituter::SubstituterRepository;
use crate::{AppError, AppResultExt};

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
    ) -> (NarFile, Result<NarStreamData, AppError>) {
        let nar_file_name = nar_file.key().to_file_name();

        if let Some(location) = nar_file.location() {
            // Don't send requests to selected substituter that is unavailable and do fallback
            // early. Always avoid short-circuit if `{url}/nar != storage_url`, because we only
            // probe the health information of host of `url` and can't determine `storage_url`'s
            // status.
            if location.substituter().has_custom_storage_url()
                || self
                    .substituter_repository
                    .exists_available(location.substituter().url())
                    .await
            {
                tracing::trace!(nar_file = %nar_file_name.value(), source_url = %location.source_url(), "use cached nar file location");

                let locations = [location.clone()];
                let outcome = self
                    .nar_stream_provider
                    .stream_nar(&locations, &headers)
                    .await;

                if let Ok(Some(data)) = outcome {
                    return (nar_file, Ok(data));
                }
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
    ) -> (NarFile, Result<NarStreamData, AppError>) {
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
                        nar_file.on_located(location, expire_at, None)
                    }
                };
                (nar_file, Ok(data))
            }
            Ok(None) => (
                nar_file,
                Err(AppError::not_found(
                    "failed to acquire stream for non-existent nar file",
                )),
            ),
            Err(err) => (
                nar_file,
                Err(err).chain_infrastructure("failed to acquire nar stream from substituters"),
            ),
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
                NarFileLocation::new(source_url, sub.meta().clone(), timeout)
            })
            .collect()
    }
}
