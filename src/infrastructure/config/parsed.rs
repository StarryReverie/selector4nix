use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Error as AnyhowError, Result as AnyhowResult};

use crate::domain::substituter::model::{Priority, Url};
use crate::infrastructure::config::raw::{
    AppRawConfiguration, CacheInfoRawConfiguration, CacheRawConfiguration, NetworkRawConfiguration,
    ProxyRawConfiguration, ServerRawConfiguration, SubstituterRawConfiguration,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppConfiguration {
    pub server: ServerConfiguration,
    pub network: NetworkConfiguration,
    pub proxy: ProxyConfiguration,
    pub cache_info: CacheInfoConfiguration,
    pub cache: CacheConfiguration,
    pub substituters: Vec<SubstituterConfiguration>,
}

impl AppConfiguration {
    pub fn deserialize(content: &str) -> AnyhowResult<Self> {
        AppRawConfiguration::deserialize(content)?
            .try_into()
            .context("configuration contains invalid value")
    }

    pub fn load_from(path: &Path) -> AnyhowResult<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("could not read configuration from {}", path.display()))?;
        let configuration = Self::deserialize(&content)?;
        tracing::info!(path = %path.display(), "loaded configuration");
        Ok(configuration)
    }

    pub fn load() -> AnyhowResult<Self> {
        let path = if let Ok(path) = std::env::var("SELECTOR4NIX_CONFIG_FILE") {
            tracing::info!(path = %path, "use configuration file from environment variable");
            PathBuf::from(path)
        } else if let Ok(path) = Path::new("./selector4nix.toml").canonicalize() {
            tracing::info!(path = %path.display(), "use configuration file from current directory");
            path
        } else if let Ok(path) = Path::new("/etc/selector4nix/selector4nix.toml").canonicalize() {
            tracing::info!(path = %path.display(), "use configuration file from /etc");
            path
        } else {
            return Err(anyhow::anyhow!("could not find any configuration file"));
        };

        Self::load_from(&path)
    }
}

impl TryFrom<AppRawConfiguration> for AppConfiguration {
    type Error = AnyhowError;

    fn try_from(raw: AppRawConfiguration) -> Result<Self, Self::Error> {
        Ok(Self {
            server: raw.server.try_into()?,
            network: raw.network.unwrap_or_default().try_into()?,
            proxy: raw.proxy.unwrap_or_default().try_into()?,
            cache_info: raw.cache_info.unwrap_or_default().try_into()?,
            cache: raw.cache.unwrap_or_default().try_into()?,
            substituters: raw
                .substituters
                .into_iter()
                .map(|c| c.try_into())
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerConfiguration {
    pub ip: IpAddr,
    pub port: u16,
}

impl ServerConfiguration {
    pub fn listen_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.port)
    }
}

impl TryFrom<ServerRawConfiguration> for ServerConfiguration {
    type Error = AnyhowError;

    fn try_from(raw: ServerRawConfiguration) -> Result<Self, Self::Error> {
        Ok(Self {
            ip: raw.ip,
            port: raw.port.unwrap_or(5496),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetworkConfiguration {
    pub nar_info_timeout: Duration,
    pub nar_timeout: Duration,
    pub max_concurrent_requests: usize,
}

impl TryFrom<NetworkRawConfiguration> for NetworkConfiguration {
    type Error = AnyhowError;

    fn try_from(raw: NetworkRawConfiguration) -> Result<Self, Self::Error> {
        Ok(Self {
            nar_info_timeout: raw
                .nar_info_timeout_secs
                .map_or(Duration::from_secs(10), |t| Duration::from_secs(t.max(1))),
            nar_timeout: raw
                .nar_timeout_secs
                .map_or(Duration::from_secs(10), |t| Duration::from_secs(t.max(1))),
            max_concurrent_requests: raw.max_concurrent_requests.unwrap_or(8),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProxyConfiguration {
    pub rewrite_nar_url: bool,
}

impl TryFrom<ProxyRawConfiguration> for ProxyConfiguration {
    type Error = AnyhowError;

    fn try_from(raw: ProxyRawConfiguration) -> Result<Self, Self::Error> {
        Ok(Self {
            rewrite_nar_url: raw.rewrite_nar_url.unwrap_or(true),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheInfoConfiguration {
    pub store_dir: String,
    pub want_mass_query: bool,
    pub priority: Priority,
}

impl TryFrom<CacheInfoRawConfiguration> for CacheInfoConfiguration {
    type Error = AnyhowError;

    fn try_from(raw: CacheInfoRawConfiguration) -> Result<Self, Self::Error> {
        Ok(Self {
            store_dir: raw.store_dir.map_or(Ok("/nix/store".into()), |s| {
                if s.starts_with("/") {
                    Ok(s)
                } else {
                    Err(anyhow::anyhow!(
                        "config `store_dir` should be an absolute path"
                    ))
                }
            })?,
            want_mass_query: raw.want_mass_query.unwrap_or(true),
            priority: raw.priority.map_or(Priority::new(40), Priority::new)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheConfiguration {
    pub nar_info_lookup_capacity: usize,
    pub nar_info_lookup_ttl: Duration,
    pub nar_location_capacity: usize,
    pub nar_location_ttl: Duration,
}

impl TryFrom<CacheRawConfiguration> for CacheConfiguration {
    type Error = AnyhowError;

    fn try_from(raw: CacheRawConfiguration) -> Result<Self, Self::Error> {
        Ok(Self {
            nar_info_lookup_capacity: raw.nar_info_lookup_capacity.unwrap_or(4096),
            nar_info_lookup_ttl: raw
                .nar_info_lookup_ttl_secs
                .map_or(Duration::from_hours(4), |t| Duration::from_secs(t.max(1))),
            nar_location_capacity: raw.nar_location_capacity.unwrap_or(4096),
            nar_location_ttl: raw
                .nar_location_ttl_secs
                .map_or(Duration::from_hours(4), |t| Duration::from_secs(t.max(1))),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubstituterConfiguration {
    pub url: Url,
    pub storage_url: Option<Url>,
    pub priority: Priority,
}

impl TryFrom<SubstituterRawConfiguration> for SubstituterConfiguration {
    type Error = AnyhowError;

    fn try_from(raw: SubstituterRawConfiguration) -> Result<Self, Self::Error> {
        Ok(Self {
            url: Url::new(&raw.url)?,
            storage_url: raw.storage_url.map(|s| Url::new(&s)).transpose()?,
            priority: raw.priority.map_or(Priority::new(40), Priority::new)?,
        })
    }
}
