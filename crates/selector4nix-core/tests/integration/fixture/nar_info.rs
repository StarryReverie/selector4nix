use std::time::Duration;

use selector4nix_core::domain::common::url::Url;
use selector4nix_core::domain::nar_info::model::test_support::{
    NAR_FILE_RUBY_XZ, make_upstream_nar_info_data_with_url,
};
use selector4nix_core::domain::nar_info::model::{StorePathHash, UpstreamNarInfoData};
use selector4nix_core::domain::nar_info::port::NarInfoQueryData;
use selector4nix_core::domain::substituter::model::test_support::make_substituter_meta_with_url;

pub fn make_upstream_nar_info_data() -> UpstreamNarInfoData {
    make_upstream_nar_info_data_with_url(&format!("nar/{NAR_FILE_RUBY_XZ}"))
}

pub fn make_nar_info_url(substituter_url: &Url, hash: &StorePathHash) -> Url {
    let meta = make_substituter_meta_with_url(substituter_url);
    hash.on_substituter(&meta)
}

pub fn make_nar_info_query_data(latency: Duration) -> NarInfoQueryData {
    NarInfoQueryData::new(make_upstream_nar_info_data(), latency)
}
