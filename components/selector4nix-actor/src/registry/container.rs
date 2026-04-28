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
    pub async fn get_with<FR, R>(&self, key: K, factory: FR) -> Address<A>
    where
        FR: FnOnce(&K) -> R,
        R: Future<Output = Address<A>>,
    {
        let fut = factory(&key);
        self.actors.get_with(key, fut).await
    }

    pub async fn insert(&self, key: K, address: Address<A>) {
        self.actors.insert(key, address).await;
    }

    pub async fn remove<Q>(&self, key: Q)
    where
        Q: Borrow<K>,
    {
        self.actors.invalidate(key.borrow()).await
    }
}

impl<K, A> Registry<K, A, AsyncFactory<K, A>>
where
    K: Eq + Hash + Send + Sync + 'static,
    A: Actor + 'static,
{
    pub async fn get(&self, key: K) -> Address<A> {
        let fut = self.factory.create(&key);
        self.actors.get_with(key, fut).await
    }
}

impl<K, A> Registry<K, A, SyncFactory<K, A>>
where
    K: Eq + Hash + Send + Sync + 'static,
    A: Actor + 'static,
{
    pub async fn get(&self, key: K) -> Address<A> {
        let address = self.factory.create(&key);
        self.actors.get_with(key, async { address }).await
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

    pub fn create<Q>(&self, key: Q) -> Pin<Box<dyn Future<Output = Address<A>>>>
    where
        Q: Borrow<K>,
    {
        (self.0)(key.borrow())
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

    pub fn create<Q>(&self, key: Q) -> Address<A>
    where
        Q: Borrow<K>,
    {
        (self.0)(key.borrow())
    }
}
