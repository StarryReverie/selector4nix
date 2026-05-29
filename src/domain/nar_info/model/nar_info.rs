use std::time::SystemTime;

use getset::Getters;

use crate::domain::common::expire_at::ExpireAt;
use crate::domain::common::url::Url;
use crate::domain::nar_info::model::{NarInfoResolution, ProxyNarInfoData, StorePathHash};

#[derive(Debug, Clone, PartialEq, Eq, Getters)]
pub struct NarInfo {
    #[getset(get = "pub")]
    hash: StorePathHash,
    resolution: Option<(NarInfoResolution, ExpireAt)>,
}

impl NarInfo {
    pub fn new(hash: StorePathHash) -> Self {
        Self {
            hash,
            resolution: None,
        }
    }

    pub fn on_resolved(mut self, resolution: NarInfoResolution, expire_at: ExpireAt) -> Self {
        self.resolution = Some((resolution, expire_at));
        self
    }

    pub fn resolution(&self) -> Option<&NarInfoResolution> {
        self.resolution.as_ref().map(|(r, _)| r)
    }

    pub fn expire_at(&self) -> Option<ExpireAt> {
        self.resolution.as_ref().map(|(_, e)| *e)
    }

    pub fn nar_info(&self) -> Option<&ProxyNarInfoData> {
        self.resolution().and_then(NarInfoResolution::nar_info)
    }

    pub fn source_url(&self) -> Option<&Url> {
        self.resolution().and_then(NarInfoResolution::source_url)
    }

    pub fn check_expiry_and_update(self, now: SystemTime) -> Self {
        if self.has_expired(now) {
            Self {
                hash: self.hash,
                resolution: None,
            }
        } else {
            self
        }
    }

    fn has_expired(&self, now: SystemTime) -> bool {
        self.expire_at().is_none_or(|e| e.has_expired(now))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn make_hash() -> StorePathHash {
        StorePathHash::new("p4pclmv1gyja5kzc26npqpia1qqxrf0l".into()).unwrap()
    }

    fn make_nar_info(expire_at: SystemTime) -> NarInfo {
        let hash = make_hash();
        let nar = NarInfo::new(hash.clone());
        nar.on_resolved(NarInfoResolution::NotFound, ExpireAt::new(expire_at))
    }

    #[test]
    fn check_expiry_and_update_changed_to_unknown_given_expired() {
        let now = SystemTime::now();
        let expire_at = now - Duration::from_secs(1);

        let nar_info = make_nar_info(expire_at);
        let nar_info = nar_info.check_expiry_and_update(now);

        assert!(nar_info.resolution().is_none());
    }

    #[test]
    fn check_expiry_and_update_kept_unchanged_given_not_expired() {
        let now = SystemTime::now();
        let expire_at = now + Duration::from_secs(1);

        let nar_info = make_nar_info(expire_at);
        let nar_info = nar_info.check_expiry_and_update(now);

        assert!(nar_info.resolution().is_some());
    }
}
