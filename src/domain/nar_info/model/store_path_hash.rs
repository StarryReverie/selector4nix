use serde::{Deserialize, Serialize};
use snafu::{Snafu, ensure};

use crate::domain::common::url::Url;
use crate::domain::substituter::model::SubstituterMeta;
use crate::{AppError, AppErrorKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorePathHash(String);

impl StorePathHash {
    pub fn new(value: String) -> Result<Self, TryNewStorePathHashError> {
        ensure!(value.len() == 32, InvalidLengthSnafu);
        ensure!(
            value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            InvalidCharacterSnafu
        );
        Ok(Self(value))
    }

    pub fn value(&self) -> &str {
        &self.0
    }

    pub fn on_substituter(&self, substituter: &SubstituterMeta) -> Url {
        let base = substituter.url().as_dir();
        base.join(&format!("{}.narinfo", self.value())).unwrap()
    }
}

#[derive(Snafu, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TryNewStorePathHashError {
    #[snafu(display("store path hash must be exactly 32 characters"))]
    InvalidLength,
    #[snafu(display("store path hash must contain only lowercase letters and digits"))]
    InvalidCharacter,
}

impl From<TryNewStorePathHashError> for AppError {
    fn from(error: TryNewStorePathHashError) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::common::url::Url;
    use crate::domain::nar_info::model::test_support::{
        STORE_PATH_HASH_RUBY, make_store_path_hash,
    };
    use crate::domain::substituter::model::test_support::{
        DEFAULT_URL, make_substituter_meta, make_substituter_meta_with_url,
    };

    use super::*;

    #[test]
    fn new_succeeds() {
        let hash = make_store_path_hash();
        assert_eq!(hash.value(), STORE_PATH_HASH_RUBY);
    }

    #[test]
    fn new_fails_given_wrong_length() {
        assert!(matches!(
            StorePathHash::new("abc".to_string()),
            Err(TryNewStorePathHashError::InvalidLength)
        ));
        assert!(matches!(
            StorePathHash::new("p4pclmv1gyja5kzc26npqpia1qqxrf0lxxx".to_string()),
            Err(TryNewStorePathHashError::InvalidLength)
        ));
    }

    #[test]
    fn new_fails_given_uppercase() {
        assert!(matches!(
            StorePathHash::new("P4pclmv1gyja5kzc26npqpia1qqxrf0l".to_string()),
            Err(TryNewStorePathHashError::InvalidCharacter)
        ));
    }

    #[test]
    fn new_fails_given_slash() {
        assert!(matches!(
            StorePathHash::new("p4pclmv1gyja5kzc26n/qpia1qqxrf0l".to_string()),
            Err(TryNewStorePathHashError::InvalidCharacter)
        ));
    }

    #[test]
    fn build_nar_info_url_succeeds() {
        let hash = make_store_path_hash();
        let substituter = make_substituter_meta();
        assert_eq!(
            hash.on_substituter(&substituter).value(),
            &format!("{DEFAULT_URL}/{STORE_PATH_HASH_RUBY}.narinfo"),
        );
    }

    #[test]
    fn build_nar_info_url_succeeds_given_base_path() {
        let hash = make_store_path_hash();
        let substituter = make_substituter_meta_with_url(
            &Url::new("https://mirrors.ustc.edu.cn/nix-channels/store").unwrap(),
        );
        assert_eq!(
            hash.on_substituter(&substituter).value(),
            &format!(
                "https://mirrors.ustc.edu.cn/nix-channels/store/{STORE_PATH_HASH_RUBY}.narinfo"
            ),
        );
    }
}
