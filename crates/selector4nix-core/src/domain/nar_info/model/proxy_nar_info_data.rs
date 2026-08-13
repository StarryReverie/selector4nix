use getset::Getters;
use serde::{Deserialize, Serialize};

use crate::domain::common::url::Url;
use crate::domain::nar_info::model::{NarFileName, UpstreamNarInfoData};
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters, Serialize, Deserialize)]
#[getset(get = "pub")]
pub struct ProxyNarInfoData {
    content: String,
    nar_file: NarFileName,
}

impl ProxyNarInfoData {
    pub const UPSTREAM_QUERY_KEY: &'static str = "upstream_query";

    pub fn proxy_by_keep_url(
        upstream_data: &UpstreamNarInfoData,
        substituter: &SubstituterMeta,
    ) -> (Self, Url) {
        let proxy_data = Self {
            content: upstream_data.content().clone(),
            nar_file: upstream_data.nar_file().clone(),
        };
        let nar_source_url = Self::build_nar_source_url(upstream_data, substituter.storage_url());
        (proxy_data, nar_source_url)
    }

    pub fn proxy_by_rewrite_url_to_self(
        upstream_data: &UpstreamNarInfoData,
        substituter: &SubstituterMeta,
    ) -> (Self, Url) {
        let maybe_query_params = if let Some(qp) = upstream_data.query_params() {
            format!("?{}={}", Self::UPSTREAM_QUERY_KEY, qp.encode())
        } else {
            String::new()
        };
        let proxy_data = Self {
            content: Self::replace_url(
                upstream_data.content(),
                &format!(
                    "nar/{}{}",
                    upstream_data.nar_file().value(),
                    maybe_query_params
                ),
            ),
            nar_file: upstream_data.nar_file().clone(),
        };
        let nar_source_url = Self::build_nar_source_url(upstream_data, substituter.storage_url());
        (proxy_data, nar_source_url)
    }

    pub fn proxy_by_rewrite_url_to_upstream(
        upstream_data: &UpstreamNarInfoData,
        substituter: &SubstituterMeta,
    ) -> (Self, Url) {
        let nar_source_url = Self::build_nar_source_url(upstream_data, substituter.storage_url());
        let proxy_data = Self {
            content: Self::replace_url(upstream_data.content(), nar_source_url.value()),
            nar_file: upstream_data.nar_file().clone(),
        };
        (proxy_data, nar_source_url)
    }

    fn build_nar_source_url(upstream_data: &UpstreamNarInfoData, storage_url: &Url) -> Url {
        upstream_data
            .nar_source_url()
            .cloned()
            .unwrap_or_else(|| upstream_data.nar_file().with_storage_prefix(storage_url))
            .with_query_params(upstream_data.query_params())
    }

    fn replace_url(content: &str, new_url_str: &str) -> String {
        content
            .lines()
            .map(|line| {
                if line.starts_with("URL:") {
                    format!("URL: {}", new_url_str)
                } else {
                    line.to_string()
                }
            })
            .fold(String::new(), |acc, x| acc + &x + "\n")
    }

    pub fn store_path(&self) -> Option<&str> {
        self.content.lines().find_map(|line| {
            line.strip_prefix("StorePath:")
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::common::query_parameters::QueryParameters;
    use crate::domain::nar_info::model::test_support::STORE_PATH_HASH_RUBY;
    use crate::domain::substituter::model::test_support::{DEFAULT_URL, make_substituter_meta};

    use super::*;

    fn make_upstream_relative() -> UpstreamNarInfoData {
        let content = "StorePath: /nix/store/abc-hello\n\
             URL: nar/abc.nar.xz?query=abc\n\
             Compression: xz\n"
            .to_string();
        UpstreamNarInfoData::new(content).unwrap()
    }

    fn make_upstream_absolute() -> UpstreamNarInfoData {
        let content = "StorePath: /nix/store/abc-hello\n\
             URL: https://other.com/custom/abc.nar.xz\n\
             Compression: xz\n"
            .to_string();
        UpstreamNarInfoData::new(content).unwrap()
    }

    #[test]
    fn keep_preserves_relative_url() {
        let (proxy, nar_source_url) = ProxyNarInfoData::proxy_by_keep_url(
            &make_upstream_relative(),
            &make_substituter_meta(),
        );
        assert_eq!(proxy.nar_file().value(), "abc.nar.xz");
        assert!(proxy.content().contains("URL: nar/abc.nar.xz?query=abc\n"));
        assert_eq!(
            nar_source_url.value(),
            &format!("{DEFAULT_URL}/nar/abc.nar.xz?query=abc")
        );
    }

    #[test]
    fn keep_preserves_absolute_url() {
        let (proxy, nar_source_url) = ProxyNarInfoData::proxy_by_keep_url(
            &make_upstream_absolute(),
            &make_substituter_meta(),
        );
        assert_eq!(proxy.nar_file().value(), "abc.nar.xz");
        assert!(
            proxy
                .content()
                .contains("URL: https://other.com/custom/abc.nar.xz\n")
        );
        assert_eq!(
            nar_source_url.value(),
            "https://other.com/custom/abc.nar.xz"
        );
    }

    #[test]
    fn to_self_encodes_query_param_given_relative_url() {
        let (proxy, nar_source_url) = ProxyNarInfoData::proxy_by_rewrite_url_to_self(
            &make_upstream_relative(),
            &make_substituter_meta(),
        );
        assert_eq!(proxy.nar_file().value(), "abc.nar.xz");
        assert!(proxy.content().contains(&format!(
            "URL: nar/abc.nar.xz?{}={}\n",
            ProxyNarInfoData::UPSTREAM_QUERY_KEY,
            QueryParameters::try_from_raw("query=abc").unwrap().encode(),
        )));
        assert_eq!(
            nar_source_url.value(),
            &format!("{DEFAULT_URL}/nar/abc.nar.xz?query=abc")
        );
    }

    #[test]
    fn to_self_rewrites_absolute_url_to_relative() {
        let (proxy, nar_source_url) = ProxyNarInfoData::proxy_by_rewrite_url_to_self(
            &make_upstream_absolute(),
            &make_substituter_meta(),
        );
        assert_eq!(proxy.nar_file().value(), "abc.nar.xz");
        assert!(proxy.content().contains("URL: nar/abc.nar.xz\n"));
        assert!(!proxy.content().contains("https://other.com"));
        assert_eq!(
            nar_source_url.value(),
            "https://other.com/custom/abc.nar.xz"
        );
    }

    #[test]
    fn to_upstream_rewrites_relative_url_to_absolute() {
        let (proxy, nar_source_url) = ProxyNarInfoData::proxy_by_rewrite_url_to_upstream(
            &make_upstream_relative(),
            &make_substituter_meta(),
        );
        assert_eq!(proxy.nar_file().value(), "abc.nar.xz");
        let expected_url = format!("{DEFAULT_URL}/nar/abc.nar.xz?query=abc");
        assert!(proxy.content().contains(&format!("URL: {expected_url}\n")));
        assert_eq!(nar_source_url.value(), &expected_url);
    }

    #[test]
    fn to_upstream_preserves_absolute_url() {
        let (proxy, nar_source_url) = ProxyNarInfoData::proxy_by_rewrite_url_to_upstream(
            &make_upstream_absolute(),
            &make_substituter_meta(),
        );
        assert_eq!(proxy.nar_file().value(), "abc.nar.xz");
        assert!(
            proxy
                .content()
                .contains("URL: https://other.com/custom/abc.nar.xz\n")
        );
        assert_eq!(
            nar_source_url.value(),
            "https://other.com/custom/abc.nar.xz"
        );
    }

    #[test]
    fn store_path_extracts_full_path() {
        let data = UpstreamNarInfoData::new(format!(
            "StorePath: /nix/store/${STORE_PATH_HASH_RUBY}-ruby-2.7.3\n\
             URL: nar/abc.nar.xz\n",
        ))
        .unwrap();
        let (proxy, _) = ProxyNarInfoData::proxy_by_keep_url(&data, &make_substituter_meta());
        assert_eq!(
            proxy.store_path(),
            Some(format!("/nix/store/${STORE_PATH_HASH_RUBY}-ruby-2.7.3").as_str())
        );
    }
}
