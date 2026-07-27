use std::time::Duration;

use tokio::time::Instant;

use crate::domain::substituter::model::{
    PeriodicProbingOption, ProbedState, Substituter, UpdateSubstituterEvent,
};
use crate::domain::substituter::port::ProbeSubstituterError;

pub struct SubstituterService {
    periodic_probing: PeriodicProbingOption,
}

impl SubstituterService {
    pub fn new(periodic_probing: PeriodicProbingOption) -> Self {
        Self { periodic_probing }
    }

    pub fn on_initial(&self, now: Instant) -> Vec<UpdateSubstituterEvent> {
        const INITIAL_PROBING_DELAY: Duration = Duration::from_secs(5);
        if self.periodic_probing == PeriodicProbingOption::Enabled {
            vec![UpdateSubstituterEvent::ScheduleProbing(Some(
                now + INITIAL_PROBING_DELAY,
            ))]
        } else {
            Vec::new()
        }
    }

    pub fn update_on_probing_finished(
        &self,
        substituter: Substituter,
        result: Result<(), ProbeSubstituterError>,
        now: Instant,
    ) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        let probed_state = match result {
            Ok(()) => ProbedState::Normal,
            Err(ProbeSubstituterError::Offline { .. }) => ProbedState::Offline,
            Err(ProbeSubstituterError::Service { .. }) => ProbedState::ServiceError,
        };
        substituter.update_on_probing_finished(probed_state, self.periodic_probing, now)
    }
}
