mod nar_file_name;
mod nar_info;
mod nar_info_resolution;
mod proxy_nar_info_data;
mod store_path_hash;
mod upstream_nar_info_data;

pub use nar_file_name::{NarFileName, TryNewNarFileNameError};
pub use nar_info::NarInfo;
pub use nar_info_resolution::{NarInfoResolution, NarUrlRewriteOption};
pub use proxy_nar_info_data::ProxyNarInfoData;
pub use store_path_hash::{StorePathHash, TryNewStorePathHashError};
pub use upstream_nar_info_data::{TryUpstreamNewNarInfoData, UpstreamNarInfoData};

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use crate::domain::nar_info::model::{NarFileName, StorePathHash, UpstreamNarInfoData};

    pub const STORE_PATH_HASH_RUBY: &str = "p4pclmv1gyja5kzc26npqpia1qqxrf0l";
    pub const NAR_FILE_RUBY_XZ: &str =
        "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz";
    pub const NAR_FILE_SAMPLE_UNCOMPRESSED: &str =
        "0mcjpwqknlcvkb42x5kyn7pmxa6ibpmrxqrcgzjm6fhwl99v19kd.nar";

    pub fn make_store_path_hash() -> StorePathHash {
        StorePathHash::new(STORE_PATH_HASH_RUBY.to_string()).unwrap()
    }

    pub fn make_nar_file_name() -> NarFileName {
        NarFileName::new(NAR_FILE_RUBY_XZ.to_string()).unwrap()
    }

    pub fn make_upstream_nar_info_data_with_url(url_field: &str) -> UpstreamNarInfoData {
        let content = format!(
            "StorePath: /nix/store/{STORE_PATH_HASH_RUBY}-ruby-2.7.3\n\
             URL: {url_field}\n"
        );
        UpstreamNarInfoData::new(content).unwrap()
    }
}
