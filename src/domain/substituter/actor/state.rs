use tokio::time::Instant;

use crate::domain::substituter::model::Substituter;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubstituterActorEffect {
    ScheduleRetryReady(Instant),
    NotifyUnavailable,
    NotifyAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubstituterActorState(Substituter);

impl SubstituterActorState {
    pub fn new(substituter: Substituter) -> Self {
        Self(substituter)
    }

    pub fn inner(&self) -> &Substituter {
        &self.0
    }

    pub fn on_service_successful(Self(substituter): Self) -> Self {
        Self::new(substituter.on_detected_normal())
    }

    pub fn on_service_failed(
        Self(substituter): Self,
        now: Instant,
    ) -> (Vec<SubstituterActorEffect>, Self) {
        if substituter.is_unavailable() {
            (Vec::new(), Self::new(substituter))
        } else {
            let (retry_instant, substituter) = substituter.on_detected_unavailable(now);
            let effects = vec![
                SubstituterActorEffect::NotifyUnavailable,
                SubstituterActorEffect::ScheduleRetryReady(retry_instant),
            ];
            (effects, Self::new(substituter))
        }
    }

    pub fn on_next_retry_ready(Self(substituter): Self) -> (Vec<SubstituterActorEffect>, Self) {
        let effects = vec![SubstituterActorEffect::NotifyAvailable];
        (effects, Self::new(substituter.on_next_retry_ready()))
    }
}

impl From<Substituter> for SubstituterActorState {
    fn from(substituter: Substituter) -> Self {
        Self::new(substituter)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::substituter::model::{Availability, Priority, SubstituterMeta, Url};

    use super::*;

    fn make_state(availability: Availability) -> SubstituterActorState {
        let url = Url::new("https://cache.nixos.org").unwrap();
        let priority = Priority::new(40).unwrap();
        let meta = SubstituterMeta::new(url, priority);
        SubstituterActorState::new(Substituter::new(meta, availability))
    }

    #[test]
    fn on_service_successful_succeeds() {
        let state = make_state(Availability::MaybeReady { prev_failures: 0 });
        let result = SubstituterActorState::on_service_successful(state);
        assert!(!result.inner().is_unavailable());
    }

    #[test]
    fn on_service_failed_succeeds() {
        let state = make_state(Availability::Normal);
        let now = Instant::now();

        let (effects, new_state) = SubstituterActorState::on_service_failed(state, now);

        assert!(new_state.inner().is_unavailable());
        assert_eq!(effects.len(), 2);
        assert!(matches!(
            effects[0],
            SubstituterActorEffect::NotifyUnavailable
        ));
        assert!(matches!(
            effects[1],
            SubstituterActorEffect::ScheduleRetryReady(t) if t == now + Duration::from_millis(500)
        ));
    }

    #[test]
    fn on_service_failed_increments_backoff_given_repeated() {
        let state = make_state(Availability::MaybeReady { prev_failures: 2 });
        let now = Instant::now();

        let (effects, new_state) = SubstituterActorState::on_service_failed(state, now);

        assert!(new_state.inner().is_unavailable());
        assert_eq!(effects.len(), 2);
        assert!(matches!(
            new_state.inner().availability(),
            Availability::Unavailable {
                prev_failures: 3,
                ..
            }
        ));
    }

    #[test]
    fn on_next_retry_ready_succeeds() {
        let state = make_state(Availability::Unavailable {
            detected_at: Instant::now(),
            prev_failures: 0,
        });

        let (effects, new_state) = SubstituterActorState::on_next_retry_ready(state);

        assert!(!new_state.inner().is_unavailable());
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            SubstituterActorEffect::NotifyAvailable
        ));
    }
}
