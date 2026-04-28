use std::sync::Arc;

use async_trait::async_trait;
use moka::future::Cache;
use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, Context, EmptyInternal};

use crate::domain::nar::index::NarFileEvent;
use crate::domain::nar::index::NarFileIndex;
use crate::domain::nar::model::NarFileName;
use crate::domain::substituter::model::Url;

#[derive(Clone)]
pub struct NarFileIndexView {
    cache: Arc<Cache<NarFileName, Url>>,
}

impl NarFileIndexView {
    pub fn new(cache: Arc<Cache<NarFileName, Url>>) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl NarFileIndex for NarFileIndexView {
    async fn get_storage_prefix(&self, nar_file: &NarFileName) -> Option<Url> {
        self.cache.get(nar_file).await
    }
}

pub struct NarFileIndexActor {
    context: Context<NarFileEvent, EmptyInternal>,
    cache: Option<Arc<Cache<NarFileName, Url>>>,
}

impl NarFileIndexActor {
    pub fn new(max_capacity: u64) -> (ActorPre<Self>, NarFileIndexView) {
        let cache = Arc::new(Cache::builder().max_capacity(max_capacity).build());
        let view = NarFileIndexView::new(Arc::clone(&cache));
        let pre = ActorPreBuilder::inject(|context| Self {
            context,
            cache: Some(cache),
        });
        (pre, view)
    }

    async fn apply_event(cache: &Cache<NarFileName, Url>, event: NarFileEvent) {
        match event {
            NarFileEvent::Registered {
                nar_file,
                storage_prefix,
            } => {
                cache.insert(nar_file, storage_prefix).await;
            }
            NarFileEvent::Evicted { nar_file } => {
                cache.remove(&nar_file).await;
            }
        }
    }
}

impl Actor for NarFileIndexActor {
    type Request = NarFileEvent;
    type Internal = EmptyInternal;
    type State = Arc<Cache<NarFileName, Url>>;

    fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
        &mut self.context
    }

    async fn on_start(&mut self) -> Option<Self::State> {
        self.cache.take()
    }

    async fn on_request(
        &mut self,
        state: Self::State,
        event: Self::Request,
    ) -> Option<Self::State> {
        Self::apply_event(&state, event).await;
        Some(state)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::substituter::model::Url;

    use super::*;

    fn make_nar_file(name: &str) -> NarFileName {
        NarFileName::new(name.to_string()).unwrap()
    }

    #[tokio::test]
    async fn apply_event_inserts_entry_given_registered() {
        let cache = Cache::new(100);
        let nar_file = make_nar_file("abc.nar.xz");
        NarFileIndexActor::apply_event(
            &cache,
            NarFileEvent::Registered {
                nar_file: nar_file.clone(),
                storage_prefix: Url::new("https://cache.nixos.org").unwrap(),
            },
        )
        .await;
        assert_eq!(
            cache.get(&nar_file).await,
            Some(Url::new("https://cache.nixos.org").unwrap())
        );
    }

    #[tokio::test]
    async fn apply_event_overwrites_entry_given_duplicate_registered() {
        let cache = Cache::new(100);
        let nar_file = make_nar_file("abc.nar.xz");
        NarFileIndexActor::apply_event(
            &cache,
            NarFileEvent::Registered {
                nar_file: nar_file.clone(),
                storage_prefix: Url::new("https://cache-a.example.com").unwrap(),
            },
        )
        .await;
        NarFileIndexActor::apply_event(
            &cache,
            NarFileEvent::Registered {
                nar_file: nar_file.clone(),
                storage_prefix: Url::new("https://cache-b.example.com").unwrap(),
            },
        )
        .await;
        assert_eq!(
            cache.get(&nar_file).await,
            Some(Url::new("https://cache-b.example.com").unwrap())
        );
    }

    #[tokio::test]
    async fn apply_event_removes_entry_given_evicted() {
        let cache = Cache::new(100);
        let nar_file = make_nar_file("abc.nar.xz");
        NarFileIndexActor::apply_event(
            &cache,
            NarFileEvent::Registered {
                nar_file: nar_file.clone(),
                storage_prefix: Url::new("https://cache.nixos.org").unwrap(),
            },
        )
        .await;
        NarFileIndexActor::apply_event(
            &cache,
            NarFileEvent::Evicted {
                nar_file: nar_file.clone(),
            },
        )
        .await;
        assert!(cache.get(&nar_file).await.is_none());
    }

    #[tokio::test]
    async fn apply_event_is_noop_given_unknown_evicted() {
        let cache = Cache::new(100);
        let nar_file = make_nar_file("abc.nar.xz");
        let other = make_nar_file("other.nar.xz");
        NarFileIndexActor::apply_event(
            &cache,
            NarFileEvent::Registered {
                nar_file: nar_file.clone(),
                storage_prefix: Url::new("https://cache.nixos.org").unwrap(),
            },
        )
        .await;
        NarFileIndexActor::apply_event(&cache, NarFileEvent::Evicted { nar_file: other }).await;
        assert!(cache.get(&nar_file).await.is_some());
    }
}
