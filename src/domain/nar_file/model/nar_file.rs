use getset::Getters;

use crate::domain::nar_file::model::{NarFileKey, NarFileLocation};

#[derive(Debug, Clone, PartialEq, Eq, Getters)]
#[getset(get = "pub")]
pub struct NarFile {
    key: NarFileKey,
    location: Option<NarFileLocation>,
}

impl NarFile {
    pub fn new(key: NarFileKey) -> Self {
        Self {
            key,
            location: None,
        }
    }

    pub fn with_location(mut self, location: NarFileLocation) -> Self {
        self.location = Some(location);
        self
    }
}
