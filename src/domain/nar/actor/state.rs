use std::time::Duration;

use anyhow::Result as AnyhowResult;

use crate::domain::nar::model::{Nar, NarInfoQueryOutcome};
use crate::domain::substituter::model::{Priority, Substituter, Url};

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

    pub fn on_all_outcomes_acquired(
        Self(nar): Self,
        outcomes: Vec<AnyhowResult<NarInfoQueryOutcome>>,
        substituters: &[Substituter],
        rewrite_nar_url: bool,
    ) -> (Vec<NarActorEffect>, Self) {
        let mut effects = Vec::new();
        let mut optimal = None;
        let mut has_error = false;

        for (outcome, substituter) in outcomes.iter().zip(substituters.iter()) {
            match outcome {
                Ok(NarInfoQueryOutcome::Found {
                    original_data,
                    latency,
                }) => {
                    let preference = Self::calc_preference(*latency, substituter.priority());
                    optimal = match optimal {
                        prev @ Some((_, prev_preference)) if prev_preference > preference => prev,
                        _ => Some(((original_data, substituter), preference)),
                    };
                    let url = substituter.url().clone();
                    if !substituter.is_normal() {
                        effects.push(NarActorEffect::ReportSubstituterSuccess(url));
                    }
                }
                Ok(NarInfoQueryOutcome::NotFound) => {
                    let url = substituter.url().clone();
                    if !substituter.is_normal() {
                        effects.push(NarActorEffect::ReportSubstituterSuccess(url));
                    }
                }
                Err(_) => {
                    has_error = true;
                    let url = substituter.url().clone();
                    effects.push(NarActorEffect::ReportSubstituterFailure(url));
                }
            }
        }

        match optimal {
            Some(((nar_info, substituter), _)) => {
                let source_url = nar_info.source_url().cloned().unwrap_or_else(|| {
                    nar_info
                        .nar_file()
                        .with_storage_prefix(substituter.target().storage_url())
                });
                let nar_info = if rewrite_nar_url {
                    nar_info.clone().rewrite_url_to_self()
                } else {
                    nar_info.clone()
                };
                let nar = nar.on_resolved(substituter.target().clone(), nar_info, source_url);
                (effects, Self::new(nar))
            }
            None if !has_error => {
                let nar = nar.on_not_found();
                (effects, Self::new(nar))
            }
            None => (effects, Self::new(nar)),
        }
    }

    fn calc_preference(latency: Duration, priority: Priority) -> i64 {
        const TOLERANCE: i64 = 50;
        -(TOLERANCE * priority.value() as i64) - latency.as_millis() as i64
    }
}

impl From<Nar> for NarActorState {
    fn from(nar: Nar) -> Self {
        Self::new(nar)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::nar::model::{
        Nar, NarInfoData, NarInfoQueryOutcome, NarState, StorePathHash,
    };
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

    fn make_unavailable_substituter(url: &str, priority: u32) -> Substituter {
        Substituter::new(
            SubstituterMeta::new(Url::new(url).unwrap(), Priority::new(priority).unwrap()),
            Availability::MaybeReady { prev_failures: 1 },
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
    fn on_all_outcomes_acquired_succeeds_given_single_found() {
        let state = make_state();
        let sub = make_substituter("https://cache.nixos.org", 40);
        let data = make_nar_info_data();
        let outcomes = vec![Ok(NarInfoQueryOutcome::Found {
            original_data: data.clone(),
            latency: Duration::from_millis(100),
        })];
        let substituters = vec![sub.clone()];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters, true);

        assert!(effects.is_empty());
        assert!(matches!(
            new_state.inner().state(),
            NarState::Resolved { .. }
        ));
    }

    #[test]
    fn on_all_outcomes_acquired_picks_higher_preference() {
        let state = make_state();
        let sub_a = make_substituter("https://cache-a.example.com", 1);
        let sub_b = make_substituter("https://cache-b.example.com", 100);
        let data = make_nar_info_data();
        let outcomes = vec![
            Ok(NarInfoQueryOutcome::Found {
                original_data: data.clone(),
                latency: Duration::from_millis(10),
            }),
            Ok(NarInfoQueryOutcome::Found {
                original_data: data.clone(),
                latency: Duration::from_millis(100),
            }),
        ];
        let substituters = vec![sub_a.clone(), sub_b];

        let (_effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters, true);

        match new_state.inner().state() {
            NarState::Resolved { best, .. } => assert_eq!(best.url(), sub_a.url()),
            _ => panic!("expected Resolved state"),
        }
    }

    #[test]
    fn on_all_outcomes_acquired_remains_empty_when_all_not_found() {
        let state = make_state();
        let outcomes = vec![Ok(NarInfoQueryOutcome::NotFound)];
        let substituters = vec![make_substituter("https://cache.nixos.org", 40)];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters, true);

        assert!(matches!(new_state.inner().state(), NarState::NotFound));
        assert!(effects.is_empty());
    }

    #[test]
    fn on_all_outcomes_acquired_remains_unknown_when_all_failed() {
        let state = make_state();
        let outcomes = vec![Err(anyhow::anyhow!("timeout"))];
        let substituters = vec![make_substituter("https://cache.nixos.org", 40)];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters, true);

        assert!(matches!(new_state.inner().state(), NarState::Unknown));
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            NarActorEffect::ReportSubstituterFailure(_)
        ));
    }

    #[test]
    fn on_all_outcomes_acquired_picks_found_given_mixed_results() {
        let state = make_state();
        let sub_a = make_substituter("https://cache-a.example.com", 40);
        let sub_b = make_substituter("https://cache-b.example.com", 40);
        let data = make_nar_info_data();
        let outcomes = vec![
            Ok(NarInfoQueryOutcome::Found {
                original_data: data.clone(),
                latency: Duration::from_millis(50),
            }),
            Err(anyhow::anyhow!("connection refused")),
        ];
        let substituters = vec![sub_a, sub_b];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters, true);

        assert!(matches!(
            new_state.inner().state(),
            NarState::Resolved { .. }
        ));
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            NarActorEffect::ReportSubstituterFailure(_)
        ));
    }

    #[test]
    fn on_all_outcomes_acquired_remains_unknown_given_mixed_not_found_and_error() {
        let state = make_state();
        let sub_a = make_substituter("https://cache-a.example.com", 40);
        let sub_b = make_substituter("https://cache-b.example.com", 40);
        let outcomes = vec![
            Ok(NarInfoQueryOutcome::NotFound),
            Err(anyhow::anyhow!("connection refused")),
        ];
        let substituters = vec![sub_a, sub_b];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters, true);

        assert!(matches!(new_state.inner().state(), NarState::Unknown));
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            NarActorEffect::ReportSubstituterFailure(_)
        ));
    }

    #[test]
    fn on_all_outcomes_acquired_generates_success_effect_given_unavailable_substituter() {
        let state = make_state();
        let sub = make_unavailable_substituter("https://cache.nixos.org", 40);
        let data = make_nar_info_data();
        let outcomes = vec![Ok(NarInfoQueryOutcome::Found {
            original_data: data.clone(),
            latency: Duration::from_millis(100),
        })];
        let substituters = vec![sub.clone()];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters, true);

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            NarActorEffect::ReportSubstituterSuccess(sub.url().clone())
        );
        assert!(matches!(
            new_state.inner().state(),
            NarState::Resolved { .. }
        ));
    }

    #[test]
    fn on_all_outcomes_acquired_preserves_original_url_given_rewrite_false() {
        let state = make_state();
        let sub = make_substituter("https://other.com", 40);
        let data = make_nar_info_data_with_external_url();
        let outcomes = vec![Ok(NarInfoQueryOutcome::Found {
            original_data: data.clone(),
            latency: Duration::from_millis(100),
        })];
        let substituters = vec![sub];

        let (_effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters, false);

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
    fn on_all_outcomes_acquired_rewrites_url_given_rewrite_true() {
        let state = make_state();
        let sub = make_substituter("https://other.com", 40);
        let data = make_nar_info_data_with_external_url();
        let outcomes = vec![Ok(NarInfoQueryOutcome::Found {
            original_data: data.clone(),
            latency: Duration::from_millis(100),
        })];
        let substituters = vec![sub];

        let (_effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters, true);

        match new_state.inner().state() {
            NarState::Resolved { nar_info, .. } => {
                assert!(nar_info.content().contains("URL: nar/abc.nar.xz\n"));
                assert!(!nar_info.content().contains("https://other.com"));
            }
            _ => panic!("expected Resolved state"),
        }
    }
}
