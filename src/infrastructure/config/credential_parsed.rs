use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::path::{Path, PathBuf};

use anyhow::{Context, Error as AnyhowError, Result as AnyhowResult};

use crate::domain::common::url::Url;
use crate::infrastructure::config::credential_raw::{AppRawCredential, AppRawCredentialEntry};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppCredential {
    pub credentials: Vec<AppCredentialEntry>,
}

impl AppCredential {
    pub fn empty() -> Self {
        Self {
            credentials: Vec::new(),
        }
    }

    pub fn with(mut self, entry: AppCredentialEntry) -> Self {
        self.credentials.push(entry);
        self
    }

    pub fn deserialize(content: &str) -> AnyhowResult<Self> {
        AppRawCredential::deserialize(content)?
            .try_into()
            .context("credential contains invalid value")
    }

    pub fn load_from(path: &Path) -> AnyhowResult<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("could not read credential from {}", path.display()))?;
        let credential = Self::deserialize(&content)?;
        tracing::info!(path = %path.display(), "loaded credential");
        Ok(credential)
    }

    pub fn load() -> Option<AnyhowResult<Self>> {
        let path = if let Ok(path) = std::env::var("SELECTOR4NIX_CREDENTIAL_FILE") {
            tracing::info!(path = %path, "use credential file from environment variable");
            PathBuf::from(path)
        } else if let Ok(path) = Path::new("./credentials.toml").canonicalize() {
            tracing::info!(path = %path.display(), "use credential file from current directory");
            path
        } else if let Ok(path) = Path::new("/etc/selector4nix/credentials.toml").canonicalize() {
            tracing::info!(path = %path.display(), "use credential file from `/etc`");
            path
        } else {
            tracing::warn!("could not find any credential file");
            return None;
        };

        Some(Self::load_from(&path))
    }

    pub fn lookup(&self, full_url: &Url) -> Option<&AppCredentialEntry> {
        let full = full_url.value();
        self.credentials
            .iter()
            .filter(|c| is_prefix_match(full, c.url.value()))
            .max_by_key(|c| c.url.value().len())
    }
}

impl TryFrom<AppRawCredential> for AppCredential {
    type Error = AnyhowError;

    fn try_from(raw: AppRawCredential) -> Result<Self, Self::Error> {
        Ok(Self {
            credentials: raw
                .credentials
                .into_iter()
                .map(|c| c.try_into())
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppCredentialEntry {
    pub url: Url,
    pub login: String,
    pub secret: Option<String>,
}

impl TryFrom<AppRawCredentialEntry> for AppCredentialEntry {
    type Error = AnyhowError;

    fn try_from(raw: AppRawCredentialEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            url: Url::new(&raw.url).with_context(|| {
                format!(
                    "invalid substituter URL in `credentials[].url`: `\"{}\"`",
                    raw.url
                )
            })?,
            login: raw.login,
            secret: raw.secret,
        })
    }
}

impl Debug for AppCredentialEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("AppCredentialEntry")
            .field("url", &self.url)
            .field("login", &self.login)
            .field(
                "secret",
                if self.secret.is_some() {
                    &"Some(_)"
                } else {
                    &"None"
                },
            )
            .finish()
    }
}

fn is_prefix_match(full: &str, prefix: &str) -> bool {
    if !full.starts_with(prefix) {
        return false;
    }
    if full.len() == prefix.len() {
        return true;
    }
    prefix.ends_with('/') || full.as_bytes()[prefix.len()] == b'/'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(url: &str) -> AppCredentialEntry {
        AppCredentialEntry {
            url: Url::new(url).unwrap(),
            login: "test".to_string(),
            secret: None,
        }
    }

    #[test]
    fn lookup_selects_longest_matching_prefix() {
        let cred = AppCredential::empty()
            .with(make_entry("https://example.org/nix/"))
            .with(make_entry("https://example.org/nix/private/"));
        let url = Url::new("https://example.org/nix/private/foo.narinfo").unwrap();
        assert_eq!(
            cred.lookup(&url).unwrap().url.value(),
            "https://example.org/nix/private/"
        );
    }

    #[test]
    fn lookup_rejects_partial_segment_match() {
        let cred = AppCredential::empty().with(make_entry("https://example.org/nix/cache1"));
        let url = Url::new("https://example.org/nix/cache10/foo.narinfo").unwrap();
        assert!(cred.lookup(&url).is_none());
    }

    #[test]
    fn lookup_returns_none_given_no_match() {
        let cred = AppCredential::empty();
        let url = Url::new("https://cache.nixos.org/foo").unwrap();
        assert!(cred.lookup(&url).is_none());
    }
}
