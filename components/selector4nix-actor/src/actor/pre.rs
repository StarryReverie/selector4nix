use std::marker::PhantomData;

use crate::actor::{Actor, Address, Context};

pub struct ActorPre<A: Actor> {
    address: Address<A>,
    actor: A,
}

impl<A: Actor> ActorPre<A> {
    pub fn new(address: Address<A>, actor: A) -> Self {
        Self { address, actor }
    }

    pub fn address(&self) -> Address<A> {
        self.address.clone()
    }

    pub fn run<S>(self, state: S) -> Address<A>
    where
        A: 'static,
        S: Into<A::State>,
    {
        self.actor.run(state);
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
        let (sender, context) = Context::new(self.capacity);
        ActorPre::new(Address::from(sender), provider(context))
    }
}

impl<A: Actor> Default for ActorPreBuilder<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::actor::EmptyInternal;

    use super::*;

    #[tokio::test]
    async fn actor_pre_builder_succeeds() {
        let addr = NoopActor::new().run(0);
        addr.shutdown().await;
    }

    enum NoopRequest {}

    struct NoopActor {
        context: Context<NoopRequest, EmptyInternal>,
    }

    impl NoopActor {
        fn new() -> ActorPre<Self> {
            ActorPreBuilder::inject(|context| Self { context })
        }
    }

    impl Actor for NoopActor {
        type Request = NoopRequest;
        type Internal = EmptyInternal;
        type State = i32;

        fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
            &mut self.context
        }

        async fn on_request(
            &mut self,
            _state: Self::State,
            _request: Self::Request,
        ) -> Option<Self::State> {
            unreachable!()
        }
    }
}
