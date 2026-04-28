use snafu::{Snafu, ensure};

use crate::domain::substituter::model::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NarFileName(String);

impl NarFileName {
    pub fn new(value: String) -> Result<Self, TryNewNarFileNameError> {
        ensure!(!value.is_empty(), EmptySnafu);
        ensure!(!value.contains('/'), ContainsSlashSnafu);
        ensure!(value.contains(".nar."), MissingNarExtensionSnafu);
        Ok(Self(value))
    }

    pub fn value(&self) -> &str {
        &self.0
    }

    pub fn with_storage_prefix(&self, prefix: &Url) -> Url {
        prefix.as_dir().join(self.value()).unwrap()
    }
}

#[derive(Snafu, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TryNewNarFileNameError {
    #[snafu(display("nar file name should not be empty"))]
    Empty,
    #[snafu(display("nar file name should not contain `/`"))]
    ContainsSlash,
    #[snafu(display("nar file name should end with `.nar.{{compression}}`"))]
    MissingNarExtension,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_succeeds() {
        let name =
            NarFileName::new("1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz".into())
                .unwrap();
        assert_eq!(
            name.value(),
            "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz"
        );
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
