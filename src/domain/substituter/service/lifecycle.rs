use tokio::time::Instant;

use crate::domain::substituter::model::Substituter;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubstituterLifecycleEvent {
    ScheduleRetryReady(Instant),
    NotifyUnavailable,
    NotifyAvailable,
}

pub struct SubstituterLifecycleService;

impl SubstituterLifecycleService {
    pub fn new() -> Self {
        Self
    }

    pub fn update_on_service_successful(
        &self,
        substituter: Substituter,
    ) -> (Substituter, Vec<SubstituterLifecycleEvent>) {
        (substituter.on_detected_normal(), Vec::new())
    }

    pub fn update_on_service_failed(
        &self,
        substituter: Substituter,
        now: Instant,
    ) -> (Substituter, Vec<SubstituterLifecycleEvent>) {
        if substituter.is_unavailable() {
            (substituter, Vec::new())
        } else {
            let (retry_instant, substituter) = substituter.on_detected_unavailable(now);
            let events = vec![
                SubstituterLifecycleEvent::NotifyUnavailable,
                SubstituterLifecycleEvent::ScheduleRetryReady(retry_instant),
            ];
            (substituter, events)
        }
    }

    pub fn update_on_next_retry_ready(
        &self,
        substituter: Substituter,
    ) -> (Substituter, Vec<SubstituterLifecycleEvent>) {
        let events = vec![SubstituterLifecycleEvent::NotifyAvailable];
        (substituter.on_next_retry_ready(), events)
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
        let service = SubstituterLifecycleService::new();
        let sub = make_substituter(Availability::MaybeReady { prev_failures: 0 });
        let (result, events) = service.update_on_service_successful(sub);
        assert!(!result.is_unavailable());
        assert!(events.is_empty());
    }

    #[test]
    fn update_on_service_failed_succeeds() {
        let service = SubstituterLifecycleService::new();
        let sub = make_substituter(Availability::Normal);
        let now = Instant::now();

        let (result, events) = service.update_on_service_failed(sub, now);

        assert!(result.is_unavailable());
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            SubstituterLifecycleEvent::NotifyUnavailable
        ));
        assert!(matches!(
            events[1],
            SubstituterLifecycleEvent::ScheduleRetryReady(t) if t == now + Duration::from_millis(500)
        ));
    }

    #[test]
    fn update_on_service_failed_increments_backoff_given_repeated() {
        let service = SubstituterLifecycleService::new();
        let sub = make_substituter(Availability::MaybeReady { prev_failures: 2 });
        let now = Instant::now();

        let (result, events) = service.update_on_service_failed(sub, now);

        assert!(result.is_unavailable());
        assert_eq!(events.len(), 2);
        assert!(matches!(
            result.availability(),
            Availability::Unavailable {
                prev_failures: 3,
                ..
            }
        ));
    }

    #[test]
    fn update_on_next_retry_ready_succeeds() {
        let service = SubstituterLifecycleService::new();
        let sub = make_substituter(Availability::Unavailable {
            detected_at: Instant::now(),
            prev_failures: 0,
        });

        let (result, events) = service.update_on_next_retry_ready(sub);

        assert!(!result.is_unavailable());
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            SubstituterLifecycleEvent::NotifyAvailable
        ));
    }
}
