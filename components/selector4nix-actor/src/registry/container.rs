use std::borrow::Borrow;
use std::hash::Hash;
use std::pin::Pin;

use moka::future::Cache;

use crate::actor::{Actor, Address};

pub struct Registry<K, A, F>
where
    A: Actor,
{
    actors: Cache<K, Address<A>>,
    factory: F,
}

impl<K, A, F> Registry<K, A, F>
where
    A: Actor,
{
    pub fn new(actors: Cache<K, Address<A>>, factory: F) -> Self {
        Self { actors, factory }
    }
}

impl<K, A, F> Registry<K, A, F>
where
    K: Eq + Hash + Send + Sync + 'static,
    A: Actor + 'static,
{
    pub async fn get_with<FR, R>(&self, key: &K, factory: FR) -> Address<A>
    where
        K: Clone,
        FR: FnOnce(&K) -> R,
        R: Future<Output = Address<A>>,
    {
        let fut = factory(key);
        self.actors.get_with_by_ref(key, fut).await
    }

    pub async fn insert(&self, key: K, address: Address<A>) {
        self.actors.insert(key, address).await;
    }

    pub async fn remove<Q>(&self, key: &Q)
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.actors.invalidate(key).await
    }

    pub async fn clear(&self) {
        self.actors.invalidate_all();
        self.actors.run_pending_tasks().await;
    }
}

impl<K, A> Registry<K, A, AsyncFactory<K, A>>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    A: Actor + 'static,
{
    pub async fn get(&self, key: &K) -> Address<A> {
        let fut = self.factory.create(key);
        self.actors.get_with_by_ref(key, fut).await
    }
}

impl<K, A> Registry<K, A, SyncFactory<K, A>>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    A: Actor + 'static,
{
    pub async fn get(&self, key: &K) -> Address<A> {
        let fut = async { self.factory.create(key) };
        self.actors.get_with_by_ref(key, fut).await
    }
}

impl<K, A> From<Cache<K, Address<A>>> for Registry<K, A, NoFactory>
where
    A: Actor,
{
    fn from(actors: Cache<K, Address<A>>) -> Self {
        Self {
            actors,
            factory: NoFactory,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct NoFactory;

type AddressFuture<A> = Pin<Box<dyn Future<Output = Address<A>> + 'static>>;

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
        R: Future<Output = Address<A>> + 'static,
    {
        Self(Box::new(move |key| Box::pin(factory(key))))
    }

    pub fn create(&self, key: &K) -> Pin<Box<dyn Future<Output = Address<A>>>> {
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

    use crate::actor::{ActorPreBuilder, Context, EmptyInternal, Message};
    use crate::registry::{CapacityOption, RegistryBuilder};

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
    async fn remove_then_get_creates_new_address() {
        let counter = Arc::new(AtomicUsize::new(0));
        let factory = tracked_sync_factory(counter.clone());
        let registry = RegistryBuilder::new().factory(factory).build();

        let original = registry.get(&"a".to_string()).await;
        registry.remove("a").await;

        let recreated = registry.get(&"a".to_string()).await;
        assert!(!original.is_same(&recreated));
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn eviction_sends_shutdown() {
        let registry = RegistryBuilder::new()
            .capacity(CapacityOption::Lru(1))
            .build();

        let (addr, mut rx) = Address::<TestActor>::mock();
        registry.insert("a".to_string(), addr).await;

        registry.clear().await;

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, Message::Shutdown));
    }
}
