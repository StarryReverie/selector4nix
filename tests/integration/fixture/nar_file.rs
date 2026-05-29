use selector4nix::domain::common::url::Url;
use selector4nix::domain::nar_file::model::{NarFileKey, NarFileLocation};

use super::{nar_info, substituter};

pub const NAR_FILE: &str = "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz";

pub fn make_source_url(substituter_url: &Url, priority: u32) -> Url {
    let meta = substituter::make_substituter_meta(substituter_url, priority);
    nar_info::make_nar_file_name().with_storage_prefix(meta.storage_url())
}

pub fn make_nar_file_key() -> NarFileKey {
    NarFileKey::from_file_name(&nar_info::make_nar_file_name())
}

pub fn make_nar_file_location(substituter_url: &Url, priority: u32) -> NarFileLocation {
    NarFileLocation::new(make_source_url(substituter_url, priority), None)
}
