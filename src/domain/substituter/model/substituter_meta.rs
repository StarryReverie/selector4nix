use getset::{CopyGetters, Getters, WithSetters};
use serde::Serialize;

use crate::domain::substituter::model::{Priority, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters, CopyGetters, Serialize, WithSetters)]
pub struct SubstituterMeta {
    #[getset(get = "pub")]
    url: Url,
    #[getset(get = "pub", set_with = "pub")]
    storage_url: Url,
    #[getset(get_copy = "pub")]
    priority: Priority,
}

impl SubstituterMeta {
    pub fn new(url: Url, priority: Priority) -> Self {
        let storage_url = url.as_dir().join("nar").unwrap();
        Self {
            url,
            storage_url,
            priority,
        }
    }
}
