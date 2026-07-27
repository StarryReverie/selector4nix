use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::domain::common::url::Url;
use crate::domain::substituter::model::Priority;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct SubstituterMetaInner {
    url: Url,
    storage_url: Url,
    has_custom_storage_url: bool,
    priority: Priority,
    nar_info_timeout: Option<Duration>,
    nar_timeout: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubstituterMeta(Arc<SubstituterMetaInner>);

impl SubstituterMeta {
    pub fn new(url: Url, priority: Priority) -> Self {
        let storage_url = url.as_dir().join("nar").unwrap();
        Self(Arc::new(SubstituterMetaInner {
            url,
            storage_url,
            has_custom_storage_url: false,
            priority,
            nar_info_timeout: None,
            nar_timeout: None,
        }))
    }

    pub fn url(&self) -> &Url {
        &self.0.url
    }

    pub fn storage_url(&self) -> &Url {
        &self.0.storage_url
    }

    pub fn has_custom_storage_url(&self) -> bool {
        self.0.has_custom_storage_url
    }

    pub fn priority(&self) -> Priority {
        self.0.priority
    }

    pub fn nar_info_timeout(&self) -> Option<Duration> {
        self.0.nar_info_timeout
    }

    pub fn nar_timeout(&self) -> Option<Duration> {
        self.0.nar_timeout
    }

    pub fn with_storage_url(&self, storage_url: Url) -> Self {
        let has_custom_storage_url = match self.0.url.as_dir().join("nar") {
            Ok(default) => storage_url != default,
            Err(_) => true,
        };
        Self(Arc::new(SubstituterMetaInner {
            storage_url,
            has_custom_storage_url,
            ..(*self.0).clone()
        }))
    }

    pub fn with_nar_info_timeout<T>(&self, timeout: T) -> Self
    where
        T: Into<Option<Duration>>,
    {
        Self(Arc::new(SubstituterMetaInner {
            nar_info_timeout: timeout.into(),
            ..(*self.0).clone()
        }))
    }

    pub fn with_nar_timeout<T>(&self, timeout: T) -> Self
    where
        T: Into<Option<Duration>>,
    {
        Self(Arc::new(SubstituterMetaInner {
            nar_timeout: timeout.into(),
            ..(*self.0).clone()
        }))
    }
}

impl Serialize for SubstituterMeta {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SubstituterMeta {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(Arc::new(SubstituterMetaInner::deserialize(
            deserializer,
        )?)))
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::common::url::Url;
    use crate::domain::substituter::model::test_support::{
        make_substituter_meta, make_substituter_meta_with_url,
    };

    #[test]
    fn default_storage_url_is_not_custom() {
        let meta = make_substituter_meta();
        assert!(!meta.has_custom_storage_url());
    }

    #[test]
    fn storage_url_equal_to_default_is_not_custom() {
        let meta = make_substituter_meta_with_url(&Url::new("https://example.com").unwrap());
        let default_storage = meta.url().as_dir().join("nar").unwrap();
        let meta = meta.with_storage_url(default_storage);
        assert!(!meta.has_custom_storage_url());
    }

    #[test]
    fn storage_url_differing_from_default_is_custom() {
        let meta = make_substituter_meta_with_url(&Url::new("https://example.com").unwrap());
        let meta = meta.with_storage_url(Url::new("https://cdn.example.com/nar").unwrap());
        assert!(meta.has_custom_storage_url());
    }
}
