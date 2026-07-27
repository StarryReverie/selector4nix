use std::time::SystemTime;

use getset::Getters;
use serde::{Deserialize, Serialize};

use crate::domain::common::expire_at::ExpireAt;
use crate::domain::nar_file::model::{NarFileKey, NarFileLocation};
use crate::domain::nar_info::model::StorePathHash;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters, Serialize, Deserialize)]
pub struct NarFile {
    #[getset(get = "pub")]
    key: NarFileKey,
    location: Option<(NarFileLocation, ExpireAt)>,
    store_path_hash: Option<StorePathHash>,
}

impl NarFile {
    pub fn new(key: NarFileKey) -> Self {
        Self {
            key,
            location: None,
            store_path_hash: None,
        }
    }

    pub fn on_located(
        mut self,
        location: NarFileLocation,
        expire_at: ExpireAt,
        store_path_hash: Option<StorePathHash>,
    ) -> Self {
        self.location = Some((location, expire_at));
        if let Some(store_path_hash) = store_path_hash {
            self.store_path_hash = Some(store_path_hash);
        }
        self
    }

    pub fn on_relocated(mut self, location: NarFileLocation) -> Self {
        self.location = self.location.map(|(_, expire_at)| (location, expire_at));
        self
    }

    pub fn location(&self) -> Option<&NarFileLocation> {
        self.location.as_ref().map(|(location, _)| location)
    }

    pub fn expire_at(&self) -> Option<ExpireAt> {
        self.location.as_ref().map(|(_, expire_at)| *expire_at)
    }

    pub fn store_path_hash(&self) -> Option<&StorePathHash> {
        self.store_path_hash.as_ref()
    }

    pub fn check_expiry_and_update(mut self, now: SystemTime) -> Self {
        if self.has_expired(now) {
            self.location = None;
        }
        self
    }

    fn has_expired(&self, now: SystemTime) -> bool {
        self.expire_at().is_none_or(|e| e.has_expired(now))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::common::url::Url;
    use crate::domain::nar_info::model::NarFileName;
    use crate::domain::substituter::model::test_support::make_substituter_meta_with_url;

    use super::*;

    fn make_nar_file(expire_at: SystemTime) -> NarFile {
        let nar_file_name = NarFileName::new("abc123.nar.xz".into()).unwrap();
        let key = NarFileKey::from_file_name(&nar_file_name);

        let location = NarFileLocation::new(
            Url::new("https://example.com/abc123.nar.xz").unwrap(),
            make_substituter_meta_with_url(&Url::new("https://example.com/").unwrap()),
            None,
        );
        NarFile::new(key).on_located(location, ExpireAt::new(expire_at), None)
    }

    #[test]
    fn check_expiry_and_update_clears_location_given_expired() {
        let now = SystemTime::now();
        let expire_at = now - Duration::from_secs(1);

        let nar_file = make_nar_file(expire_at);
        let nar_file = nar_file.check_expiry_and_update(now);

        assert!(nar_file.location().is_none());
    }

    #[test]
    fn check_expiry_and_update_preserves_location_given_not_expired() {
        let now = SystemTime::now();
        let expire_at = now + Duration::from_secs(1);

        let nar_file = make_nar_file(expire_at);
        let nar_file = nar_file.check_expiry_and_update(now);

        assert!(nar_file.location().is_some());
    }
}
