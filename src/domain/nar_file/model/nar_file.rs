use getset::Getters;

use crate::domain::nar_info::model::NarFileName;

use super::NarFileLocation;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters)]
#[getset(get = "pub")]
pub struct NarFileKey {
    nar_hash: String,
    compression: String,
}

impl NarFileKey {
    pub fn new(nar_hash: String, compression: String) -> Self {
        Self {
            nar_hash,
            compression,
        }
    }

    pub fn from_file_name(file: &NarFileName) -> Option<Self> {
        let (prefix, suffix) = file.value().split_once(".nar.")?;
        Some(Self {
            nar_hash: prefix.to_string(),
            compression: suffix.to_string(),
        })
    }

    pub fn to_file_name(&self) -> NarFileName {
        NarFileName::new(format!("{}.nar.{}", self.nar_hash, self.compression))
            .expect("valid NarFileName from NarFileKey")
    }
}

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
