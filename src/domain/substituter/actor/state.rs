use tokio::time::Instant;

use crate::domain::substituter::model::Substituter;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubstituterActorEffect {
    ScheduleRetryReady(Instant),
    NotifyUnavailable,
    NotifyMaybeReady,
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
        let (retry_instant, substituter) = substituter.on_detected_unavailable(now);
        let effects = vec![
            SubstituterActorEffect::NotifyUnavailable,
            SubstituterActorEffect::ScheduleRetryReady(retry_instant),
        ];
        (effects, Self::new(substituter))
    }

    pub fn on_next_retry_ready(Self(subtituter): Self) -> (Vec<SubstituterActorEffect>, Self) {
        let effects = vec![SubstituterActorEffect::NotifyMaybeReady];
        (effects, Self::new(subtituter.on_next_retry_ready()))
    }
}
