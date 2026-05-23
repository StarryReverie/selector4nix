use std::time::Duration;

use tokio::time::Instant;

use crate::domain::substituter::model::{Availability, Substituter};
use crate::domain::substituter::port::ProbeSubstituterError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UpdateSubstituterEvent {
    ScheduleRetryReady(Instant),
    ScheduleProbing(Option<Instant>),
    NotifyUnavailable,
    NotifyAvailable,
}

pub struct SubstituterService {
    periodic_probing: bool,
}

impl SubstituterService {
    pub fn new(periodic_probing: bool) -> Self {
        Self { periodic_probing }
    }

    pub fn on_initial(&self, now: Instant) -> Vec<UpdateSubstituterEvent> {
        const INITIAL_PROBING_DELAY: Duration = Duration::from_secs(5);
        if self.periodic_probing {
            vec![UpdateSubstituterEvent::ScheduleProbing(Some(
                now + INITIAL_PROBING_DELAY,
            ))]
        } else {
            Vec::new()
        }
    }

    pub fn update_on_service_successful(
        &self,
        substituter: Substituter,
    ) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        (substituter.on_detected_normal(), Vec::new())
    }

    pub fn update_on_service_offline(
        &self,
        substituter: Substituter,
        now: Instant,
    ) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        if substituter.is_unavailable() {
            (substituter, Vec::new())
        } else {
            let (retry_instant, substituter) = substituter.on_detected_offline(now);
            let events = vec![
                UpdateSubstituterEvent::NotifyUnavailable,
                UpdateSubstituterEvent::ScheduleRetryReady(retry_instant),
            ];
            (substituter, events)
        }
    }

    pub fn update_on_service_error(
        &self,
        substituter: Substituter,
        now: Instant,
    ) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        if substituter.is_unavailable() {
            (substituter, Vec::new())
        } else {
            let (retry_instant, substituter) = substituter.on_detected_service_error(now);
            let events = vec![
                UpdateSubstituterEvent::NotifyUnavailable,
                UpdateSubstituterEvent::ScheduleRetryReady(retry_instant),
            ];
            (substituter, events)
        }
    }

    pub fn update_on_next_retry_ready(
        &self,
        substituter: Substituter,
    ) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        let events = vec![UpdateSubstituterEvent::ScheduleProbing(None)];
        (substituter.on_next_retry_ready(), events)
    }

    pub fn update_on_probing_finished(
        &self,
        substituter: Substituter,
        result: Result<(), ProbeSubstituterError>,
        now: Instant,
    ) -> (Substituter, Vec<UpdateSubstituterEvent>) {
        match result {
            Ok(()) => {
                let substituter = substituter.on_detected_normal();
                let events = match (substituter.is_normal(), self.periodic_probing) {
                    (true, true) => vec![
                        UpdateSubstituterEvent::NotifyAvailable,
                        UpdateSubstituterEvent::ScheduleProbing(Some(
                            now + Availability::REPROBING_PERIOD,
                        )),
                    ],
                    (true, false) => vec![UpdateSubstituterEvent::NotifyAvailable],
                    (false, _) => Vec::new(),
                };
                (substituter, events)
            }
            Err(ProbeSubstituterError::Offline { .. }) => {
                self.update_on_service_offline(substituter, now)
            }
            Err(ProbeSubstituterError::Service { .. }) => {
                self.update_on_service_error(substituter, now)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::substituter::model::{Availability, Priority, SubstituterMeta, Url};

    use super::*;

    fn make_substituter(availability: Availability) -> Substituter {
        let url = Url::new("https://cache.nixos.org").unwrap();
        let priority = Priority::new(40).unwrap();
        let meta = SubstituterMeta::new(url, priority);
        Substituter::new(meta, availability)
    }

    #[test]
    fn update_on_service_successful_succeeds() {
        let service = SubstituterService::new(true);
        let sub = make_substituter(Availability::MaybeReady { prev_failures: 0 });
        let (result, events) = service.update_on_service_successful(sub);
        assert!(!result.is_unavailable());
        assert!(events.is_empty());
    }

    #[test]
    fn update_on_service_failed_succeeds() {
        let service = SubstituterService::new(true);
        let sub = make_substituter(Availability::Normal);
        let now = Instant::now();

        let (result, events) = service.update_on_service_error(sub, now);

        assert!(result.is_unavailable());
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            UpdateSubstituterEvent::NotifyUnavailable
        ));
        assert!(matches!(
            events[1],
            UpdateSubstituterEvent::ScheduleRetryReady(t) if t == now + Duration::from_millis(500)
        ));
    }

    #[test]
    fn update_on_service_failed_increments_backoff_given_repeated() {
        let service = SubstituterService::new(true);
        let sub = make_substituter(Availability::MaybeReady { prev_failures: 2 });
        let now = Instant::now();

        let (result, events) = service.update_on_service_error(sub, now);

        assert!(result.is_unavailable());
        assert_eq!(events.len(), 2);
        assert!(matches!(
            result.availability(),
            Availability::ServiceError {
                prev_failures: 3,
                ..
            }
        ));
    }

    #[test]
    fn update_on_next_retry_ready_succeeds() {
        let service = SubstituterService::new(true);
        let sub = make_substituter(Availability::ServiceError {
            detected_at: Instant::now(),
            prev_failures: 0,
        });

        let (result, events) = service.update_on_next_retry_ready(sub);

        assert!(!result.is_unavailable());
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            UpdateSubstituterEvent::ScheduleProbing(None),
        ));
    }

    #[test]
    fn update_on_probing_finished_succeeds_given_ok() {
        let service = SubstituterService::new(true);
        let sub = make_substituter(Availability::MaybeReady { prev_failures: 0 });
        let now = Instant::now();

        let (result, events) = service.update_on_probing_finished(sub, Ok(()), now);

        assert!(result.is_normal());
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], UpdateSubstituterEvent::NotifyAvailable,));
        assert!(matches!(
            events[1],
            UpdateSubstituterEvent::ScheduleProbing(Some(_)),
        ));
    }

    #[test]
    fn update_on_probing_finished_schedules_reprobing_given_ok_and_already_normal() {
        let service = SubstituterService::new(true);
        let sub = make_substituter(Availability::Normal);
        let now = Instant::now();

        let (result, events) = service.update_on_probing_finished(sub, Ok(()), now);

        assert!(result.is_normal());
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], UpdateSubstituterEvent::NotifyAvailable,));
        assert!(matches!(
            events[1],
            UpdateSubstituterEvent::ScheduleProbing(Some(_)),
        ));
    }

    #[test]
    fn update_on_probing_finished_emits_unavailable_given_offline() {
        let service = SubstituterService::new(true);
        let sub = make_substituter(Availability::MaybeReady { prev_failures: 0 });
        let now = Instant::now();
        let err = ProbeSubstituterError::Offline {
            source: anyhow::anyhow!("test"),
        };

        let (result, events) = service.update_on_probing_finished(sub, Err(err), now);

        assert!(result.is_unavailable());
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            UpdateSubstituterEvent::NotifyUnavailable,
        ));
        assert!(matches!(
            events[1],
            UpdateSubstituterEvent::ScheduleRetryReady(_),
        ));
    }

    #[test]
    fn update_on_probing_finished_emits_unavailable_given_service_error() {
        let service = SubstituterService::new(true);
        let sub = make_substituter(Availability::MaybeReady { prev_failures: 2 });
        let now = Instant::now();
        let err = ProbeSubstituterError::Service {
            source: anyhow::anyhow!("test"),
        };

        let (result, events) = service.update_on_probing_finished(sub, Err(err), now);

        assert!(result.is_unavailable());
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            UpdateSubstituterEvent::NotifyUnavailable,
        ));
        assert!(matches!(
            events[1],
            UpdateSubstituterEvent::ScheduleRetryReady(_),
        ));
    }
}
