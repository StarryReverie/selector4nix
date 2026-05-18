use tokio::sync::mpsc::error::{SendError, TrySendError};
use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};
use tokio::sync::oneshot::{self, Sender as OneshotSender};

use crate::actor::{Actor, Message};

#[derive(Debug)]
pub struct Address<A: Actor> {
    inner: AnyAddress<A::Request>,
}

impl<A: Actor> Address<A> {
    pub fn mock() -> (Self, MpscReceiver<Message<A::Request>>) {
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

    pub async fn ask<F, T>(&self, preparation: F) -> Result<T, AskError<A::Request>>
    where
        F: FnOnce(OneshotSender<T>) -> A::Request,
    {
        self.inner.ask(preparation).await
    }

    pub async fn try_ask<F, T>(&self, preparation: F) -> Result<T, TryAskError<A::Request>>
    where
        F: FnOnce(OneshotSender<T>) -> A::Request,
    {
        self.inner.try_ask(preparation).await
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

impl<A: Actor> From<MpscSender<Message<A::Request>>> for Address<A> {
    fn from(sender: MpscSender<Message<A::Request>>) -> Self {
        Self {
            inner: AnyAddress::from(sender),
        }
    }
}

#[derive(Debug)]
pub struct AnyAddress<R> {
    sender: MpscSender<Message<R>>,
}

impl<R> AnyAddress<R> {
    pub fn mock() -> (Self, MpscReceiver<Message<R>>) {
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

    pub async fn ask<F, T>(&self, preparation: F) -> Result<T, AskError<R>>
    where
        F: FnOnce(OneshotSender<T>) -> R,
    {
        let (reply_to, reply) = oneshot::channel::<T>();
        let request = preparation(reply_to);
        self.tell(request)
            .await
            .map_err(|err| AskError::Tell(err))?;
        reply.await.map_err(|_| AskError::Receive)
    }

    pub async fn try_ask<F, T>(&self, preparation: F) -> Result<T, TryAskError<R>>
    where
        F: FnOnce(OneshotSender<T>) -> R,
    {
        let (reply_to, reply) = oneshot::channel::<T>();
        let request = preparation(reply_to);
        self.try_tell(request)
            .map_err(|err| TryAskError::Tell(err))?;
        reply.await.map_err(|_| TryAskError::Receive)
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

impl<R> From<MpscSender<Message<R>>> for AnyAddress<R> {
    fn from(sender: MpscSender<Message<R>>) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AskError<T> {
    Tell(TellError<T>),
    Receive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TryAskError<T> {
    Tell(TryTellError<T>),
    Receive,
}
