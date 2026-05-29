use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExpireAt(u64);

impl ExpireAt {
    pub fn new(time: SystemTime) -> Self {
        let unix_timestamp = time
            .duration_since(UNIX_EPOCH)
            .expect("`UNIX_EPOCH` should be earlier than any other system time")
            .as_secs();
        Self(unix_timestamp)
    }

    pub fn since(created_at: SystemTime, ttl: Duration) -> Self {
        Self::new(created_at + ttl)
    }

    pub fn unix_timpstamp(&self) -> u64 {
        self.0
    }

    pub fn has_expired(&self, now: SystemTime) -> bool {
        let expire_at = UNIX_EPOCH + Duration::from_secs(self.unix_timpstamp());
        expire_at <= now
    }
}
