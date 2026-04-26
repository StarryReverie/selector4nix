use std::time::Duration;

use moka::future::{Cache, FutureExt};
use tokio::sync::mpsc::Sender as MpscSender;

use crate::domain::nar::actor::NarMessage;
use crate::domain::nar::model::StorePathHash;

pub struct NarActorRegistry {
    actors: Cache<StorePathHash, MpscSender<NarMessage>>,
}

impl NarActorRegistry {
    pub fn new(capacity: u64, ttl: Duration) -> Self {
        let actors = Cache::builder()
            .max_capacity(capacity)
            .time_to_idle(ttl)
            .async_eviction_listener(|_hash, sender: MpscSender<NarMessage>, _cause| {
                async move {
                    let _ = sender.send(NarMessage::Evict).await;
                }
                .boxed()
            })
            .build();
        Self { actors }
    }

    pub async fn get_or_create<F>(&self, hash: StorePathHash, factory: F) -> MpscSender<NarMessage>
    where
        F: FnOnce(&StorePathHash) -> MpscSender<NarMessage>,
    {
        self.actors
            .get_with_by_ref(&hash, async { factory(&hash) })
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc;

    use super::*;

    fn make_hash() -> StorePathHash {
        StorePathHash::new(format!("{:0<32}", "0")).unwrap()
    }

    #[tokio::test]
    async fn get_or_create_returns_sender_given_new_hash() {
        let registry = NarActorRegistry::new(100, Duration::from_secs(300));
        let hash = make_hash();

        let (tx, mut rx) = mpsc::channel(32);

        let sender = registry.get_or_create(hash, |_| tx).await;
        sender.send(NarMessage::Evict).await.unwrap();

        assert!(matches!(rx.recv().await.unwrap(), NarMessage::Evict));
    }

    #[tokio::test]
    async fn get_or_create_returns_existing_sender_given_known_hash() {
        let registry = NarActorRegistry::new(100, Duration::from_secs(300));
        let hash = make_hash();

        let (tx1, _) = mpsc::channel(32);
        let first = registry.get_or_create(hash.clone(), |_| tx1).await;

        let (tx2, _) = mpsc::channel(32);
        let second = registry.get_or_create(hash, |_| tx2).await;

        assert!(first.same_channel(&second));
    }
}
