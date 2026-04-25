use getset::{CopyGetters, Getters};

use crate::domain::substituter::model::{Priority, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters, CopyGetters)]
pub struct SubstituterMeta {
    #[getset(get = "pub")]
    url: Url,
    #[getset(get_copy = "pub")]
    priority: Priority,
}

impl SubstituterMeta {
    pub fn new(url: Url, priority: Priority) -> Self {
        Self { url, priority }
    }
}
