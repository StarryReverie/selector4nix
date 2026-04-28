use std::sync::Arc;

use async_trait::async_trait;
use moka::future::Cache;
use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, Context, EmptyInternal};

use crate::domain::nar::index::NarPathEvent;
use crate::domain::nar::index::NarPathIndex;
use crate::domain::substituter::model::Url;

#[derive(Clone)]
pub struct NarPathIndexView {
    cache: Arc<Cache<String, Url>>,
}

impl NarPathIndexView {
    pub fn new(cache: Arc<Cache<String, Url>>) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl NarPathIndex for NarPathIndexView {
    async fn get_storage_prefix(&self, nar_path: &str) -> Option<Url> {
        self.cache.get(&nar_path.to_string()).await
    }
}

pub struct NarPathIndexActor {
    context: Context<NarPathEvent, EmptyInternal>,
    cache: Option<Arc<Cache<String, Url>>>,
}

impl NarPathIndexActor {
    pub fn new(max_capacity: u64) -> (ActorPre<Self>, NarPathIndexView) {
        let cache = Arc::new(Cache::builder().max_capacity(max_capacity).build());
        let view = NarPathIndexView::new(Arc::clone(&cache));
        let pre = ActorPreBuilder::inject(|context| Self {
            context,
            cache: Some(cache),
        });
        (pre, view)
    }

    async fn apply_event(cache: &Cache<String, Url>, event: NarPathEvent) {
        match event {
            NarPathEvent::Registered {
                nar_path,
                storage_prefix,
            } => {
                cache.insert(nar_path, storage_prefix).await;
            }
            NarPathEvent::Evicted { nar_path } => {
                cache.remove(&nar_path).await;
            }
        }
    }
}

impl Actor for NarPathIndexActor {
    type Request = NarPathEvent;
    type Internal = EmptyInternal;
    type State = Arc<Cache<String, Url>>;

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
        Self::apply_event(&*state, event).await;
        Some(state)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::substituter::model::Url;

    use super::*;

    #[tokio::test]
    async fn apply_event_inserts_entry_given_registered() {
        let cache = Cache::new(100);
        NarPathIndexActor::apply_event(
            &cache,
            NarPathEvent::Registered {
                nar_path: "nar/abc.nar.xz".to_string(),
                storage_prefix: Url::new("https://cache.nixos.org").unwrap(),
            },
        )
        .await;
        assert_eq!(
            cache.get("nar/abc.nar.xz").await,
            Some(Url::new("https://cache.nixos.org").unwrap())
        );
    }

    #[tokio::test]
    async fn apply_event_overwrites_entry_given_duplicate_registered() {
        let cache = Cache::new(100);
        NarPathIndexActor::apply_event(
            &cache,
            NarPathEvent::Registered {
                nar_path: "nar/abc.nar.xz".to_string(),
                storage_prefix: Url::new("https://cache-a.example.com").unwrap(),
            },
        )
        .await;
        NarPathIndexActor::apply_event(
            &cache,
            NarPathEvent::Registered {
                nar_path: "nar/abc.nar.xz".to_string(),
                storage_prefix: Url::new("https://cache-b.example.com").unwrap(),
            },
        )
        .await;
        assert_eq!(
            cache.get("nar/abc.nar.xz").await,
            Some(Url::new("https://cache-b.example.com").unwrap())
        );
    }

    #[tokio::test]
    async fn apply_event_removes_entry_given_evicted() {
        let cache = Cache::new(100);
        NarPathIndexActor::apply_event(
            &cache,
            NarPathEvent::Registered {
                nar_path: "nar/abc.nar.xz".to_string(),
                storage_prefix: Url::new("https://cache.nixos.org").unwrap(),
            },
        )
        .await;
        NarPathIndexActor::apply_event(
            &cache,
            NarPathEvent::Evicted {
                nar_path: "nar/abc.nar.xz".to_string(),
            },
        )
        .await;
        assert!(cache.get("nar/abc.nar.xz").await.is_none());
    }

    #[tokio::test]
    async fn apply_event_is_noop_given_unknown_evicted() {
        let cache = Cache::new(100);
        NarPathIndexActor::apply_event(
            &cache,
            NarPathEvent::Registered {
                nar_path: "nar/abc.nar.xz".to_string(),
                storage_prefix: Url::new("https://cache.nixos.org").unwrap(),
            },
        )
        .await;
        NarPathIndexActor::apply_event(
            &cache,
            NarPathEvent::Evicted {
                nar_path: "nar/other.nar.xz".to_string(),
            },
        )
        .await;
        assert!(cache.get("nar/abc.nar.xz").await.is_some());
    }
}
