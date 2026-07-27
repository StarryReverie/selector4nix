use std::future::Future;
use std::time::Duration;

use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};
use tokio::sync::watch::{self, Receiver as WatchReceiver, Sender as WatchSender};
use tokio::task::JoinSet;

pub trait Actor: Send {
    type Request: Send;
    type Internal: Send;
    type State: Send;

    fn context(&mut self) -> &mut Context<Self::Request, Self::Internal>;

    fn run(mut self)
    where
        Self: Sized + 'static,
    {
        tokio::spawn(async move {
            let mut next_state = self.on_start().await;
            while let Some(state) = next_state {
                let context = self.context();
                let requests = &mut context.requests;
                let internal = &mut context.internal;

                next_state = tokio::select! {
                    Some(Ok(message)) = internal.join_next(), if !internal.is_empty() => {
                        self.on_internal(state, message).await
                    },
                    received = requests.recv() => match received {
                        Some(message) => self.on_request(state, message).await,
                        None => break,
                    },
                };
            }
            self.on_shutdown().await;
            let _ = self.context().terminated.send(true);
        });
    }

    fn on_start(&mut self) -> impl Future<Output = Option<Self::State>> + Send;

    fn on_request(
        &mut self,
        state: Self::State,
        request: Self::Request,
    ) -> impl Future<Output = Option<Self::State>> + Send;

    fn on_internal(
        &mut self,
        state: Self::State,
        internal: Self::Internal,
    ) -> impl Future<Output = Option<Self::State>> + Send {
        let _unused = internal;
        async { Some(state) }
    }

    fn on_shutdown(&mut self) -> impl Future<Output = ()> + Send {
        async {}
    }

    fn dispatch_internal<F>(&mut self, delay: Duration, fut: F)
    where
        F: IntoFuture<Output = Self::Internal> + Send + 'static,
        F::IntoFuture: Send,
        Self::Internal: 'static,
    {
        self.context().internal.spawn(async move {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            fut.into_future().await
        });
    }
}

pub struct Context<R, I> {
    requests: MpscReceiver<R>,
    internal: JoinSet<I>,
    terminated: WatchSender<bool>,
}

impl<R, I> Context<R, I> {
    pub const DEFAULT_REQUESTER_CAPACITY: usize = 64;

    pub fn new(num_requests: usize) -> (MpscSender<R>, WatchReceiver<bool>, Self) {
        let (sender, requests) = mpsc::channel(num_requests.max(1));
        let (terminated, terminated_rx) = watch::channel(false);
        let context = Context {
            requests,
            internal: JoinSet::new(),
            terminated,
        };
        (sender, terminated_rx, context)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EmptyInternal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn actor_handle_requests_succeeds() {
        let (sender, actor) = CounterActor::new(0);
        actor.run();
        let _ = sender.send(CounterActorRequest::Increase).await;
        let _ = sender.send(CounterActorRequest::AssertEqual(1)).await;
        let _ = sender.send(CounterActorRequest::Decrease).await;
        let _ = sender.send(CounterActorRequest::AssertEqual(0)).await;
    }

    enum CounterActorRequest {
        Increase,
        Decrease,
        AssertEqual(i32),
    }

    struct CounterActor {
        context: Context<CounterActorRequest, EmptyInternal>,
        init: Option<i32>,
    }

    impl CounterActor {
        fn new(init: i32) -> (MpscSender<CounterActorRequest>, Self) {
            let (sender, _, context) = Context::new(16);
            let actor = Self {
                context,
                init: Some(init),
            };
            (sender, actor)
        }
    }

    impl Actor for CounterActor {
        type Request = CounterActorRequest;
        type Internal = EmptyInternal;
        type State = i32;

        fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
            &mut self.context
        }

        async fn on_start(&mut self) -> Option<Self::State> {
            self.init.take()
        }

        async fn on_request(
            &mut self,
            state: Self::State,
            request: Self::Request,
        ) -> Option<Self::State> {
            match request {
                CounterActorRequest::Increase => Some(state.saturating_add(1)),
                CounterActorRequest::Decrease => Some(state.saturating_sub(1)),
                CounterActorRequest::AssertEqual(expected) => {
                    assert_eq!(state, expected);
                    Some(state)
                }
            }
        }
    }
}
