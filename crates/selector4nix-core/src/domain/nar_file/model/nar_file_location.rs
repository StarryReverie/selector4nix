use std::time::Duration;

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crate::domain::common::url::Url;
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters, CopyGetters, Serialize, Deserialize)]
pub struct NarFileLocation {
    #[getset(get = "pub")]
    source_url: Url,
    #[getset(get = "pub")]
    substituter: SubstituterMeta,
    #[getset(get_copy = "pub")]
    timeout: Option<Duration>,
}

impl NarFileLocation {
    pub fn new(source_url: Url, substituter: SubstituterMeta, timeout: Option<Duration>) -> Self {
        Self {
            source_url,
            substituter,
            timeout,
        }
    }
}
