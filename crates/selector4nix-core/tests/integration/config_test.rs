use std::num::NonZeroUsize;
use std::time::Duration;

use selector4nix_core::domain::nar_info::model::NarUrlRewriteOption;
use selector4nix_core::domain::substituter::model::PeriodicProbingOption;
use selector4nix_core::infrastructure::config::AppConfiguration;

use super::fixture::config::{make_config_string_minimal, make_config_string_overriden};

#[test]
fn example_config_file_is_valid() {
    let content = include_str!("../../../../docs/selector4nix.example.toml");
    AppConfiguration::deserialize(content).unwrap();
}

#[test]
fn defaults_are_applied_when_sections_omitted() {
    let config = AppConfiguration::deserialize(&make_config_string_minimal()).unwrap();

    assert_eq!(config.server.port, 5496);
    assert_eq!(config.network.nar_info_timeout, Duration::from_secs(30));
    assert_eq!(config.network.nar_timeout, Duration::from_secs(30));
    assert_eq!(
        config.network.max_concurrent_requests,
        NonZeroUsize::new(12).unwrap(),
    );
    assert_eq!(config.network.tolerance, 50);
    assert!(!config.network.ignore_nar_info_error);
    assert_eq!(
        config.network.periodic_probing,
        PeriodicProbingOption::Enabled,
    );
    assert!(config.network.chunked_streaming);
    assert_eq!(
        config.network.streaming_chunk_max_len,
        NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
    );
    assert_eq!(
        config.network.streaming_window_max_len,
        NonZeroUsize::new(8).unwrap(),
    );
    assert_eq!(config.proxy.rewrite_nar_url, NarUrlRewriteOption::ToSelf);
    assert_eq!(config.cache_info.store_dir, "/nix/store");
    assert!(config.cache_info.want_mass_query);
    assert_eq!(config.cache_info.priority.value(), 40);
    assert_eq!(
        config.cache.nar_info_cache_capacity,
        NonZeroUsize::new(4096).unwrap(),
    );
    assert_eq!(config.cache.nar_info_ttl, Duration::from_secs(14400));
    assert_eq!(
        config.cache.nar_file_cache_capacity,
        NonZeroUsize::new(4096).unwrap(),
    );
    assert_eq!(config.cache.nar_file_ttl, Duration::from_secs(14400));
    assert_eq!(config.substituters.len(), 1);
    assert!(config.substituters[0].storage_url.is_none());
    assert!(config.substituters[0].nar_info_timeout.is_none());
    assert!(config.substituters[0].nar_timeout.is_none());
    assert!(config.substituters[0].max_concurrent_requests.is_none());
}

#[test]
fn invalid_rewrite_to_target_is_rejected() {
    let result = AppConfiguration::deserialize(&make_config_string_overriden(
        r#"
[proxy]
rewrite_to_target = "invalid"
"#,
    ));

    assert!(result.is_err());
}

#[test]
fn non_absolute_store_dir_is_rejected() {
    let result = AppConfiguration::deserialize(&make_config_string_overriden(
        r#"
[cache_info]
store_dir = "relative/path"
"#,
    ));

    assert!(result.is_err());
}

#[test]
fn zero_priority_is_rejected() {
    let result = AppConfiguration::deserialize(&make_config_string_overriden(
        r#"
[[substituters]]
url = "https://cache.nixos.org/"
priority = 0
"#,
    ));

    assert!(result.is_err());
}

#[test]
fn empty_substituters_is_rejected() {
    let result = AppConfiguration::deserialize(
        r#"
[server]
ip = "127.0.0.1"
"#,
    );

    assert!(result.is_err());
}
