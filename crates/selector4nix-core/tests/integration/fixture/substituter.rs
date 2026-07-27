use std::time::Duration;

use selector4nix_core::domain::common::url::Url;
use selector4nix_core::domain::substituter::model::test_support::make_substituter_meta_with_url_pri;
use selector4nix_core::domain::substituter::model::{Availability, Substituter, SubstituterMeta};

pub fn make_substituter_meta_with_storage_url(
    url: &Url,
    storage_url: Url,
    priority: u32,
) -> SubstituterMeta {
    make_substituter_meta_with_url_pri(url, priority).with_storage_url(storage_url)
}

pub fn make_substituter_normal_with_nar_info_timeout(
    url: &Url,
    priority: u32,
    timeout: Duration,
) -> Substituter {
    Substituter::new(
        make_substituter_meta_with_url_pri(url, priority).with_nar_info_timeout(timeout),
        Availability::Normal,
    )
}
