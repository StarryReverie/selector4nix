use std::time::Duration;

use selector4nix::domain::common::url::Url;
use selector4nix::domain::nar_info::model::{NarFileName, StorePathHash, UpstreamNarInfoData};
use selector4nix::domain::nar_info::port::NarInfoQueryData;

use super::substituter;

const STORE_PATH_HASH: &str = "p4pclmv1gyja5kzc26npqpia1qqxrf0l";
const NAR_FILE: &str = super::nar_file::NAR_FILE;

pub fn make_store_path_hash() -> StorePathHash {
    StorePathHash::new(STORE_PATH_HASH.to_string()).unwrap()
}

pub fn make_nar_file_name() -> NarFileName {
    NarFileName::new(NAR_FILE.to_string()).unwrap()
}

pub fn make_upstream_nar_info_data() -> UpstreamNarInfoData {
    UpstreamNarInfoData::new(format!(
        "StorePath: /nix/store/{STORE_PATH_HASH}-ruby-2.7.3\n\
         URL: nar/{NAR_FILE}\n\
         Compression: xz\n"
    ))
    .unwrap()
}

pub fn make_nar_info_url(substituter_url: &Url, hash: &StorePathHash) -> Url {
    let meta = substituter::make_substituter_meta(substituter_url, 1);
    hash.on_substituter(&meta)
}

pub fn make_nar_info_query_data(latency: Duration) -> NarInfoQueryData {
    NarInfoQueryData::new(make_upstream_nar_info_data(), latency)
}
