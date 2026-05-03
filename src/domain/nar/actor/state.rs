use crate::domain::nar::model::{AbnormalQueryOutcome, Nar, NarInfoData};
use crate::domain::substituter::model::{Substituter, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarActorEffect {
    ReportSubstituterSuccess(Url),
    ReportSubstituterFailure(Url),
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
        let outcome = outcome
            .map(|(substituter, nar_info)| (nar_info, substituter.target().storage_url().clone()));
        Self(nar.on_query_completed(outcome, rewrite_nar_url))
    }
}

impl From<Nar> for NarActorState {
    fn from(nar: Nar) -> Self {
        Self::new(nar)
    }
}
