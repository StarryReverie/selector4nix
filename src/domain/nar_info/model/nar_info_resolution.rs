use crate::domain::common::url::Url;
use crate::domain::nar_info::model::{ProxyNarInfoData, UpstreamNarInfoData};
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarInfoResolution {
    Resolved {
        nar_info: ProxyNarInfoData,
        substituter: SubstituterMeta,
        source_url: Url,
    },
    NotFound,
}

impl NarInfoResolution {
    pub fn from_completed_query(
        successful_outcome: Option<(UpstreamNarInfoData, SubstituterMeta)>,
        rewrite_nar_url: NarUrlRewriteOption,
    ) -> Self {
        match successful_outcome {
            Some((nar_info, substituter)) => {
                let (nar_info, source_url) = match rewrite_nar_url {
                    NarUrlRewriteOption::Keep => {
                        ProxyNarInfoData::proxy_by_keep_url(&nar_info, &substituter)
                    }
                    NarUrlRewriteOption::ToSelf => {
                        ProxyNarInfoData::proxy_by_rewrite_url_to_self(&nar_info, &substituter)
                    }
                    NarUrlRewriteOption::ToUpstream => {
                        ProxyNarInfoData::proxy_by_rewrite_url_to_upstream(&nar_info, &substituter)
                    }
                };
                Self::Resolved {
                    nar_info,
                    substituter,
                    source_url,
                }
            }
            None => Self::NotFound,
        }
    }

    pub fn nar_info(&self) -> Option<&ProxyNarInfoData> {
        match self {
            Self::Resolved { nar_info, .. } => Some(nar_info),
            Self::NotFound => None,
        }
    }

    pub fn source_url(&self) -> Option<&Url> {
        match self {
            Self::Resolved { source_url, .. } => Some(source_url),
            Self::NotFound => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NarUrlRewriteOption {
    Keep,
    ToSelf,
    ToUpstream,
}

#[cfg(test)]
mod tests {
    use crate::domain::substituter::model::Priority;

    use super::*;

    fn make_upstream_nar_info_data() -> UpstreamNarInfoData {
        UpstreamNarInfoData::new(
            "StorePath: /nix/store/p4pclmv1gyja5kzc26npqpia1qqxrf0l-ruby-2.7.3\n\
             URL: nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz\n"
                .into(),
        )
        .unwrap()
    }

    fn make_upstream_nar_info_data_with_external_url() -> UpstreamNarInfoData {
        UpstreamNarInfoData::new(
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
    fn from_completed_query_returns_not_found_given_none() {
        let resolution = NarInfoResolution::from_completed_query(None, NarUrlRewriteOption::ToSelf);
        assert!(matches!(resolution, NarInfoResolution::NotFound));
    }

    #[test]
    fn from_completed_query_resolves_given_relative_url() {
        let resolution = NarInfoResolution::from_completed_query(
            Some((make_upstream_nar_info_data(), make_substituter_meta())),
            NarUrlRewriteOption::ToSelf,
        );

        match resolution {
            NarInfoResolution::Resolved {
                nar_info,
                source_url,
                ..
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
                make_upstream_nar_info_data_with_external_url(),
                make_substituter_meta(),
            )),
            NarUrlRewriteOption::ToSelf,
        );

        match resolution {
            NarInfoResolution::Resolved {
                nar_info,
                source_url,
                ..
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
                make_upstream_nar_info_data_with_external_url(),
                make_substituter_meta(),
            )),
            NarUrlRewriteOption::Keep,
        );

        match resolution {
            NarInfoResolution::Resolved {
                nar_info,
                source_url,
                ..
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
