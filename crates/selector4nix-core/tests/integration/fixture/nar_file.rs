use std::time::{Duration, SystemTime};

use selector4nix_core::domain::common::expire_at::ExpireAt;
use selector4nix_core::domain::common::url::Url;
use selector4nix_core::domain::nar_file::model::{NarFile, NarFileKey, NarFileLocation};
use selector4nix_core::domain::nar_info::model::test_support::make_nar_file_name;
use selector4nix_core::domain::substituter::model::SubstituterMeta;
use selector4nix_core::domain::substituter::model::test_support::make_substituter_meta_with_url_pri;

pub fn make_source_url_with_substituter_meta(meta: &SubstituterMeta) -> Url {
    make_nar_file_name().with_storage_prefix(meta.storage_url())
}

pub fn make_source_url(substituter_url: &Url, priority: u32) -> Url {
    let meta = make_substituter_meta_with_url_pri(substituter_url, priority);
    make_source_url_with_substituter_meta(&meta)
}

pub fn make_nar_file_key() -> NarFileKey {
    NarFileKey::from_file_name(&make_nar_file_name())
}

pub fn make_nar_file_location_with_substituter_meta(meta: &SubstituterMeta) -> NarFileLocation {
    NarFileLocation::new(
        make_source_url_with_substituter_meta(meta),
        meta.clone(),
        None,
    )
}

pub fn make_nar_file_location(substituter_url: &Url, priority: u32) -> NarFileLocation {
    let meta = make_substituter_meta_with_url_pri(substituter_url, priority);
    make_nar_file_location_with_substituter_meta(&meta)
}

pub fn make_nar_file_with_location(location: NarFileLocation) -> NarFile {
    let expire_at = ExpireAt::since(SystemTime::now(), Duration::from_secs(3600));
    NarFile::new(make_nar_file_key()).on_located(location, expire_at, None)
}
