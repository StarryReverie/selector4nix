use getset::Getters;
use tokio::time::Instant;

use crate::domain::substituter::model::{Availability, Priority, SubstituterMeta, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters)]
#[getset(get = "pub")]
pub struct Substituter {
    target: SubstituterMeta,
    availability: Availability,
}

impl Substituter {
    pub fn new(target: SubstituterMeta, availability: Availability) -> Self {
        Self {
            target,
            availability,
        }
    }

    pub fn url(&self) -> &Url {
        self.target.url()
    }

    pub fn priority(&self) -> Priority {
        self.target.priority()
    }

    pub fn prev_failures(&self) -> usize {
        self.availability.prev_failures()
    }

    pub fn is_normal(&self) -> bool {
        matches!(&self.availability, Availability::Normal)
    }

    pub fn is_maybe_ready(&self) -> bool {
        matches!(&self.availability, Availability::MaybeReady { .. })
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(
            &self.availability,
            Availability::Offline { .. } | Availability::ServiceError { .. },
        )
    }

    pub fn update_on_service_successful(mut self) -> (Self, Vec<UpdateSubstituterEvent>) {
        self.availability = self.availability.try_change_to_normal();
        let events = if !self.is_unavailable() {
            vec![UpdateSubstituterEvent::NotifyAvailable]
        } else {
            Vec::new()
        };
        (self, events)
    }

    pub fn update_on_service_offline(
        mut self,
        now: Instant,
    ) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        if self.is_unavailable() {
            (self, Vec::new())
        } else {
            self.availability = self.availability.try_change_to_offline(now);
            let retry_instant = now + self.availability.retry_duration().unwrap();
            let events = vec![
                UpdateSubstituterEvent::NotifyUnavailable,
                UpdateSubstituterEvent::ScheduleRetryReady(retry_instant),
            ];
            (self, events)
        }
    }

    pub fn update_on_service_error(
        mut self,
        now: Instant,
    ) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        if self.is_unavailable() {
            (self, Vec::new())
        } else {
            self.availability = self.availability.try_change_to_service_error(now);
            let retry_instant = now + self.availability.retry_duration().unwrap();
            let events = vec![
                UpdateSubstituterEvent::NotifyUnavailable,
                UpdateSubstituterEvent::ScheduleRetryReady(retry_instant),
            ];
            (self, events)
        }
    }

    pub fn update_on_next_retry_ready(mut self) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        self.availability = self.availability.try_change_to_maybe_ready();
        let events = vec![UpdateSubstituterEvent::ScheduleProbing(None)];
        (self, events)
    }

    pub fn update_on_probing_finished(
        mut self,
        probed_state: ProbedState,
        periodic_probing: PeriodicProbingOption,
        now: Instant,
    ) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        match probed_state {
            ProbedState::Normal => {
                if self.is_unavailable() {
                    (self, Vec::new())
                } else {
                    self.availability = self.availability.try_change_to_maybe_ready();
                    let events = match periodic_probing {
                        PeriodicProbingOption::Enabled => vec![
                            UpdateSubstituterEvent::NotifyAvailable,
                            UpdateSubstituterEvent::ScheduleProbing(Some(
                                now + Availability::REPROBING_PERIOD,
                            )),
                        ],
                        PeriodicProbingOption::None => {
                            vec![UpdateSubstituterEvent::NotifyAvailable]
                        }
                    };
                    (self, events)
                }
            }
            ProbedState::Offline => self.update_on_service_offline(now),
            ProbedState::ServiceError => self.update_on_service_error(now),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UpdateSubstituterEvent {
    ScheduleRetryReady(Instant),
    ScheduleProbing(Option<Instant>),
    NotifyUnavailable,
    NotifyAvailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProbedState {
    Normal,
    Offline,
    ServiceError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeriodicProbingOption {
    Enabled,
    None,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::Duration;

    use crate::domain::substituter::model::{Availability, Priority, SubstituterMeta, Url};

    use super::*;

    fn make_substituter(availability: Availability) -> Substituter {
        let url = Url::new("https://cache.nixos.org").unwrap();
        let priority = Priority::new(40).unwrap();
        let meta = SubstituterMeta::new(url, priority);
        Substituter::new(meta, availability)
    }

    fn assert_events_eq(
        actual: impl IntoIterator<Item = UpdateSubstituterEvent>,
        expected: impl IntoIterator<Item = UpdateSubstituterEvent>,
    ) {
        assert_eq!(
            actual.into_iter().collect::<HashSet<_>>(),
            expected.into_iter().collect::<HashSet<_>>(),
        );
    }

    #[test]
    fn update_on_service_successful_given_maybe_ready() {
        let substituter = make_substituter(Availability::MaybeReady { prev_failures: 0 });
        let (result, events) = substituter.update_on_service_successful();
        assert!(!result.is_unavailable());
        assert_events_eq(events, vec![UpdateSubstituterEvent::NotifyAvailable]);
    }

    #[test]
    fn update_on_service_failed_changes_state_from_normal() {
        let substituter = make_substituter(Availability::Normal);
        let now = Instant::now();

        let (result, events) = substituter.update_on_service_error(now);

        assert!(result.is_unavailable());
        assert_events_eq(
            events,
            vec![
                UpdateSubstituterEvent::NotifyUnavailable,
                UpdateSubstituterEvent::ScheduleRetryReady(now + Duration::from_millis(500)),
            ],
        );
    }

    #[test]
    fn update_on_service_error_increments_backoff_given_repeated_error() {
        let substituter = make_substituter(Availability::MaybeReady { prev_failures: 2 });
        let now = Instant::now();

        let (result, events) = substituter.update_on_service_error(now);

        assert!(result.is_unavailable());
        assert!(matches!(
            result.availability(),
            Availability::ServiceError {
                prev_failures: 3,
                ..
            }
        ));
        assert_events_eq(
            events,
            vec![
                UpdateSubstituterEvent::NotifyUnavailable,
                UpdateSubstituterEvent::ScheduleRetryReady(now + Duration::from_millis(4000)),
            ],
        );
    }

    #[test]
    fn update_on_next_retry_ready_succeeds() {
        let substituter = make_substituter(Availability::ServiceError {
            detected_at: Instant::now(),
            prev_failures: 0,
        });

        let (result, events) = substituter.update_on_next_retry_ready();

        assert!(!result.is_unavailable());
        assert_events_eq(events, vec![UpdateSubstituterEvent::ScheduleProbing(None)]);
    }

    #[test]
    fn update_on_probing_finished_succeeds_given_probed_state_normal() {
        let substituter = make_substituter(Availability::MaybeReady { prev_failures: 0 });
        let now = Instant::now();

        let (result, events) = substituter.update_on_probing_finished(
            ProbedState::Normal,
            PeriodicProbingOption::Enabled,
            now,
        );

        assert!(!result.is_unavailable());
        assert_events_eq(
            events,
            vec![
                UpdateSubstituterEvent::NotifyAvailable,
                UpdateSubstituterEvent::ScheduleProbing(Some(now + Availability::REPROBING_PERIOD)),
            ],
        );
    }

    #[test]
    fn update_on_probing_finished_schedules_reprobing_given_already_normal() {
        let substituter = make_substituter(Availability::Normal);
        let now = Instant::now();

        let (result, events) = substituter.update_on_probing_finished(
            ProbedState::Normal,
            PeriodicProbingOption::Enabled,
            now,
        );

        assert!(result.is_normal());
        assert_events_eq(
            events,
            vec![
                UpdateSubstituterEvent::NotifyAvailable,
                UpdateSubstituterEvent::ScheduleProbing(Some(now + Availability::REPROBING_PERIOD)),
            ],
        );
    }

    #[test]
    fn update_on_probing_finished_emits_unavailable_given_offline() {
        let substituter = make_substituter(Availability::MaybeReady { prev_failures: 0 });
        let now = Instant::now();

        let (result, events) = substituter.update_on_probing_finished(
            ProbedState::Offline,
            PeriodicProbingOption::Enabled,
            now,
        );

        assert!(result.is_unavailable());
        assert_events_eq(
            events,
            vec![
                UpdateSubstituterEvent::NotifyUnavailable,
                UpdateSubstituterEvent::ScheduleRetryReady(
                    now + Availability::OFFLINE_RETRY_PERIOD,
                ),
            ],
        );
    }

    #[test]
    fn update_on_probing_finished_emits_unavailable_given_service_error() {
        let substituter = make_substituter(Availability::MaybeReady { prev_failures: 2 });
        let now = Instant::now();

        let (result, events) = substituter.update_on_probing_finished(
            ProbedState::ServiceError,
            PeriodicProbingOption::Enabled,
            now,
        );

        assert!(result.is_unavailable());
        assert_events_eq(
            events,
            vec![
                UpdateSubstituterEvent::NotifyUnavailable,
                UpdateSubstituterEvent::ScheduleRetryReady(now + Duration::from_millis(4000)),
            ],
        );
    }
}
