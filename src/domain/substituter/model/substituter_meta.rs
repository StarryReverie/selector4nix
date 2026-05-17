use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use serde::{Serialize, Serializer};

use crate::domain::substituter::model::{Priority, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
struct SubstituterMetaInner {
    url: Url,
    storage_url: Url,
    priority: Priority,
    nar_info_timeout: Option<Duration>,
    nar_timeout: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct SubstituterMeta(Arc<SubstituterMetaInner>);

impl SubstituterMeta {
    pub fn new(url: Url, priority: Priority) -> Self {
        let storage_url = url.as_dir().join("nar").unwrap();
        Self(Arc::new(SubstituterMetaInner {
            url,
            storage_url,
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
        Self(Arc::new(SubstituterMetaInner {
            storage_url,
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

impl PartialEq for SubstituterMeta {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for SubstituterMeta {}

impl Hash for SubstituterMeta {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.0.hash(state);
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
