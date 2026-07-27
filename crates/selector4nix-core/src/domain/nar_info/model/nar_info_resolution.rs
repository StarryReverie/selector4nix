use serde::{Deserialize, Serialize};

use crate::domain::common::url::Url;
use crate::domain::nar_info::model::{ProxyNarInfoData, UpstreamNarInfoData};
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    use crate::domain::nar_info::model::test_support::{
        NAR_FILE_RUBY_XZ, make_upstream_nar_info_data_with_url,
    };
    use crate::domain::substituter::model::test_support::{DEFAULT_URL, make_substituter_meta};

    use super::*;

    #[test]
    fn from_completed_query_returns_not_found_given_none() {
        let resolution = NarInfoResolution::from_completed_query(None, NarUrlRewriteOption::ToSelf);
        assert_eq!(resolution, NarInfoResolution::NotFound);
    }

    #[test]
    fn from_completed_query_resolves_given_relative_url() {
        let upstream = make_upstream_nar_info_data_with_url(&format!("nar/{NAR_FILE_RUBY_XZ}"));
        let resolution = NarInfoResolution::from_completed_query(
            Some((upstream, make_substituter_meta())),
            NarUrlRewriteOption::ToSelf,
        );

        match resolution {
            NarInfoResolution::Resolved {
                nar_info,
                source_url,
                ..
            } => {
                assert!(
                    nar_info
                        .content()
                        .contains(&format!("URL: nar/{NAR_FILE_RUBY_XZ}\n"))
                );
                assert_eq!(
                    source_url.value(),
                    &format!("{DEFAULT_URL}/nar/{NAR_FILE_RUBY_XZ}")
                );
            }
            _ => panic!("expected Resolved"),
        }
    }

    #[test]
    fn from_completed_query_resolves_given_external_url_and_rewrite_true() {
        let upstream = make_upstream_nar_info_data_with_url(&format!(
            "https://storage.example.com/nar/{NAR_FILE_RUBY_XZ}"
        ));
        let resolution = NarInfoResolution::from_completed_query(
            Some((upstream, make_substituter_meta())),
            NarUrlRewriteOption::ToSelf,
        );

        match resolution {
            NarInfoResolution::Resolved {
                nar_info,
                source_url,
                ..
            } => {
                assert!(
                    nar_info
                        .content()
                        .contains(&format!("URL: nar/{NAR_FILE_RUBY_XZ}\n"))
                );
                assert!(!nar_info.content().contains("https://storage.example.com"));
                assert_eq!(
                    source_url.value(),
                    &format!("https://storage.example.com/nar/{NAR_FILE_RUBY_XZ}")
                );
            }
            _ => panic!("expected Resolved"),
        }
    }

    #[test]
    fn from_completed_query_preserves_external_url_given_rewrite_false() {
        let upstream = make_upstream_nar_info_data_with_url(&format!(
            "https://storage.example.com/nar/{NAR_FILE_RUBY_XZ}"
        ));
        let resolution = NarInfoResolution::from_completed_query(
            Some((upstream, make_substituter_meta())),
            NarUrlRewriteOption::Keep,
        );

        match resolution {
            NarInfoResolution::Resolved {
                nar_info,
                source_url,
                ..
            } => {
                let expected_url = format!("https://storage.example.com/nar/{NAR_FILE_RUBY_XZ}");
                assert!(nar_info.content().contains(&expected_url));
                assert!(!nar_info.content().contains("URL: nar/"));
                assert_eq!(source_url.value(), &expected_url);
            }
            _ => panic!("expected Resolved"),
        }
    }
}
