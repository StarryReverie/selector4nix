use std::time::Duration;

use anyhow::Result as AnyhowResult;

use crate::domain::nar::model::{Nar, NarInfoQueryOutcome};
use crate::domain::substituter::model::{Priority, SubstituterMeta, Url};

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
        substituters: &[SubstituterMeta],
    ) -> (Vec<NarActorEffect>, Self) {
        let mut effects = Vec::new();
        let mut optimal = None;

        for (outcome, substituter) in outcomes.iter().zip(substituters.iter()) {
            match outcome {
                Ok(NarInfoQueryOutcome::Found { data, latency }) => {
                    let preference = Self::calc_preference(*latency, substituter.priority());
                    optimal = match optimal {
                        prev @ Some((_, prev_preference)) if prev_preference > preference => prev,
                        _ => Some(((data, substituter), preference)),
                    };
                    let url = substituter.url().clone();
                    effects.push(NarActorEffect::ReportSubstituterSuccess(url));
                }
                Ok(NarInfoQueryOutcome::NotFound) => {
                    let url = substituter.url().clone();
                    effects.push(NarActorEffect::ReportSubstituterSuccess(url));
                }
                Err(_) => {
                    let url = substituter.url().clone();
                    effects.push(NarActorEffect::ReportSubstituterFailure(url));
                }
            }
        }

        match optimal {
            Some(((nar_info, substituter), _)) => {
                let nar = nar.on_resolved(substituter.clone(), nar_info.clone());
                (effects, Self::new(nar))
            }
            None => (effects, Self::new(nar)),
        }
    }

    fn calc_preference(latency: Duration, priority: Priority) -> i64 {
        const TOLERANCE: i64 = 50;
        TOLERANCE * (priority.value() + 1).ilog2() as i64 - latency.as_millis() as i64
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::nar::model::{
        Nar, NarInfoData, NarInfoQueryOutcome, NarState, StorePathHash,
    };
    use crate::domain::substituter::model::{Priority, SubstituterMeta, Url};

    use super::*;

    fn make_state() -> NarActorState {
        let hash = StorePathHash::new("p4pclmv1gyja5kzc26npqpia1qqxrf0l".into()).unwrap();
        NarActorState::new(Nar::new(hash))
    }

    fn make_meta(url: &str, priority: u32) -> SubstituterMeta {
        SubstituterMeta::new(Url::new(url).unwrap(), Priority::new(priority).unwrap())
    }

    fn make_nar_info_data() -> NarInfoData {
        NarInfoData::new(
            "StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-hello\nURL: nar/abc.nar.xz\n"
                .into(),
        )
        .unwrap()
    }

    #[test]
    fn on_all_outcomes_acquired_succeeds_given_single_found() {
        let state = make_state();
        let meta = make_meta("https://cache.nixos.org", 40);
        let data = make_nar_info_data();
        let outcomes = vec![Ok(NarInfoQueryOutcome::Found {
            data: data.clone(),
            latency: Duration::from_millis(100),
        })];
        let substituters = vec![meta.clone()];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters);

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            NarActorEffect::ReportSubstituterSuccess(meta.url().clone())
        );
        assert!(matches!(
            new_state.inner().state(),
            NarState::Resolved { .. }
        ));
    }

    #[test]
    fn on_all_outcomes_acquired_picks_higher_preference() {
        let state = make_state();
        let meta_a = make_meta("https://cache-a.example.com", 1);
        let meta_b = make_meta("https://cache-b.example.com", 100);
        let data = make_nar_info_data();
        let outcomes = vec![
            Ok(NarInfoQueryOutcome::Found {
                data: data.clone(),
                latency: Duration::from_millis(10),
            }),
            Ok(NarInfoQueryOutcome::Found {
                data: data.clone(),
                latency: Duration::from_millis(100),
            }),
        ];
        let substituters = vec![meta_a, meta_b.clone()];

        let (_effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters);

        match new_state.inner().state() {
            NarState::Resolved { best, .. } => assert_eq!(best.url(), meta_b.url()),
            _ => panic!("expected Resolved state"),
        }
    }

    #[test]
    fn on_all_outcomes_acquired_remains_empty_when_all_not_found() {
        let state = make_state();
        let outcomes = vec![Ok(NarInfoQueryOutcome::NotFound)];
        let substituters = vec![make_meta("https://cache.nixos.org", 40)];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters);

        assert!(matches!(new_state.inner().state(), NarState::Empty));
        assert_eq!(effects.len(), 1);
    }

    #[test]
    fn on_all_outcomes_acquired_remains_empty_when_all_failed() {
        let state = make_state();
        let outcomes = vec![Err(anyhow::anyhow!("timeout"))];
        let substituters = vec![make_meta("https://cache.nixos.org", 40)];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters);

        assert!(matches!(new_state.inner().state(), NarState::Empty));
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            NarActorEffect::ReportSubstituterFailure(_)
        ));
    }

    #[test]
    fn on_all_outcomes_acquired_picks_found_given_mixed_results() {
        let state = make_state();
        let meta_a = make_meta("https://cache-a.example.com", 40);
        let meta_b = make_meta("https://cache-b.example.com", 40);
        let data = make_nar_info_data();
        let outcomes = vec![
            Ok(NarInfoQueryOutcome::Found {
                data: data.clone(),
                latency: Duration::from_millis(50),
            }),
            Err(anyhow::anyhow!("connection refused")),
        ];
        let substituters = vec![meta_a.clone(), meta_b];

        let (effects, new_state) =
            NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters);

        assert!(matches!(
            new_state.inner().state(),
            NarState::Resolved { .. }
        ));
        assert_eq!(effects.len(), 2);
        assert!(matches!(
            effects[0],
            NarActorEffect::ReportSubstituterSuccess(_)
        ));
        assert!(matches!(
            effects[1],
            NarActorEffect::ReportSubstituterFailure(_)
        ));
    }
}
