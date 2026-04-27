use std::future::Future;
use std::time::Duration;

use tokio::sync::mpsc::{self, Receiver, Sender};
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
                        self.handle_internal(state, message).await
                    },
                    received = requests.recv() => match received {
                        Some(message) => self.handle_request(state, message).await,
                        None => break,
                    },
                };
            }
        });
    }

    fn handle_request(
        &mut self,
        state: Self::State,
        message: Message<Self::Request>,
    ) -> impl Future<Output = Option<Self::State>> + Send {
        async {
            match message {
                Message::Main(request) => self.on_request(state, request).await,
                Message::Shutdown => {
                    self.on_shutdown(state).await;
                    None
                }
            }
        }
    }

    fn handle_internal(
        &mut self,
        state: Self::State,
        message: Message<Self::Internal>,
    ) -> impl Future<Output = Option<Self::State>> + Send {
        async {
            match message {
                Message::Main(internal) => self.on_internal(state, internal).await,
                Message::Shutdown => {
                    self.on_shutdown(state).await;
                    None
                }
            }
        }
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

    fn on_shutdown(&mut self, state: Self::State) -> impl Future<Output = ()> + Send {
        let _unused = state;
        async {}
    }

    fn dispatch_internal<F>(&mut self, delay: Duration, fut: F)
    where
        F: IntoFuture + Send + 'static,
        F::Output: Into<Message<Self::Internal>>,
        F::IntoFuture: Send,
        Self::Internal: 'static,
    {
        self.context().internal.spawn(async move {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            fut.into_future().await.into()
        });
    }
}

pub struct Context<R, I> {
    requests: Receiver<Message<R>>,
    internal: JoinSet<Message<I>>,
}

impl<R, I> Context<R, I> {
    pub const DEFAULT_REQUESTER_CAPACITY: usize = 64;

    pub fn new(num_requests: usize) -> (Sender<Message<R>>, Self) {
        let (sender, requests) = mpsc::channel(num_requests.max(1));
        let context = Context {
            requests,
            internal: JoinSet::new(),
        };
        (sender, context)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Message<M> {
    Main(M),
    Shutdown,
}

impl<M> From<M> for Message<M> {
    fn from(msg: M) -> Self {
        Self::Main(msg)
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
        let _ = sender
            .send(Message::Main(CounterActorRequest::Increase))
            .await;
        let _ = sender
            .send(Message::Main(CounterActorRequest::AssertEqual(1)))
            .await;
        let _ = sender
            .send(Message::Main(CounterActorRequest::Decrease))
            .await;
        let _ = sender
            .send(Message::Main(CounterActorRequest::AssertEqual(0)))
            .await;
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
        fn new(init: i32) -> (Sender<Message<CounterActorRequest>>, Self) {
            let (sender, context) = Context::new(16);
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
