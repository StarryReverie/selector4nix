use std::fmt::{Display, Formatter, Result as FmtResult};

use serde::{Deserialize, Serialize};
use snafu::{Snafu, ensure};

use crate::domain::common::url::Url;
use crate::{AppError, AppErrorKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NarFileName(String);

impl NarFileName {
    pub fn new(value: String) -> Result<Self, TryNewNarFileNameError> {
        ensure!(!value.is_empty(), EmptySnafu);
        ensure!(!value.contains('/'), ContainsSlashSnafu);
        ensure!(value.contains(".nar"), MissingNarExtensionSnafu);
        Ok(Self(value))
    }

    pub fn value(&self) -> &str {
        &self.0
    }

    pub fn with_storage_prefix(&self, prefix: &Url) -> Url {
        prefix.as_dir().join(self.value()).unwrap()
    }
}

impl Display for NarFileName {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.fmt(f)
    }
}

#[derive(Snafu, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TryNewNarFileNameError {
    #[snafu(display("nar file name should not be empty"))]
    Empty,
    #[snafu(display("nar file name should not contain `/`"))]
    ContainsSlash,
    #[snafu(display("nar file name should end with `\".nar\"` or `\".nar.{{compression}}\"`"))]
    MissingNarExtension,
}

impl From<TryNewNarFileNameError> for AppError {
    fn from(error: TryNewNarFileNameError) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::common::url::Url;
    use crate::domain::nar_info::model::test_support::{
        NAR_FILE_RUBY_XZ, NAR_FILE_SAMPLE_UNCOMPRESSED,
    };

    use super::*;

    #[test]
    fn new_succeeds() {
        let name = NarFileName::new(NAR_FILE_RUBY_XZ.to_string()).unwrap();
        assert_eq!(name.value(), NAR_FILE_RUBY_XZ);

        let name = NarFileName::new(NAR_FILE_SAMPLE_UNCOMPRESSED.to_string()).unwrap();
        assert_eq!(name.value(), NAR_FILE_SAMPLE_UNCOMPRESSED);
    }

    #[test]
    fn new_fails_given_empty() {
        assert!(matches!(
            NarFileName::new("".into()),
            Err(TryNewNarFileNameError::Empty)
        ));
    }

    #[test]
    fn new_fails_given_slash() {
        assert!(matches!(
            NarFileName::new("nar/abc.nar.xz".into()),
            Err(TryNewNarFileNameError::ContainsSlash)
        ));
    }

    #[test]
    fn new_fails_given_no_nar_extension() {
        assert!(matches!(
            NarFileName::new("abc.txt".into()),
            Err(TryNewNarFileNameError::MissingNarExtension)
        ));
    }

    #[test]
    fn with_storage_prefix_succeeds() {
        let name = NarFileName::new("abc.nar.xz".into()).unwrap();
        let prefix = Url::new("https://cache.nixos.org/nar").unwrap();
        let url = name.with_storage_prefix(&prefix);
        assert_eq!(url.value(), "https://cache.nixos.org/nar/abc.nar.xz");
    }
}
