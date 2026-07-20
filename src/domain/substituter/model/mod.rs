mod availability;
mod priority;
mod substituter;
mod substituter_meta;

pub use availability::Availability;
pub use priority::{Priority, TryNewPriorityError};
pub use substituter::{PeriodicProbingOption, ProbedState, Substituter, UpdateSubstituterEvent};
pub use substituter_meta::SubstituterMeta;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use tokio::time::Instant;

    use crate::domain::common::url::Url;

    use super::{Availability, Priority, Substituter, SubstituterMeta};

    pub const DEFAULT_PRIORITY: u32 = 40;
    pub const DEFAULT_URL: &str = "https://cache.nixos.org";

    pub fn make_substituter_meta() -> SubstituterMeta {
        SubstituterMeta::new(
            Url::new(DEFAULT_URL).unwrap(),
            Priority::new(DEFAULT_PRIORITY).unwrap(),
        )
    }

    pub fn make_substituter_meta_with_url(url: &Url) -> SubstituterMeta {
        SubstituterMeta::new(url.clone(), Priority::new(DEFAULT_PRIORITY).unwrap())
    }

    pub fn make_substituter_normal_with_url(url: &Url) -> Substituter {
        Substituter::new(make_substituter_meta_with_url(url), Availability::Normal)
    }

    pub fn make_substituter_offline_with_url(url: &Url) -> Substituter {
        Substituter::new(
            make_substituter_meta_with_url(url),
            Availability::Offline {
                detected_at: Instant::now(),
            },
        )
    }

    pub fn make_substituter_maybe_ready_with_url(url: &Url) -> Substituter {
        Substituter::new(
            make_substituter_meta_with_url(url),
            Availability::MaybeReady { prev_failures: 0 },
        )
    }
}
