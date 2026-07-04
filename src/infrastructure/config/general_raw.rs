use std::net::IpAddr;

use anyhow::{Context, Result as AnyhowResult};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct AppRawConfiguration {
    pub server: ServerRawConfiguration,
    pub network: Option<NetworkRawConfiguration>,
    pub download: Option<DownloadRawConfiguration>,
    pub proxy: Option<ProxyRawConfiguration>,
    pub cache_info: Option<CacheInfoRawConfiguration>,
    pub cache: Option<CacheRawConfiguration>,
    pub substituters: Vec<SubstituterRawConfiguration>,
}

impl AppRawConfiguration {
    pub fn deserialize(content: &str) -> AnyhowResult<Self> {
        toml::from_str(content).context("could not deserialize content to TOML configuration")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct ServerRawConfiguration {
    pub ip: IpAddr,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Default)]
pub struct NetworkRawConfiguration {
    pub nar_info_timeout_secs: Option<u64>,
    pub nar_timeout_secs: Option<u64>,
    pub max_concurrent_requests: Option<usize>,
    pub tolerance_msecs: Option<u64>,
    pub ignore_nar_info_error: Option<bool>,
    pub periodic_probing: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Default)]
pub struct DownloadRawConfiguration {
    pub segmented: Option<bool>,
    pub segmented_min_file_bytes: Option<u64>,
    pub segmented_max_connections: Option<usize>,
    pub segmented_load_threshold: Option<usize>,
    pub segmented_buffer_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Default)]
pub struct ProxyRawConfiguration {
    pub rewrite_nar_url: Option<bool>,
    pub rewrite_to_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Default)]
pub struct CacheInfoRawConfiguration {
    pub store_dir: Option<String>,
    pub want_mass_query: Option<bool>,
    pub priority: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Default)]
pub struct CacheRawConfiguration {
    pub nar_info_lookup_capacity: Option<usize>,
    pub nar_info_lookup_ttl_secs: Option<u64>,
    pub nar_location_capacity: Option<usize>,
    pub nar_location_ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct SubstituterRawConfiguration {
    pub url: String,
    pub storage_url: Option<String>,
    pub priority: Option<u32>,
    pub nar_info_timeout_secs: Option<u64>,
    pub nar_timeout_secs: Option<u64>,
}
