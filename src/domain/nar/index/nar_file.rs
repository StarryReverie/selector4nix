use std::time::Duration;

use async_trait::async_trait;
use getset::{CopyGetters, Getters};

use crate::domain::nar::model::NarFileName;
use crate::domain::substituter::model::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NarFileEvent {
    Registered {
        nar_file: NarFileName,
        location: NarFileLocation,
    },
    Evicted {
        nar_file: NarFileName,
    },
}

#[async_trait]
pub trait NarFileIndex: Send + Sync {
    async fn get_location(&self, nar_file: &NarFileName) -> Option<NarFileLocation>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters, CopyGetters)]
pub struct NarFileLocation {
    #[getset(get = "pub")]
    source_url: Url,
    #[getset(get_copy = "pub")]
    timeout: Option<Duration>,
}

impl NarFileLocation {
    pub fn new(source_url: Url, timeout: Option<Duration>) -> Self {
        Self {
            source_url,
            timeout,
        }
    }
}
