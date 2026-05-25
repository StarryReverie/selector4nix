use std::marker::PhantomData;

use tokio::sync::watch::Receiver as WatchReceiver;

use crate::actor::{Actor, Address, AnyAddress, Context};

pub struct ActorPre<A: Actor> {
    address: Address<A>,
    terminated: WatchReceiver<bool>,
    actor: A,
}

impl<A: Actor> ActorPre<A> {
    pub fn new(address: Address<A>, terminated: WatchReceiver<bool>, actor: A) -> Self {
        Self {
            address,
            terminated,
            actor,
        }
    }

    pub fn address(&self) -> Address<A> {
        self.address.clone()
    }

    pub fn terminated(&self) -> WatchReceiver<bool> {
        self.terminated.clone()
    }

    pub fn run(self) -> Address<A>
    where
        A: 'static,
    {
        self.actor.run();
        self.address
    }
}

pub struct ActorPreBuilder<A: Actor> {
    capacity: usize,
    _marker: PhantomData<A>,
}

impl<A: Actor> ActorPreBuilder<A> {
    pub fn new() -> Self {
        Self {
            capacity: Context::<A::Request, A::Internal>::DEFAULT_REQUESTER_CAPACITY,
            _marker: PhantomData,
        }
    }

    pub fn inject<P>(provider: P) -> ActorPre<A>
    where
        P: FnOnce(Context<A::Request, A::Internal>) -> A,
    {
        Self::new().build(provider)
    }

    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn build<P>(self, provider: P) -> ActorPre<A>
    where
        P: FnOnce(Context<A::Request, A::Internal>) -> A,
    {
        let (sender, terminated, context) = Context::new(self.capacity);
        ActorPre::new(
            Address::from(AnyAddress::new(sender, terminated.clone())),
            terminated,
            provider(context),
        )
    }
}

impl<A: Actor> Default for ActorPreBuilder<A> {
    fn default() -> Self {
        Self::new()
    }
}
