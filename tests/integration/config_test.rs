use std::time::Duration;

use selector4nix::domain::nar_info::model::NarUrlRewriteOption;
use selector4nix::domain::substituter::model::PeriodicProbingOption;
use selector4nix::infrastructure::config::AppConfiguration;

use super::fixture;

#[test]
fn example_config_file_is_valid() {
    let content = include_str!("../../docs/selector4nix.example.toml");
    AppConfiguration::deserialize(content).unwrap();
}

#[test]
fn defaults_are_applied_when_sections_omitted() {
    let config =
        AppConfiguration::deserialize(&fixture::config::make_config_string_minimal()).unwrap();

    assert_eq!(config.server.port, 5496);
    assert_eq!(config.network.nar_info_timeout, Duration::from_secs(30));
    assert_eq!(config.network.nar_timeout, Duration::from_secs(30));
    assert_eq!(config.network.max_concurrent_requests, 12);
    assert_eq!(config.network.tolerance, 50);
    assert_eq!(config.network.ignore_nar_info_error, false);
    assert_eq!(
        config.network.periodic_probing,
        PeriodicProbingOption::Enabled
    );
    assert_eq!(config.proxy.rewrite_nar_url, NarUrlRewriteOption::ToSelf);
    assert_eq!(config.cache_info.store_dir, "/nix/store");
    assert!(config.cache_info.want_mass_query);
    assert_eq!(config.cache_info.priority.value(), 40);
    assert_eq!(config.cache.nar_info_lookup_capacity, 4096);
    assert_eq!(config.cache.nar_info_lookup_ttl, Duration::from_secs(14400));
    assert_eq!(config.cache.nar_location_capacity, 4096);
    assert_eq!(config.cache.nar_location_ttl, Duration::from_secs(14400));
    assert_eq!(config.substituters.len(), 1);
    assert!(config.substituters[0].storage_url.is_none());
    assert!(config.substituters[0].nar_info_timeout.is_none());
    assert!(config.substituters[0].nar_timeout.is_none());
}

#[test]
fn zero_timeout_is_clamped_to_one() {
    let config = AppConfiguration::deserialize(&fixture::config::make_config_string_overriden(
        r#"
[network]
nar_info_timeout_secs = 0
nar_timeout_secs = 0
"#,
    ))
    .unwrap();

    assert_eq!(config.network.nar_info_timeout, Duration::from_secs(1));
    assert_eq!(config.network.nar_timeout, Duration::from_secs(1));
}

#[test]
fn zero_tolerance_is_clamped_to_one() {
    let config = AppConfiguration::deserialize(&fixture::config::make_config_string_overriden(
        r#"
[network]
tolerance_msecs = 0
"#,
    ))
    .unwrap();

    assert_eq!(config.network.tolerance, 1);
}

#[test]
fn invalid_rewrite_to_target_is_rejected() {
    let result = AppConfiguration::deserialize(&fixture::config::make_config_string_overriden(
        r#"
[proxy]
rewrite_to_target = "invalid"
"#,
    ));

    assert!(result.is_err());
}

#[test]
fn non_absolute_store_dir_is_rejected() {
    let result = AppConfiguration::deserialize(&fixture::config::make_config_string_overriden(
        r#"
[cache_info]
store_dir = "relative/path"
"#,
    ));

    assert!(result.is_err());
}

#[test]
fn zero_priority_is_rejected() {
    let result = AppConfiguration::deserialize(&fixture::config::make_config_string_overriden(
        r#"
[[substituters]]
url = "https://cache.nixos.org/"
priority = 0
"#,
    ));

    assert!(result.is_err());
}
