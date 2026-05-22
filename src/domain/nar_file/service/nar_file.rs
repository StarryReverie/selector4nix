use std::sync::Arc;

use snafu::Snafu;
use tracing;

use crate::domain::nar_file::model::{NarFile, NarFileLocation};
use crate::domain::nar_file::port::{NarStreamData, NarStreamProvider};
use crate::domain::nar_info::model::NarFileName;
use crate::domain::substituter::index::SubstituterAvailabilityIndex;

pub struct NarFileService {
    nar_stream_provider: Arc<dyn NarStreamProvider>,
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
}

impl NarFileService {
    pub fn new(
        nar_stream_provider: Arc<dyn NarStreamProvider>,
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
    ) -> Self {
        Self {
            nar_stream_provider,
            substituter_availability_index,
        }
    }

    pub async fn stream(
        &self,
        nar_file: NarFile,
    ) -> (NarFile, Result<Option<NarStreamData>, StreamNarFileError>) {
        let nar_file_name = nar_file.key().to_file_name();

        if let Some(location) = nar_file.location() {
            tracing::trace!(nar_file = %nar_file_name.value(), source_url = %location.source_url(), "use cached nar file location");

            let locations = [location.clone()];
            let outcome = self.nar_stream_provider.stream_nar(&locations).await;

            if let Ok(Some(data)) = outcome {
                return (nar_file, Ok(Some(data)));
            }

            tracing::trace!(nar_file = %nar_file_name.value(), "fallback to query all substituters for nar file location");
        } else {
            tracing::trace!(nar_file = %nar_file_name.value(), "query all substituters for nar file location");
        }

        let candidates = self.build_candidates_from_all(&nar_file_name);
        self.stream_from_all(nar_file, candidates).await
    }

    async fn stream_from_all(
        &self,
        nar_file: NarFile,
        candidates: Vec<NarFileLocation>,
    ) -> (NarFile, Result<Option<NarStreamData>, StreamNarFileError>) {
        let outcome = self.nar_stream_provider.stream_nar(&candidates).await;

        match outcome {
            Ok(Some(data)) => {
                let location = candidates
                    .into_iter()
                    .find(|loc| loc.source_url() == &data.source_url)
                    .expect("returned `source_url` should match a candidate");
                (nar_file.with_location(location), Ok(Some(data)))
            }
            Ok(None) => (nar_file, Ok(None)),
            Err(_) => (nar_file, Err(StreamNarFileError::Infrastructure)),
        }
    }

    fn build_candidates_from_all(&self, nar_file_name: &NarFileName) -> Vec<NarFileLocation> {
        self.substituter_availability_index
            .query_all()
            .iter()
            .map(|sub| {
                let source_url = nar_file_name.with_storage_prefix(sub.target().storage_url());
                let timeout = sub.target().nar_timeout();
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
