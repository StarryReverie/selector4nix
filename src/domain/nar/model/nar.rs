use getset::Getters;

use crate::domain::nar::model::{NarInfoData, StorePathHash};
use crate::domain::substituter::model::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarState {
    Unknown,
    NotFound,
    Resolved {
        nar_info: NarInfoData,
        source_url: Url,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Getters)]
#[getset(get = "pub")]
pub struct Nar {
    hash: StorePathHash,
    state: NarState,
}

impl Nar {
    pub fn new(hash: StorePathHash) -> Self {
        Self {
            hash,
            state: NarState::Unknown,
        }
    }

    pub fn on_resolved(mut self, nar_info: NarInfoData, source_url: Url) -> Self {
        self.state = NarState::Resolved {
            nar_info,
            source_url,
        };
        self
    }

    pub fn on_not_found(mut self) -> Self {
        self.state = NarState::NotFound;
        self
    }

    pub fn on_query_completed(
        self,
        outcome: Result<(NarInfoData, Url), AbnormalQueryOutcome>,
        rewrite_nar_url: bool,
    ) -> Self {
        match outcome {
            Ok((nar_info, storage_url)) => {
                let source_url = nar_info
                    .source_url()
                    .cloned()
                    .unwrap_or_else(|| nar_info.nar_file().with_storage_prefix(&storage_url));
                let nar_info = if rewrite_nar_url {
                    nar_info.rewrite_url_to_self()
                } else {
                    nar_info
                };
                self.on_resolved(nar_info, source_url)
            }
            Err(AbnormalQueryOutcome::NotFound) => self.on_not_found(),
            Err(AbnormalQueryOutcome::Error) => self,
        }
    }
}

pub enum AbnormalQueryOutcome {
    NotFound,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hash() -> StorePathHash {
        StorePathHash::new("p4pclmv1gyja5kzc26npqpia1qqxrf0l".into()).unwrap()
    }

    fn make_nar_info_data() -> NarInfoData {
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
        NarInfoData::rewritten(content).unwrap()
    }

    #[test]
    fn new_succeeds() {
        let hash = make_hash();
        let nar = Nar::new(hash.clone());
        assert!(matches!(nar.state(), NarState::Unknown));
        assert_eq!(nar.hash(), &hash);
    }

    #[test]
    fn on_resolved_succeeds() {
        let nar = Nar::new(make_hash());
        let data = make_nar_info_data();
        let source_url = Url::new("https://cache.nixos.org/nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz").unwrap();
        let nar = nar.on_resolved(data.clone(), source_url.clone());
        match nar.state() {
            NarState::Resolved {
                nar_info,
                source_url: su,
            } => {
                assert_eq!(*nar_info, data);
                assert_eq!(*su, source_url);
            }
            _ => panic!("expected Resolved state"),
        }
    }

    #[test]
    fn on_not_found_succeeds() {
        let nar = Nar::new(make_hash());
        let nar = nar.on_not_found();
        assert!(matches!(nar.state(), NarState::NotFound));
    }

    fn make_query_nar_info_data() -> NarInfoData {
        NarInfoData::rewritten(
            "StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-hello\nURL: nar/abc.nar.xz\n"
                .into(),
        )
        .unwrap()
    }

    fn make_query_nar_info_data_with_external_url() -> NarInfoData {
        NarInfoData::original(
            "StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-hello\nURL: https://other.com/custom/abc.nar.xz\n"
                .into(),
        )
        .unwrap()
    }

    fn make_storage_url() -> Url {
        Url::new("https://cache.nixos.org/nar").unwrap()
    }

    #[test]
    fn on_query_completed_resolves_given_found() {
        let nar = Nar::new(make_hash());
        let data = make_query_nar_info_data();
        let storage_url = make_storage_url();

        let nar = nar.on_query_completed(Ok((data, storage_url)), true);

        assert!(matches!(nar.state(), NarState::Resolved { .. }));
    }

    #[test]
    fn on_query_completed_preserves_original_url_given_rewrite_false() {
        let nar = Nar::new(make_hash());
        let data = make_query_nar_info_data_with_external_url();
        let storage_url = Url::new("https://other.com/nar").unwrap();

        let nar = nar.on_query_completed(Ok((data, storage_url)), false);

        match nar.state() {
            NarState::Resolved { nar_info, .. } => {
                assert!(
                    nar_info
                        .content()
                        .contains("https://other.com/custom/abc.nar.xz")
                );
                assert!(!nar_info.content().contains("URL: nar/abc.nar.xz"));
            }
            _ => panic!("expected Resolved state"),
        }
    }

    #[test]
    fn on_query_completed_rewrites_url_given_rewrite_true() {
        let nar = Nar::new(make_hash());
        let data = make_query_nar_info_data_with_external_url();
        let storage_url = Url::new("https://other.com/nar").unwrap();

        let nar = nar.on_query_completed(Ok((data, storage_url)), true);

        match nar.state() {
            NarState::Resolved { nar_info, .. } => {
                assert!(nar_info.content().contains("URL: nar/abc.nar.xz\n"));
                assert!(!nar_info.content().contains("https://other.com"));
            }
            _ => panic!("expected Resolved state"),
        }
    }

    #[test]
    fn on_query_completed_transitions_to_not_found() {
        let nar = Nar::new(make_hash());

        let nar = nar.on_query_completed(Err(AbnormalQueryOutcome::NotFound), true);

        assert!(matches!(nar.state(), NarState::NotFound));
    }

    #[test]
    fn on_query_completed_remains_unknown_given_error() {
        let nar = Nar::new(make_hash());

        let nar = nar.on_query_completed(Err(AbnormalQueryOutcome::Error), true);

        assert!(matches!(nar.state(), NarState::Unknown));
    }
}
