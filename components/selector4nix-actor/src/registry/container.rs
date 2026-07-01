use std::hash::Hash;
use std::pin::Pin;
use std::sync::Arc;

use dashmap::DashMap;
use moka::future::Cache;
use tokio::sync::watch::Receiver as WatchReceiver;

use crate::actor::{Actor, Address};

pub struct PendingTermination<K> {
    inner: Arc<DashMap<K, WatchReceiver<bool>>>,
}

impl<K> PendingTermination<K>
where
    K: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    pub fn insert(&self, key: K, terminated: WatchReceiver<bool>) {
        self.inner.insert(key, terminated);
    }

    pub async fn drain(&self, key: &K) {
        loop {
            let mut terminated = match self.inner.get(key) {
                Some(guard) => guard.value().clone(),
                None => return,
            };
            if *terminated.borrow() {
                self.inner.remove(key);
                return;
            }
            if terminated.changed().await.is_err() {
                self.inner.remove(key);
                return;
            }
        }
    }
}

impl<K> Clone for PendingTermination<K> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

pub struct Registry<K, A, F = AsyncFactory<K, A>>
where
    A: Actor,
{
    actors: Cache<K, Address<A>>,
    pending_termination: PendingTermination<K>,
    factory: F,
}

impl<K, A, F> Registry<K, A, F>
where
    A: Actor,
{
    pub fn new(
        actors: Cache<K, Address<A>>,
        pending_termination: PendingTermination<K>,
        factory: F,
    ) -> Self {
        Self {
            actors,
            pending_termination,
            factory,
        }
    }
}

impl<K, A, F> Registry<K, A, F>
where
    K: Eq + Hash + Send + Sync + 'static,
    A: Actor + 'static,
{
    pub async fn insert(&self, key: K, address: Address<A>) {
        self.actors.insert(key, address).await;
    }

    pub async fn interrupt(&self, key: &K) {
        self.actors.invalidate(key).await;
        self.actors.run_pending_tasks().await;
    }

    pub async fn interrupt_all(&self) {
        self.actors.invalidate_all();
        self.actors.run_pending_tasks().await;
    }

    pub async fn entry_count(&self) -> u64 {
        self.actors.entry_count()
    }
}

impl<K, A, F> Registry<K, A, F>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    A: Actor + 'static,
{
    pub async fn get_with<FR, R>(&self, key: &K, factory: FR) -> Address<A>
    where
        FR: FnOnce(&K) -> R,
        R: Future<Output = Address<A>>,
    {
        if let Some(addr) = self.actors.get(key).await
            && !addr.is_closed()
        {
            return addr;
        }
        self.actors.invalidate(key).await;
        self.actors.run_pending_tasks().await;
        self.pending_termination.drain(key).await;
        let fut = factory(key);
        self.actors.get_with_by_ref(key, fut).await
    }
}

impl<K, A> Registry<K, A, AsyncFactory<K, A>>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    A: Actor + 'static,
{
    pub async fn get(&self, key: &K) -> Address<A> {
        self.get_with(key, move |_| self.factory.create(key)).await
    }
}

impl<K, A> Registry<K, A, SyncFactory<K, A>>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    A: Actor + 'static,
{
    pub async fn get(&self, key: &K) -> Address<A> {
        self.get_with(key, move |_| async { self.factory.create(key) })
            .await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct NoFactory;

type AddressFuture<A> = Pin<Box<dyn Future<Output = Address<A>> + Send + 'static>>;

#[allow(clippy::type_complexity)]
pub struct AsyncFactory<K, A>(Box<dyn Fn(&K) -> AddressFuture<A> + Send + Sync + 'static>)
where
    A: Actor;

impl<K, A> AsyncFactory<K, A>
where
    A: Actor,
{
    pub fn new<FR, R>(factory: FR) -> Self
    where
        FR: Fn(&K) -> R + Send + Sync + 'static,
        R: Future<Output = Address<A>> + Send + 'static,
    {
        Self(Box::new(move |key| Box::pin(factory(key))))
    }

    pub fn create(&self, key: &K) -> Pin<Box<dyn Future<Output = Address<A>> + Send + 'static>> {
        (self.0)(key)
    }
}

#[allow(clippy::type_complexity)]
pub struct SyncFactory<K, A>(Box<dyn Fn(&K) -> Address<A> + Send + Sync + 'static>)
where
    A: Actor;

impl<K, A> SyncFactory<K, A>
where
    A: Actor,
{
    pub fn new<FR>(factory: FR) -> Self
    where
        FR: Fn(&K) -> Address<A> + Send + Sync + 'static,
    {
        Self(Box::new(factory))
    }

    pub fn create(&self, key: &K) -> Address<A> {
        (self.0)(key)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::actor::{ActorPreBuilder, Context, EmptyInternal};
    use crate::registry::RegistryBuilder;

    use super::*;

    #[derive(Debug)]
    enum TestRequest {}

    struct TestActor {
        context: Context<TestRequest, EmptyInternal>,
    }

    impl Actor for TestActor {
        type Request = TestRequest;
        type Internal = EmptyInternal;
        type State = ();

        fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
            &mut self.context
        }

        async fn on_start(&mut self) -> Option<Self::State> {
            Some(())
        }

        async fn on_request(
            &mut self,
            _state: Self::State,
            _request: Self::Request,
        ) -> Option<Self::State> {
            unreachable!()
        }
    }

    fn make_address() -> Address<TestActor> {
        ActorPreBuilder::inject(|context| TestActor { context }).run()
    }

    fn make_unlimited_registry() -> Registry<String, TestActor, NoFactory> {
        RegistryBuilder::new().build()
    }

    fn tracked_sync_factory(counter: Arc<AtomicUsize>) -> SyncFactory<String, TestActor> {
        SyncFactory::new(move |_key| {
            counter.fetch_add(1, Ordering::Relaxed);
            make_address()
        })
    }

    fn tracked_async_factory(counter: Arc<AtomicUsize>) -> AsyncFactory<String, TestActor> {
        AsyncFactory::new(move |_key| {
            let addr = make_address();
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::Relaxed);
                addr
            }
        })
    }

    #[tokio::test]
    async fn sync_get_returns_same_address_for_same_key() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = tracked_sync_factory(counter.clone());
        let registry = RegistryBuilder::new().factory(factory).build();

        let first = registry.get(&"a".to_string()).await;
        let second = registry.get(&"a".to_string()).await;

        assert!(first.is_same(&second));
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn sync_get_returns_different_address_for_different_keys() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = tracked_sync_factory(counter.clone());
        let registry = RegistryBuilder::new().factory(factory).build();

        let a = registry.get(&"a".to_string()).await;
        let b = registry.get(&"b".to_string()).await;

        assert!(!a.is_same(&b));
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn async_get_returns_same_address_for_same_key() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = tracked_async_factory(counter.clone());
        let registry = RegistryBuilder::new().factory(factory).build();

        let first = registry.get(&"a".to_string()).await;
        let second = registry.get(&"a".to_string()).await;

        assert!(first.is_same(&second));
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn async_get_returns_different_address_for_different_keys() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = tracked_async_factory(counter.clone());
        let registry = RegistryBuilder::new().factory(factory).build();

        let a = registry.get(&"a".to_string()).await;
        let b = registry.get(&"b".to_string()).await;

        assert!(!a.is_same(&b));
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn get_with_is_idempotent() {
        let registry = make_unlimited_registry();
        let counter = Arc::new(AtomicUsize::new(0));

        let first = registry
            .get_with(&"a".to_string(), |_key| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    make_address()
                }
            })
            .await;

        let second = registry
            .get_with(&"a".to_string(), |_key| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    make_address()
                }
            })
            .await;

        assert!(first.is_same(&second));
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn get_with_creates_new_for_new_key() {
        let registry = make_unlimited_registry();

        let a = registry
            .get_with(&"a".to_string(), |_| async { make_address() })
            .await;
        let b = registry
            .get_with(&"b".to_string(), |_| async { make_address() })
            .await;

        assert!(!a.is_same(&b));
    }

    #[tokio::test]
    async fn insert_then_get_returns_same_address() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = tracked_sync_factory(counter.clone());
        let registry = RegistryBuilder::new().factory(factory).build();

        let addr = make_address();
        registry.insert("a".to_string(), addr.clone()).await;

        let got = registry.get(&"a".to_string()).await;
        assert!(got.is_same(&addr));
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn interrupt_then_get_creates_new_address() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = tracked_sync_factory(counter.clone());
        let registry = RegistryBuilder::new().factory(factory).build();

        let original = registry.get(&"a".to_string()).await;
        assert!(!original.is_closed());
        drop(original);

        registry.interrupt(&"a".to_string()).await;

        registry.get(&"a".to_string()).await;
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn concurrent_get_same_key_only_creates_one_actor() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = tracked_sync_factory(counter.clone());
        let registry = Arc::new(RegistryBuilder::new().factory(factory).build());

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let reg = registry.clone();
                tokio::spawn(async move { reg.get(&"a".to_string()).await })
            })
            .collect();

        let addresses: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|h| h.unwrap())
            .collect();

        assert_eq!(counter.load(Ordering::Relaxed), 1);
        for i in 1..addresses.len() {
            assert!(addresses[0].is_same(&addresses[i]));
        }
    }
}
