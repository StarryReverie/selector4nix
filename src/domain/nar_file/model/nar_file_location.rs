use std::time::Duration;

use getset::{CopyGetters, Getters};

use crate::domain::substituter::model::Url;

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
