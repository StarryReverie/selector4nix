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

#[cfg(test)]
mod tests {
    use crate::domain::nar::model::{Nar, NarState, StorePathHash};
    use crate::domain::substituter::model::{Availability, Priority, SubstituterMeta, Url};

    use super::*;

    fn make_state() -> NarActorState {
        let hash = StorePathHash::new("p4pclmv1gyja5kzc26npqpia1qqxrf0l".into()).unwrap();
        NarActorState::new(Nar::new(hash))
    }

    fn make_substituter(url: &str, priority: u32) -> Substituter {
        Substituter::new(
            SubstituterMeta::new(Url::new(url).unwrap(), Priority::new(priority).unwrap()),
            Availability::Normal,
        )
    }

    fn make_nar_info_data() -> NarInfoData {
        NarInfoData::rewritten(
            "StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-hello\nURL: nar/abc.nar.xz\n"
                .into(),
        )
        .unwrap()
    }

    fn make_nar_info_data_with_external_url() -> NarInfoData {
        NarInfoData::original(
            "StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-hello\nURL: https://other.com/custom/abc.nar.xz\n"
                .into(),
        )
        .unwrap()
    }

    #[test]
    fn on_query_completed_resolves_given_found() {
        let state = make_state();
        let sub = make_substituter("https://cache.nixos.org", 40);
        let data = make_nar_info_data();

        let new_state = NarActorState::on_query_completed(state, Ok((sub, data)), true);

        assert!(matches!(
            new_state.inner().state(),
            NarState::Resolved { .. }
        ));
    }

    #[test]
    fn on_query_completed_preserves_original_url_given_rewrite_false() {
        let state = make_state();
        let sub = make_substituter("https://other.com", 40);
        let data = make_nar_info_data_with_external_url();

        let new_state = NarActorState::on_query_completed(state, Ok((sub, data)), false);

        match new_state.inner().state() {
            NarState::Resolved { nar_info, .. } => {
                assert!(
                    nar_info
                        .content()
                        .contains("https://other.com/custom/abc.nar.xz")
                );
                assert!(!nar_info.content().contains("URL: nar/abc.nar.xz"));
            }
            _ => panic!("expected Resolved state"),
        }
    }

    #[test]
    fn on_query_completed_rewrites_url_given_rewrite_true() {
        let state = make_state();
        let sub = make_substituter("https://other.com", 40);
        let data = make_nar_info_data_with_external_url();

        let new_state = NarActorState::on_query_completed(state, Ok((sub, data)), true);

        match new_state.inner().state() {
            NarState::Resolved { nar_info, .. } => {
                assert!(nar_info.content().contains("URL: nar/abc.nar.xz\n"));
                assert!(!nar_info.content().contains("https://other.com"));
            }
            _ => panic!("expected Resolved state"),
        }
    }

    #[test]
    fn on_query_completed_transitions_to_not_found() {
        let state = make_state();

        let new_state =
            NarActorState::on_query_completed(state, Err(AbnormalQueryOutcome::NotFound), true);

        assert!(matches!(new_state.inner().state(), NarState::NotFound));
    }

    #[test]
    fn on_query_completed_remains_unknown_given_error() {
        let state = make_state();

        let new_state =
            NarActorState::on_query_completed(state, Err(AbnormalQueryOutcome::Error), true);

        assert!(matches!(new_state.inner().state(), NarState::Unknown));
    }
}
