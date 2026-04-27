use tokio::sync::mpsc::error::{SendError, TrySendError};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::actor::{Actor, Message};

#[derive(Debug)]
pub struct Address<A: Actor> {
    inner: AnyAddress<A::Request>,
}

impl<A: Actor> Address<A> {
    pub fn mock() -> (Self, Receiver<Message<A::Request>>) {
        let (inner, receiver) = AnyAddress::mock();
        (Self { inner }, receiver)
    }

    pub fn erased(self) -> AnyAddress<A::Request> {
        self.inner
    }

    pub async fn tell(&self, request: A::Request) -> Result<(), TellError<A::Request>> {
        self.inner.tell(request).await
    }

    pub fn try_tell(&self, request: A::Request) -> Result<(), TryTellError<A::Request>> {
        self.inner.try_tell(request)
    }

    pub async fn shutdown(self) {
        self.inner.shutdown().await
    }

    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    pub fn is_same(&self, other: &Self) -> bool {
        self.inner.is_same(&other.inner)
    }
}

impl<A: Actor> Clone for Address<A> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<A: Actor> PartialEq for Address<A> {
    fn eq(&self, other: &Self) -> bool {
        self.is_same(other)
    }
}

impl<A: Actor> Eq for Address<A> {}

impl<A: Actor> From<Sender<Message<A::Request>>> for Address<A> {
    fn from(sender: Sender<Message<A::Request>>) -> Self {
        Self {
            inner: AnyAddress::from(sender),
        }
    }
}

#[derive(Debug)]
pub struct AnyAddress<R> {
    sender: Sender<Message<R>>,
}

impl<R> AnyAddress<R> {
    pub fn mock() -> (Self, Receiver<Message<R>>) {
        let (sender, receiver) = mpsc::channel(64);
        (sender.into(), receiver)
    }

    pub async fn tell(&self, request: R) -> Result<(), TellError<R>> {
        match self.sender.send(Message::Main(request)).await {
            Ok(()) => Ok(()),
            Err(SendError(Message::Main(request))) => Err(TellError(request)),
            Err(_) => {
                unreachable!("`tell(..)` should not send messages other than `Message::Main(..)`")
            }
        }
    }

    pub fn try_tell(&self, request: R) -> Result<(), TryTellError<R>> {
        match self.sender.try_send(Message::Main(request)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(Message::Main(request))) => Err(TryTellError::Full(request)),
            Err(TrySendError::Closed(Message::Main(request))) => Err(TryTellError::Closed(request)),
            Err(_) => unreachable!(
                "`try_tell(..)` should not send messages other than `Message::Main(..)`"
            ),
        }
    }

    pub async fn shutdown(self) {
        let _ = self.sender.send(Message::Shutdown).await;
    }

    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    pub fn is_same(&self, other: &Self) -> bool {
        self.sender.same_channel(&other.sender)
    }
}

impl<R> Clone for AnyAddress<R> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<R> PartialEq for AnyAddress<R> {
    fn eq(&self, other: &Self) -> bool {
        self.is_same(other)
    }
}

impl<R> Eq for AnyAddress<R> {}

impl<R> From<Sender<Message<R>>> for AnyAddress<R> {
    fn from(sender: Sender<Message<R>>) -> Self {
        Self { sender }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TellError<T>(pub T);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TryTellError<T> {
    Full(T),
    Closed(T),
}
