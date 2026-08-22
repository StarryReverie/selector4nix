use std::fmt::{Display, Formatter, Result as FmtResult};

use snafu::{OptionExt, ResultExt, Snafu, ensure};

use crate::domain::common::url::Url;
use crate::domain::nar_info::model::{StorePathHash, TryNewStorePathHashError};
use crate::domain::substituter::model::SubstituterMeta;
use crate::{AppError, AppErrorKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DerivingPath {
    hash: StorePathHash,
    extra: String,
}

impl DerivingPath {
    pub fn new(mut value: String) -> Result<Self, TryNewDerivingPathError> {
        ensure!(value.is_ascii(), InvalidAsciiSnafu);
        ensure!(value.ends_with(".drv"), InvalidExtensionSnafu);

        let extra_bytes = value
            .as_bytes()
            .get(33..(value.len() - ".drv".len()))
            .context(TooShortSnafu)?
            .to_owned();
        let extra = String::from_utf8(extra_bytes).unwrap();

        value.truncate(32);
        let hash = StorePathHash::new(value).context(InvalidStorePathHashSnafu)?;

        Ok(Self { hash, extra })
    }

    pub fn on_substituter_log(&self, substituter: &SubstituterMeta) -> Url {
        let base = substituter.url().as_dir();
        base.join(&format!("/log/{self}")).unwrap()
    }
}

impl Display for DerivingPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}-{}.drv", self.hash.value(), self.extra)
    }
}

#[derive(Debug, Snafu, Clone, PartialEq, Eq)]
pub enum TryNewDerivingPathError {
    #[snafu(display("the deriving path is not a valid ASCII string"))]
    InvalidAscii,
    #[snafu(display("the deriving path should end with `.drv`"))]
    InvalidExtension,
    #[snafu(display("the deriving path should have at least 34 characters excluding `.drv`"))]
    TooShort,
    #[snafu(display("the deriving path contains an invalid store path hash"))]
    InvalidStorePathHash { source: TryNewStorePathHashError },
}

impl From<TryNewDerivingPathError> for AppError {
    fn from(error: TryNewDerivingPathError) -> Self {
        Self::new(AppErrorKind::Input, error)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::substituter::model::test_support::make_substituter_meta_with_url;

    use super::*;

    #[test]
    fn new_fails_given_invalid_ascii() {
        assert!(matches!(
            DerivingPath::new("bidkcs01mww363s4s7akdhbl6ws66b0z-rubé-2.7.3.drv".to_string()),
            Err(TryNewDerivingPathError::InvalidAscii),
        ));
    }

    #[test]
    fn new_fails_given_missing_drv_extension() {
        assert!(matches!(
            DerivingPath::new("".to_string()),
            Err(TryNewDerivingPathError::InvalidExtension),
        ));
        assert!(matches!(
            DerivingPath::new("bidkcs01mww363s4s7akdhbl6ws66b0z-ruby-2.7.3".to_string()),
            Err(TryNewDerivingPathError::InvalidExtension),
        ));
    }

    #[test]
    fn new_fails_given_too_short() {
        assert!(matches!(
            DerivingPath::new("abc.drv".to_string()),
            Err(TryNewDerivingPathError::TooShort),
        ));
        assert!(matches!(
            DerivingPath::new("bidkcs01mww363s4s7akdhbl6ws66b0z.drv".to_string()),
            Err(TryNewDerivingPathError::TooShort),
        ));
    }

    #[test]
    fn new_fails_given_invalid_store_path_hash() {
        assert!(matches!(
            DerivingPath::new("Bidkcs01mww363s4s7akdhbl6ws66b0z-ruby-2.7.3.drv".to_string()),
            Err(TryNewDerivingPathError::InvalidStorePathHash { .. }),
        ));
    }

    #[test]
    fn display_succeeds() {
        let deriving_path =
            DerivingPath::new("bidkcs01mww363s4s7akdhbl6ws66b0z-ruby-2.7.3.drv".to_string())
                .unwrap();
        assert_eq!(
            deriving_path.to_string(),
            "bidkcs01mww363s4s7akdhbl6ws66b0z-ruby-2.7.3.drv",
        );
    }

    #[test]
    fn build_log_url_succeeds_given_base_path() {
        let deriving_path =
            DerivingPath::new("bidkcs01mww363s4s7akdhbl6ws66b0z-ruby-2.7.3.drv".to_string())
                .unwrap();
        let substituter =
            make_substituter_meta_with_url(&Url::new("https://cache.nixos.org/").unwrap());
        assert_eq!(
            deriving_path.on_substituter_log(&substituter).value(),
            "https://cache.nixos.org/log/bidkcs01mww363s4s7akdhbl6ws66b0z-ruby-2.7.3.drv",
        );
    }
}
