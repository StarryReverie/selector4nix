use getset::Getters;
use snafu::{OptionExt, ResultExt, Snafu};

use crate::domain::nar::model::nar_file_name::TryNewNarFileNameError;

use super::NarFileName;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters)]
#[getset(get = "pub")]
pub struct NarInfoData {
    content: String,
    nar_file: NarFileName,
}

impl NarInfoData {
    pub fn new(original_content: String) -> Result<Self, TryNewNarInfoData> {
        let original_url = original_content
            .lines()
            .find(|line| line.starts_with("URL:"))
            .map(|line| line.trim_start_matches("URL:").trim().to_string())
            .context(NoUrlFieldSnafu)?;

        let filename = original_url
            .rfind('/')
            .map_or(original_url.as_str(), |pos| &original_url[pos + 1..]);

        let nar_file = NarFileName::new(filename.to_string()).context(InvalidNarFileNameSnafu)?;

        let rewritten_content = original_content
            .lines()
            .map(|line| {
                if line.starts_with("URL:") {
                    format!("URL: nar/{}", nar_file.value())
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(Self {
            content: rewritten_content,
            nar_file,
        })
    }
}

#[derive(Snafu, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TryNewNarInfoData {
    #[snafu(display("narinfo file should contains a relative path to a nar file"))]
    NoUrlField,
    #[snafu(display("nar file name is invalid"))]
    InvalidNarFileName { source: TryNewNarFileNameError },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_succeeds() {
        let mut content = String::new();
        content.push_str("StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-ruby-2.7.3\n");
        content.push_str("URL: nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz\n");
        content.push_str("Compression: xz\n");
        content.push_str("FileHash: sha256:1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3\n");
        content.push_str("FileSize: 4029176\n");
        content.push_str("NarHash: sha256:1impfw8zdgisxkghq9a3q7cn7jb9zyzgxdydiamp8z2nlyyl0h5h\n");
        content.push_str("NarSize: 18735072\n");
        content.push_str("References: 0d71ygfwbmy1xjlbj1v027dfmy9cqavy-libffi-3.3 0dbbrvlw2rahvzi69bmpqy1z9mvzg62s-gdbm-1.19 0i6vphc3vnr8mg0gxjr61564hnp0s2md-gnugrep-3.6 0vkw1m51q34dr64z5i87dy99an4hfmyg-coreutils-8.32 64ylsrpd025kcyi608w3dqckzyz57mdc-libyaml-0.2.5 65ys3k6gn2s27apky0a0la7wryg3az9q-zlib-1.2.11 9m4hy7cy70w6v2rqjmhvd7ympqkj6yxk-ncurses-6.2 a4yw1svqqk4d8lhwinn9xp847zz9gfma-bash-4.4-p23 hbm0951q7xrl4qd0ccradp6bhjayfi4b-openssl-1.1.1k hjwjf3bj86gswmxva9k40nqx6jrb5qvl-readline-6.3p08 p4pclmv1gyja5kzc26npqpia1qqxrf0l-ruby-2.7.3 sbbifs2ykc05inws26203h0xwcadnf0l-glibc-2.32-46\n");
        content.push_str("Deriver: bidkcs01mww363s4s7akdhbl6ws66b0z-ruby-2.7.3.drv\n");
        content.push_str("Sig: cache.nixos.org-1:GrGV/Ls10TzoOaCnrcAqmPbKXFLLSBDeGNh5EQGKyuGA4K1wv1LcRVb6/sU+NAPK8lDiam8XcdJzUngmdhfTBQ==\n");

        let data = NarInfoData::new(content).unwrap();
        assert_eq!(
            data.nar_file().value(),
            "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz"
        );
        assert!(
            data.content()
                .contains("URL: nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz\n")
        );
    }

    #[test]
    fn new_rewrites_url_given_non_standard_substituter() {
        let mut content = String::new();
        content.push_str("StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-hello\n");
        content.push_str("URL: https://other.com/custom/abc.nar.xz\n");
        content.push_str("Compression: xz\n");

        let data = NarInfoData::new(content).unwrap();
        assert_eq!(data.nar_file().value(), "abc.nar.xz");
        assert!(data.content().contains("URL: nar/abc.nar.xz\n"));
        assert!(!data.content().contains("https://other.com"));
    }

    #[test]
    fn new_fails_given_no_url_field() {
        let content = "StorePath: /nix/store/abc-hello\nCompression: xz\n".to_string();
        assert!(NarInfoData::new(content).is_err());
    }
}
