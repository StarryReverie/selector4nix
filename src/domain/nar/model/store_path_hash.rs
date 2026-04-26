use snafu::{Snafu, ensure};

use crate::domain::substituter::model::{SubstituterMeta, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

#[cfg(test)]
mod tests {
    use crate::domain::substituter::model::Priority;

    use super::*;

    #[test]
    fn new_succeeds() {
        let hash = StorePathHash::new("p4pclmv1gyja5kzc26npqpia1qqxrf0l".to_string()).unwrap();
        assert_eq!(hash.value(), "p4pclmv1gyja5kzc26npqpia1qqxrf0l");
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
        let hash = StorePathHash::new("p4pclmv1gyja5kzc26npqpia1qqxrf0l".into()).unwrap();
        let meta = SubstituterMeta::new(
            Url::new("https://cache.nixos.org").unwrap(),
            Priority::new(40).unwrap(),
        );
        assert_eq!(
            hash.on_substituter(&meta).value(),
            "https://cache.nixos.org/p4pclmv1gyja5kzc26npqpia1qqxrf0l.narinfo",
        );
    }

    #[test]
    fn build_nar_info_url_succeeds_given_base_path() {
        let hash = StorePathHash::new("p4pclmv1gyja5kzc26npqpia1qqxrf0l".into()).unwrap();
        let meta = SubstituterMeta::new(
            Url::new("https://mirrors.ustc.edu.cn/nix-channels/store").unwrap(),
            Priority::new(40).unwrap(),
        );
        assert_eq!(
            hash.on_substituter(&meta).value(),
            "https://mirrors.ustc.edu.cn/nix-channels/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l.narinfo",
        );
    }
}
