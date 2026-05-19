use std::time::Duration;

use selector4nix::domain::substituter::model::{
    Availability, Priority, Substituter, SubstituterMeta, Url,
};

pub fn make_substituter_meta(url: &Url, priority: u32) -> SubstituterMeta {
    SubstituterMeta::new(url.clone(), Priority::new(priority).unwrap())
}

pub fn make_substituter_normal(url: &Url, priority: u32) -> Substituter {
    Substituter::new(make_substituter_meta(url, priority), Availability::Normal)
}

pub fn make_substituter_normal_with_nar_info_timeout(
    url: &Url,
    priority: u32,
    timeout: Duration,
) -> Substituter {
    Substituter::new(
        make_substituter_meta(url, priority).with_nar_info_timeout(timeout),
        Availability::Normal,
    )
}

pub fn make_substituter_maybe_ready(url: &Url, priority: u32) -> Substituter {
    Substituter::new(
        make_substituter_meta(url, priority),
        Availability::MaybeReady { prev_failures: 0 },
    )
}
