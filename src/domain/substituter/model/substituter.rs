use getset::Getters;
use tokio::time::Instant;

use crate::domain::substituter::model::{Availability, Priority, SubstituterMeta, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters)]
pub struct Substituter {
    #[getset(get = "pub")]
    target: SubstituterMeta,
    #[getset(get = "pub")]
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

    pub fn is_unavailable(&self) -> bool {
        matches!(&self.availability, Availability::Unavailable { .. })
    }

    pub fn on_detected_unavailable(mut self, now: Instant) -> (Instant, Self) {
        self.availability = self.availability.change_to_unavailable(now);
        let retry_instant = now + self.availability.retry_duration().unwrap();
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
    fn on_detected_unavailable_returns_retry_instant() {
        let sub = make_substituter();
        let now = Instant::now();
        let (retry, _) = sub.on_detected_unavailable(now);
        assert_eq!(retry, now + Duration::from_millis(500));
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
}
