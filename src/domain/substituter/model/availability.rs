use std::time::Duration;

use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Availability {
    Normal,
    Offline {
        detected_at: Instant,
    },
    ServiceError {
        detected_at: Instant,
        prev_failures: usize,
    },
    MaybeReady {
        prev_failures: usize,
    },
}

impl Availability {
    pub const OFFLINE_RETRY_PERIOD: Duration = Duration::from_secs(30);
    pub const REPROBING_PERIOD: Duration = Duration::from_secs(30);

    pub fn change_to_normal(self) -> Self {
        match self {
            Self::MaybeReady { .. } => Self::Normal,
            otherwise => otherwise,
        }
    }

    pub fn change_to_offline(self, now: Instant) -> Self {
        match self {
            Self::Normal => Self::Offline { detected_at: now },
            s @ Self::Offline { .. } => s,
            s @ Self::ServiceError { .. } => s,
            Self::MaybeReady { .. } => Self::Offline { detected_at: now },
        }
    }

    pub fn change_to_service_error(self, now: Instant) -> Self {
        match self {
            Self::Normal => Self::ServiceError {
                detected_at: now,
                prev_failures: 0,
            },
            s @ Self::Offline { .. } => s,
            s @ Self::ServiceError { .. } => s,
            Self::MaybeReady { prev_failures } => Self::ServiceError {
                detected_at: now,
                prev_failures: prev_failures + 1,
            },
        }
    }

    pub fn change_to_maybe_ready(self) -> Self {
        match self {
            Self::Offline { .. } => Self::MaybeReady { prev_failures: 0 },
            Self::ServiceError { prev_failures, .. } => Self::MaybeReady { prev_failures },
            otherwise => otherwise,
        }
    }

    pub fn update_and_check_availability(self, now: Instant) -> (bool, Self) {
        match &self {
            Self::Offline { detected_at } => {
                if *detected_at + Self::OFFLINE_RETRY_PERIOD <= now {
                    (true, self.change_to_maybe_ready())
                } else {
                    (false, self)
                }
            }
            Self::ServiceError {
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
            Self::Offline { .. } => Some(Self::OFFLINE_RETRY_PERIOD),
            Self::ServiceError { prev_failures, .. } => {
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

    pub fn prev_failures(&self) -> usize {
        match self {
            Self::Normal => 0,
            Self::Offline { .. } => 0,
            Self::ServiceError { prev_failures, .. } => *prev_failures,
            Self::MaybeReady { prev_failures } => *prev_failures,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_to_service_error_succeeds_from_normal() {
        let now = Instant::now();
        let result = Availability::Normal.change_to_service_error(now);
        assert_eq!(
            result,
            Availability::ServiceError {
                detected_at: now,
                prev_failures: 0,
            }
        );
    }

    #[test]
    fn change_to_service_error_doesnt_change_prev_failures() {
        let now = Instant::now();
        let state = Availability::ServiceError {
            detected_at: now,
            prev_failures: 1,
        };
        let result = state.change_to_service_error(now);
        assert_eq!(
            result,
            Availability::ServiceError {
                detected_at: now,
                prev_failures: 1,
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
    fn update_and_check_availability_returns_false_when_service_error_before_timeout() {
        let now = Instant::now();
        let state = Availability::ServiceError {
            detected_at: now,
            prev_failures: 0,
        };
        let (available, _) = state.update_and_check_availability(now);
        assert!(!available);
    }

    #[test]
    fn update_and_check_availability_returns_true_when_service_error_after_timeout() {
        let now = Instant::now();
        let state = Availability::ServiceError {
            detected_at: now,
            prev_failures: 0,
        };
        let retry_at = now + Duration::from_millis(500);
        let (available, new_state) = state.update_and_check_availability(retry_at);
        assert!(available);
        assert_eq!(new_state, Availability::MaybeReady { prev_failures: 0 });
    }

    #[test]
    fn update_and_check_availability_returns_true_when_offline_before_timeout() {
        let now = Instant::now();
        let state = Availability::Offline { detected_at: now };
        let retry_at = now + Availability::OFFLINE_RETRY_PERIOD - Duration::from_millis(1);
        let (available, _) = state.update_and_check_availability(retry_at);
        assert!(!available);
    }

    #[test]
    fn update_and_check_availability_returns_true_when_offline_after_timeout() {
        let now = Instant::now();
        let state = Availability::Offline { detected_at: now };
        let retry_at = now + Availability::OFFLINE_RETRY_PERIOD + Duration::from_millis(1);
        let (available, new_state) = state.update_and_check_availability(retry_at);
        assert!(available);
        assert_eq!(new_state, Availability::MaybeReady { prev_failures: 0 });
    }
}
