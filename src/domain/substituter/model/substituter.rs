use getset::Getters;
use tokio::time::Instant;

use crate::domain::substituter::model::{Availability, Priority, SubstituterMeta, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters)]
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

    pub fn is_unavailable(&self) -> bool {
        matches!(&self.availability, Availability::Unavailable { .. })
    }

    pub fn on_detected_unavailable(mut self, now: Instant) -> (NextRetryInstant, Self) {
        self.availability = self.availability.change_to_unavailable(now);
        let retry_instant = self.next_retry_instant();
        (retry_instant, self)
    }

    pub fn on_next_retry_ready(mut self) -> Self {
        self.availability = self.availability.change_to_maybe_ready();
        self
    }

    pub fn on_detected_normal(mut self) -> Self {
        self.availability = Availability::Normal;
        self
    }

    pub fn next_retry_instant(&self) -> NextRetryInstant {
        match &self.availability {
            Availability::Unavailable { detected_at, .. } => {
                let duration = self.availability.retry_duration().unwrap();
                NextRetryInstant::Future(*detected_at + duration)
            }
            _ => NextRetryInstant::Immediate,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextRetryInstant {
    Immediate,
    Future(Instant),
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn make_substituter() -> Substituter {
        let url = Url::new("https://cache.nixos.org").unwrap();
        let priority = Priority::new(40).unwrap();
        Substituter::new(SubstituterMeta::new(url, priority), Availability::Normal)
    }

    #[test]
    fn on_detected_unavailable_transitions_to_unavailable() {
        let sub = make_substituter();
        assert!(!sub.is_unavailable());

        let (_, sub) = sub.on_detected_unavailable(Instant::now());
        assert!(sub.is_unavailable());
    }

    #[test]
    fn on_next_retry_ready_transitions_from_unavailable() {
        let sub = make_substituter();
        let (_, sub) = sub.on_detected_unavailable(Instant::now());
        assert!(sub.is_unavailable());

        let sub = sub.on_next_retry_ready();
        assert!(!sub.is_unavailable());
    }

    #[test]
    fn on_detected_normal_resets_availability() {
        let sub = make_substituter();
        let (_, sub) = sub.on_detected_unavailable(Instant::now());
        assert!(sub.is_unavailable());

        let sub = sub.on_detected_normal();
        assert!(!sub.is_unavailable());
    }

    #[test]
    fn next_retry_instant_returns_immediate_given_normal() {
        let sub = make_substituter();
        assert_eq!(sub.next_retry_instant(), NextRetryInstant::Immediate);
    }

    #[test]
    fn next_retry_instant_returns_future_given_unavailable() {
        let sub = make_substituter();
        let now = Instant::now();
        let (retry, sub) = sub.on_detected_unavailable(now);

        let expected = NextRetryInstant::Future(now + Duration::from_millis(500));
        assert_eq!(retry, expected);
        assert_eq!(sub.next_retry_instant(), expected);
    }
}
