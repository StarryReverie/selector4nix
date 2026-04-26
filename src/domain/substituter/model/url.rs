use std::fmt::{Display, Formatter, Result as FmtResult};

use serde::Serialize;
use snafu::{ResultExt, Snafu, ensure};
use url::{ParseError, Url as UrlInner};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Url(UrlInner);

impl Url {
    pub fn new(value: &str) -> Result<Self, TryNewUrlError> {
        let parsed = UrlInner::parse(value).context(InvalidSnafu)?;
        ensure!(
            matches!(parsed.scheme(), "http" | "https"),
            UnsupportedSchemeSnafu {
                scheme: parsed.scheme()
            }
        );
        ensure!(parsed.host_str().is_some(), NoHostSnafu);
        Ok(Self(parsed))
    }

    pub fn value(&self) -> &str {
        self.0.as_str()
    }

    pub fn join(&self, path: &str) -> Result<Self, TryNewUrlError> {
        let joined = self.0.join(path).context(InvalidSnafu)?;
        Ok(Self(joined))
    }

    pub fn as_dir(&self) -> Self {
        let mut inner = self.0.clone();
        let path = inner.path();
        if !path.is_empty() && !path.ends_with('/') {
            inner.set_path(&format!("{path}/"));
        }
        Self(inner)
    }
}

impl Display for Url {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.fmt(f)
    }
}

#[derive(Snafu, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TryNewUrlError {
    #[snafu(display("URL should have a host"))]
    NoHost,
    #[snafu(display("URL scheme `{scheme}` is not one of `http` or `https`"))]
    UnsupportedScheme { scheme: String },
    #[snafu(display("could not build invalid URL"))]
    Invalid { source: ParseError },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_succeeds() {
        let _ = Url::new("http://cache.nixos.org").unwrap();
        let _ = Url::new("https://cache.nixos.org/path").unwrap();
        let _ = Url::new("http://localhost:8080").unwrap();
    }

    #[test]
    fn new_fails_given_invalid_urls() {
        assert!(matches!(
            Url::new("ftp://files.example.com"),
            Err(TryNewUrlError::UnsupportedScheme { .. }),
        ));
        assert!(matches!(
            Url::new("cache.nixos.org"),
            Err(TryNewUrlError::Invalid { .. }),
        ));
        assert!(matches!(Url::new(""), Err(TryNewUrlError::Invalid { .. }),));
    }

    #[test]
    fn join_url_with_path_succeeds() {
        let base = Url::new("https://cache.nixos.org").unwrap();
        assert_eq!(
            base.join("nar/abc.nar.xz").unwrap().value(),
            "https://cache.nixos.org/nar/abc.nar.xz"
        );
        assert_eq!(
            base.join("0bm7a1sgh5q7nf19yl7basf6bqw9i0i2.narinfo")
                .unwrap()
                .value(),
            "https://cache.nixos.org/0bm7a1sgh5q7nf19yl7basf6bqw9i0i2.narinfo"
        );
    }

    #[test]
    fn as_dir_appends_slash_given_path_without_trailing_slash() {
        let url = Url::new("https://mirrors.ustc.edu.cn/nix-channels/store").unwrap();
        let dir = url.as_dir();
        assert_eq!(
            dir.value(),
            "https://mirrors.ustc.edu.cn/nix-channels/store/"
        );
    }

    #[test]
    fn as_dir_preserves_url_given_path_with_trailing_slash() {
        let url = Url::new("https://mirrors.ustc.edu.cn/nix-channels/store/").unwrap();
        let dir = url.as_dir();
        assert_eq!(
            dir.value(),
            "https://mirrors.ustc.edu.cn/nix-channels/store/"
        );
    }

    #[test]
    fn as_dir_preserves_url_given_root_path() {
        let url = Url::new("https://cache.nixos.org").unwrap();
        let dir = url.as_dir();
        assert_eq!(dir.value(), "https://cache.nixos.org/");
    }

    #[test]
    fn join_preserves_path_segments_given_dir_url() {
        let base = Url::new("https://mirrors.ustc.edu.cn/nix-channels/store")
            .unwrap()
            .as_dir();
        let joined = base.join("abc.narinfo").unwrap();
        assert_eq!(
            joined.value(),
            "https://mirrors.ustc.edu.cn/nix-channels/store/abc.narinfo"
        );
    }
}
