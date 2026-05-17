use std::time::Duration;

use selector4nix::domain::nar::model::{NarFileName, NarInfoData, StorePathHash};
use selector4nix::domain::nar::port::NarInfoQueryData;
use selector4nix::domain::substituter::model::Url;

use super::substituter;

const STORE_PATH_HASH: &str = "p4pclmv1gyja5kzc26npqpia1qqxrf0l";

const NAR_FILE: &str = "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz";

pub fn make_store_path_hash() -> StorePathHash {
    StorePathHash::new(STORE_PATH_HASH.to_string()).unwrap()
}

pub fn make_nar_info_data() -> NarInfoData {
    NarInfoData::original(format!(
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

pub fn make_source_url(substituter_url: &Url, priority: u32) -> Url {
    let meta = substituter::make_substituter_meta(substituter_url, priority);
    let nar_file = NarFileName::new(NAR_FILE.to_string()).unwrap();
    nar_file.with_storage_prefix(meta.storage_url())
}

pub fn make_nar_info_query_data(latency: Duration) -> NarInfoQueryData {
    NarInfoQueryData::new(make_nar_info_data(), latency)
}
