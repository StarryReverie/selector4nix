use std::time::Duration;

use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Availability {
    Normal,
    Unavailable {
        detected_at: Instant,
        prev_failures: usize,
    },
    MaybeReady {
        prev_failures: usize,
    },
}

impl Availability {
    pub fn normal() -> Self {
        Self::Normal
    }

    pub fn change_to_unavailable(self, now: Instant) -> Self {
        match self {
            Self::Normal => Self::Unavailable {
                detected_at: now,
                prev_failures: 0,
            },
            Self::Unavailable { prev_failures, .. } => Self::Unavailable {
                detected_at: now,
                prev_failures: prev_failures + 1,
            },
            Self::MaybeReady { prev_failures } => Self::Unavailable {
                detected_at: now,
                prev_failures: prev_failures + 1,
            },
        }
    }

    pub fn change_to_maybe_ready(self) -> Self {
        match self {
            Self::Unavailable { prev_failures, .. } => Self::MaybeReady { prev_failures },
            otherwise => otherwise,
        }
    }

    pub fn update_and_check_availability(self, now: Instant) -> (bool, Self) {
        match &self {
            Availability::Unavailable {
                detected_at,
                prev_failures,
            } => {
                if *detected_at + Self::calc_retry_duration(*prev_failures) <= now {
                    (true, self.change_to_maybe_ready())
                } else {
                    (false, self)
                }
            }
            _ => (true, self),
        }
    }

    pub fn retry_duration(&self) -> Option<Duration> {
        match self {
            Self::Unavailable { prev_failures, .. } => {
                Some(Self::calc_retry_duration(*prev_failures))
            }
            _ => None,
        }
    }

    fn calc_retry_duration(prev_failures: usize) -> Duration {
        const BASE_RETRY_DURATION: u64 = 500;
        let exp = prev_failures.min(10) as u32;
        let multiplier = 2u32.saturating_pow(exp);
        Duration::from_millis(BASE_RETRY_DURATION) * multiplier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_to_unavailable_succeeds_from_normal() {
        let now = Instant::now();
        let result = Availability::Normal.change_to_unavailable(now);
        assert_eq!(
            result,
            Availability::Unavailable {
                detected_at: now,
                prev_failures: 0,
            }
        );
    }

    #[test]
    fn change_to_unavailable_increments_prev_failures() {
        let now = Instant::now();
        let state = Availability::Unavailable {
            detected_at: now,
            prev_failures: 1,
        };
        let result = state.change_to_unavailable(now);
        assert_eq!(
            result,
            Availability::Unavailable {
                detected_at: now,
                prev_failures: 2,
            }
        );
    }

    #[test]
    fn update_and_check_availability_returns_true_given_normal() {
        let (available, new_state) =
            Availability::Normal.update_and_check_availability(Instant::now());
        assert!(available);
        assert_eq!(new_state, Availability::Normal);
    }

    #[test]
    fn update_and_check_availability_returns_false_when_unavailable_before_timeout() {
        let now = Instant::now();
        let state = Availability::Unavailable {
            detected_at: now,
            prev_failures: 0,
        };
        let (available, _) = state.update_and_check_availability(now);
        assert!(!available);
    }

    #[test]
    fn update_and_check_availability_returns_true_when_unavailable_after_timeout() {
        let now = Instant::now();
        let state = Availability::Unavailable {
            detected_at: now,
            prev_failures: 0,
        };
        let retry_at = now + Duration::from_millis(500);
        let (available, new_state) = state.update_and_check_availability(retry_at);
        assert!(available);
        assert_eq!(new_state, Availability::MaybeReady { prev_failures: 0 });
    }
}
