use anyhow::Error as AnyhowError;
use async_trait::async_trait;
use snafu::Snafu;

use crate::domain::substituter::model::SubstituterMeta;

#[async_trait]
pub trait SubstituterProbingProvider: Send + Sync {
    async fn probe_substituter(
        &self,
        substituter: &SubstituterMeta,
    ) -> Result<(), ProbeSubstituterError>;
}

#[derive(Snafu, Debug)]
#[non_exhaustive]
#[snafu(visibility(pub))]
pub enum ProbeSubstituterError {
    #[snafu(display("could not probe offline substituter"))]
    Offline { source: AnyhowError },
    #[snafu(display("probing got service error from substituter"))]
    Service { source: AnyhowError },
}

pub mod error_ctx {
    pub use super::{OfflineSnafu, ServiceSnafu};
}
