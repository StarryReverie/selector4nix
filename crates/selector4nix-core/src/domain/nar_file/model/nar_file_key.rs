use getset::Getters;
use serde::{Deserialize, Serialize};

use crate::domain::nar_info::model::NarFileName;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters, Serialize, Deserialize)]
pub struct NarFileKey {
    #[getset(get = "pub")]
    file_hash: String,
    compression: Option<String>,
}

impl NarFileKey {
    pub fn new(file_hash: String) -> Self {
        Self {
            file_hash,
            compression: None,
        }
    }

    pub fn with_compression<V>(mut self, compression: V) -> Self
    where
        V: Into<Option<String>>,
    {
        self.compression = compression.into().filter(|c| !c.is_empty());
        self
    }

    pub fn from_file_name(nar_file: &NarFileName) -> Self {
        let (file_hash, suffix) = nar_file
            .value()
            .split_once(".nar")
            .expect("`nar_file` should contains `\".nar\"`");
        let compression = suffix.trim_start_matches(".");
        Self::new(file_hash.to_string()).with_compression(compression.to_string())
    }

    pub fn to_file_name(&self) -> NarFileName {
        if let Some(compression) = &self.compression {
            NarFileName::new(format!("{}.nar.{}", self.file_hash, compression))
                .expect("converting `NarFileKey` to `NarFileName` should always be valid")
        } else {
            NarFileName::new(format!("{}.nar", self.file_hash))
                .expect("converting `NarFileKey` to `NarFileName` should always be valid")
        }
    }

    pub fn compression(&self) -> Option<&str> {
        self.compression.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::nar_info::model::test_support::{
        NAR_FILE_SAMPLE_UNCOMPRESSED, make_nar_file_name,
    };

    use super::*;

    #[test]
    fn new_succeeds_given_compression() {
        let key = NarFileKey::from_file_name(&make_nar_file_name());
        assert_eq!(
            key.file_hash(),
            "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3"
        );
        assert_eq!(key.compression(), Some("xz"));
    }

    #[test]
    fn new_succeeds_given_no_compression() {
        let name = NarFileName::new(NAR_FILE_SAMPLE_UNCOMPRESSED.to_string()).unwrap();
        let key = NarFileKey::from_file_name(&name);
        assert_eq!(
            key.file_hash(),
            "0mcjpwqknlcvkb42x5kyn7pmxa6ibpmrxqrcgzjm6fhwl99v19kd"
        );
    }
}
