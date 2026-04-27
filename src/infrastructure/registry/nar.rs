use std::time::Duration;

use moka::future::{Cache, FutureExt};
use selector4nix_actor::actor::Address;

use crate::domain::nar::actor::NarActor;
use crate::domain::nar::model::StorePathHash;

pub struct NarActorRegistry {
    actors: Cache<StorePathHash, Address<NarActor>>,
}

impl NarActorRegistry {
    pub fn new(capacity: u64, ttl: Duration) -> Self {
        let actors = Cache::builder()
            .max_capacity(capacity)
            .time_to_idle(ttl)
            .async_eviction_listener(|_hash, address: Address<NarActor>, _cause| {
                address.shutdown().boxed()
            })
            .build();
        Self { actors }
    }

    pub async fn get_or_create<F>(&self, hash: StorePathHash, factory: F) -> Address<NarActor>
    where
        F: FnOnce(&StorePathHash) -> Address<NarActor>,
    {
        self.actors
            .get_with_by_ref(&hash, async { factory(&hash) })
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use selector4nix_actor::actor::Message;

    use super::*;

    fn make_hash() -> StorePathHash {
        StorePathHash::new(format!("{:0<32}", "0")).unwrap()
    }

    #[tokio::test]
    async fn get_or_create_returns_sender_given_new_hash() {
        let registry = NarActorRegistry::new(100, Duration::from_secs(300));
        let hash = make_hash();

        let (tx, mut rx) = Address::mock();

        let address = registry.get_or_create(hash, |_| tx).await;
        address.shutdown().await;

        assert!(matches!(rx.recv().await.unwrap(), Message::Shutdown));
    }

    #[tokio::test]
    async fn get_or_create_returns_existing_sender_given_known_hash() {
        let registry = NarActorRegistry::new(100, Duration::from_secs(300));
        let hash = make_hash();

        let (tx1, _) = Address::mock();
        let first = registry.get_or_create(hash.clone(), |_| tx1).await;

        let (tx2, _) = Address::mock();
        let second = registry.get_or_create(hash, |_| tx2).await;

        assert!(first.is_same(&second));
    }
}
