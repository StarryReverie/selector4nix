use selector4nix::domain::substituter::model::{
    Availability, Priority, Substituter, SubstituterMeta, Url,
};

pub fn make_substituter_meta(url: &Url, priority: u32) -> SubstituterMeta {
    SubstituterMeta::new(url.clone(), Priority::new(priority).unwrap())
}

pub fn make_substituter_normal(url: &Url, priority: u32) -> Substituter {
    Substituter::new(make_substituter_meta(url, priority), Availability::Normal)
}

pub fn make_substituter_maybe_ready(url: &Url, priority: u32) -> Substituter {
    Substituter::new(
        make_substituter_meta(url, priority),
        Availability::MaybeReady { prev_failures: 0 },
    )
}
