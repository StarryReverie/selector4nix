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

    pub fn try_change_to_normal(self) -> Self {
        match self {
            Self::MaybeReady { .. } => Self::Normal,
            otherwise => otherwise,
        }
    }

    pub fn try_change_to_offline(self, now: Instant) -> Self {
        match self {
            Self::Normal => Self::Offline { detected_at: now },
            s @ Self::Offline { .. } => s,
            s @ Self::ServiceError { .. } => s,
            Self::MaybeReady { .. } => Self::Offline { detected_at: now },
        }
    }

    pub fn try_change_to_service_error(self, now: Instant) -> Self {
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

    pub fn try_change_to_maybe_ready(self) -> Self {
        match self {
            Self::Offline { .. } => Self::MaybeReady { prev_failures: 0 },
            Self::ServiceError { prev_failures, .. } => Self::MaybeReady { prev_failures },
            otherwise => otherwise,
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
        let result = Availability::Normal.try_change_to_service_error(now);
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
        let result = state.try_change_to_service_error(now);
        assert_eq!(
            result,
            Availability::ServiceError {
                detected_at: now,
                prev_failures: 1,
            }
        );
    }
}
