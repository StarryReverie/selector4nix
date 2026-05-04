use getset::Getters;

use crate::domain::nar::model::{NarInfoData, StorePathHash};
use crate::domain::substituter::model::{SubstituterMeta, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarInfoResolution {
    Resolved {
        nar_info: NarInfoData,
        source_url: Url,
    },
    NotFound,
}

impl NarInfoResolution {
    pub fn from_completed_query(
        successful_outcome: Option<(NarInfoData, SubstituterMeta)>,
        rewrite_nar_url: bool,
    ) -> Self {
        match successful_outcome {
            Some((nar_info, substituter)) => {
                let source_url = nar_info.source_url().cloned().unwrap_or_else(|| {
                    nar_info
                        .nar_file()
                        .with_storage_prefix(substituter.storage_url())
                });
                Self::Resolved {
                    nar_info: if rewrite_nar_url {
                        nar_info.rewrite_url_to_self()
                    } else {
                        nar_info
                    },
                    source_url,
                }
            }
            None => Self::NotFound,
        }
    }

    pub fn nar_info(&self) -> Option<&NarInfoData> {
        match self {
            Self::Resolved { nar_info, .. } => Some(nar_info),
            _ => None,
        }
    }

    pub fn source_url(&self) -> Option<&Url> {
        match self {
            Self::Resolved { source_url, .. } => Some(source_url),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Getters)]
pub struct Nar {
    #[getset(get = "pub")]
    hash: StorePathHash,
    resolution: Option<NarInfoResolution>,
}

impl Nar {
    pub fn new(hash: StorePathHash) -> Self {
        Self {
            hash,
            resolution: None,
        }
    }

    pub fn on_resolved(mut self, resolution: NarInfoResolution) -> Self {
        self.resolution = Some(resolution);
        self
    }

    pub fn resolution(&self) -> Option<&NarInfoResolution> {
        self.resolution.as_ref()
    }

    pub fn nar_info(&self) -> Option<&NarInfoData> {
        self.resolution().and_then(NarInfoResolution::nar_info)
    }

    pub fn source_url(&self) -> Option<&Url> {
        self.resolution().and_then(NarInfoResolution::source_url)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::substituter::model::Priority;

    use super::*;

    fn make_hash() -> StorePathHash {
        StorePathHash::new("p4pclmv1gyja5kzc26npqpia1qqxrf0l".into()).unwrap()
    }

    fn make_nar_info_data() -> NarInfoData {
        NarInfoData::original(
            "StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-ruby-2.7.3\n\
             URL: nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz\n"
                .into(),
        )
        .unwrap()
    }

    fn make_nar_info_data_with_external_url() -> NarInfoData {
        NarInfoData::original(
            "StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-ruby-2.7.3\n\
             URL: https://storage.example.com/nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz\n"
                .into(),
        )
        .unwrap()
    }

    fn make_substituter_meta() -> SubstituterMeta {
        SubstituterMeta::new(
            Url::new("https://cache.nixos.org").unwrap(),
            Priority::new(40).unwrap(),
        )
    }

    #[test]
    fn new_succeeds() {
        let hash = make_hash();
        let nar = Nar::new(hash.clone());
        assert_eq!(nar.hash(), &hash);
        assert!(nar.resolution().is_none());
    }

    #[test]
    fn from_completed_query_returns_not_found_given_none() {
        let resolution = NarInfoResolution::from_completed_query(None, true);
        assert!(matches!(resolution, NarInfoResolution::NotFound));
    }

    #[test]
    fn from_completed_query_resolves_given_relative_url() {
        let resolution = NarInfoResolution::from_completed_query(
            Some((make_nar_info_data(), make_substituter_meta())),
            true,
        );

        match resolution {
            NarInfoResolution::Resolved {
                nar_info,
                source_url,
            } => {
                assert!(nar_info.content().contains(
                    "URL: nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz\n"
                ));
                assert_eq!(
                    source_url.value(),
                    "https://cache.nixos.org/nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz"
                );
            }
            _ => panic!("expected Resolved"),
        }
    }

    #[test]
    fn from_completed_query_resolves_given_external_url_and_rewrite_true() {
        let resolution = NarInfoResolution::from_completed_query(
            Some((
                make_nar_info_data_with_external_url(),
                make_substituter_meta(),
            )),
            true,
        );

        match resolution {
            NarInfoResolution::Resolved {
                nar_info,
                source_url,
            } => {
                assert!(nar_info.content().contains(
                    "URL: nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz\n"
                ));
                assert!(!nar_info.content().contains("https://storage.example.com"));
                assert_eq!(
                    source_url.value(),
                    "https://storage.example.com/nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz"
                );
            }
            _ => panic!("expected Resolved"),
        }
    }

    #[test]
    fn from_completed_query_preserves_external_url_given_rewrite_false() {
        let resolution = NarInfoResolution::from_completed_query(
            Some((
                make_nar_info_data_with_external_url(),
                make_substituter_meta(),
            )),
            false,
        );

        match resolution {
            NarInfoResolution::Resolved {
                nar_info,
                source_url,
            } => {
                assert!(nar_info.content().contains("https://storage.example.com/nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz"));
                assert!(!nar_info.content().contains("URL: nar/"));
                assert_eq!(
                    source_url.value(),
                    "https://storage.example.com/nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz"
                );
            }
            _ => panic!("expected Resolved"),
        }
    }
}
