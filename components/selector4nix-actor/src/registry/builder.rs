use std::hash::Hash;
use std::marker::PhantomData;
use std::time::Duration;

use moka::future::{Cache, FutureExt};

use crate::actor::{Actor, Address};
use crate::registry::{NoFactory, Registry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CapacityOption {
    #[default]
    Unlimited,
    Lru(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ExpirationOption {
    #[default]
    Permanent,
    Ttl(Duration),
    Tti(Duration),
}

pub struct RegistryBuilder<K, A, F> {
    capacity: CapacityOption,
    expiration: ExpirationOption,
    factory: F,
    _marker: PhantomData<(K, A, F)>,
}

impl<K, A, F> RegistryBuilder<K, A, F> {
    pub fn capacity(mut self, capacity: CapacityOption) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn expiration(mut self, expiration: ExpirationOption) -> Self {
        self.expiration = expiration;
        self
    }

    pub fn factory<F2>(self, factory: F2) -> RegistryBuilder<K, A, F2> {
        RegistryBuilder {
            capacity: self.capacity,
            expiration: self.expiration,
            factory,
            _marker: PhantomData,
        }
    }

    pub fn build(self) -> Registry<K, A, F>
    where
        K: Eq + Hash + Send + Sync + 'static,
        A: Actor + 'static,
    {
        let max_capacity = match self.capacity {
            CapacityOption::Unlimited => u64::MAX,
            CapacityOption::Lru(max_capacity) => max_capacity as u64,
        };
        let builder = Cache::builder().max_capacity(max_capacity);
        let builder = match self.expiration {
            ExpirationOption::Permanent => builder,
            ExpirationOption::Ttl(duration) => builder.time_to_live(duration),
            ExpirationOption::Tti(duration) => builder.time_to_idle(duration),
        };
        let actors = builder
            .async_eviction_listener(|_hash, address: Address<A>, _cause| {
                address.shutdown().boxed()
            })
            .build();
        Registry::new(actors, self.factory)
    }
}

impl<K, A> RegistryBuilder<K, A, NoFactory> {
    pub fn new() -> Self {
        Self {
            capacity: CapacityOption::default(),
            expiration: ExpirationOption::default(),
            factory: NoFactory,
            _marker: PhantomData,
        }
    }
}

impl<K, A> Default for RegistryBuilder<K, A, NoFactory> {
    fn default() -> Self {
        Self::new()
    }
}
