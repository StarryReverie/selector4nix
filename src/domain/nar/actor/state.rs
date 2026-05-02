use crate::domain::nar::model::{Nar, NarInfoData};
use crate::domain::substituter::model::{Substituter, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarActorEffect {
    ReportSubstituterSuccess(Url),
    ReportSubstituterFailure(Url),
}

pub enum AbnormalQueryOutcome {
    NotFound,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NarActorState(Nar);

impl NarActorState {
    pub fn new(nar: Nar) -> Self {
        Self(nar)
    }

    pub fn inner(&self) -> &Nar {
        &self.0
    }

    pub fn on_query_completed(
        Self(nar): Self,
        outcome: Result<(Substituter, NarInfoData), AbnormalQueryOutcome>,
        rewrite_nar_url: bool,
    ) -> Self {
        match outcome {
            Ok((substituter, nar_info)) => {
                let source_url = nar_info.source_url().cloned().unwrap_or_else(|| {
                    nar_info
                        .nar_file()
                        .with_storage_prefix(substituter.target().storage_url())
                });
                let nar_info = if rewrite_nar_url {
                    nar_info.rewrite_url_to_self()
                } else {
                    nar_info
                };
                Self(nar.on_resolved(nar_info, source_url))
            }
            Err(AbnormalQueryOutcome::NotFound) => Self(nar.on_not_found()),
            Err(AbnormalQueryOutcome::Error) => Self(nar),
        }
    }
}

impl From<Nar> for NarActorState {
    fn from(nar: Nar) -> Self {
        Self::new(nar)
    }
}
