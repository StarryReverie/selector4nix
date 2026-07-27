use tokio::sync::mpsc::error::{SendError, TrySendError};
use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};
use tokio::sync::oneshot::{self, Sender as OneshotSender};
use tokio::sync::watch::{self, Receiver as WatchReceiver, Sender as WatchSender};

use crate::actor::Actor;

#[derive(Debug)]
pub struct Address<A: Actor> {
    inner: AnyAddress<A::Request>,
}

impl<A: Actor> Address<A> {
    pub fn mock() -> (Self, MpscReceiver<A::Request>, WatchSender<bool>) {
        let (inner, receiver, terminated_tx) = AnyAddress::mock();
        (inner.into(), receiver, terminated_tx)
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

    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    pub fn closed_listener(&self) -> WatchReceiver<bool> {
        self.inner.closed_listener()
    }

    pub async fn wait_closed(self) {
        self.inner.wait_closed().await;
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

impl<A: Actor> From<AnyAddress<A::Request>> for Address<A> {
    fn from(inner: AnyAddress<A::Request>) -> Self {
        Self { inner }
    }
}

#[derive(Debug)]
pub struct AnyAddress<R> {
    sender: MpscSender<R>,
    terminated: WatchReceiver<bool>,
}

impl<R> AnyAddress<R> {
    pub fn new(sender: MpscSender<R>, terminated: WatchReceiver<bool>) -> Self {
        Self { sender, terminated }
    }

    pub fn mock() -> (Self, MpscReceiver<R>, WatchSender<bool>) {
        let (sender, receiver) = mpsc::channel(64);
        let (terminated_tx, terminated) = watch::channel(false);
        (AnyAddress::new(sender, terminated), receiver, terminated_tx)
    }

    pub async fn tell(&self, request: R) -> Result<(), TellError<R>> {
        match self.sender.send(request).await {
            Ok(()) => Ok(()),
            Err(SendError(request)) => Err(TellError(request)),
        }
    }

    pub fn try_tell(&self, request: R) -> Result<(), TryTellError<R>> {
        match self.sender.try_send(request) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(request)) => Err(TryTellError::Full(request)),
            Err(TrySendError::Closed(request)) => Err(TryTellError::Closed(request)),
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

    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    pub fn closed_listener(&self) -> WatchReceiver<bool> {
        self.terminated.clone()
    }

    pub async fn wait_closed(mut self) {
        drop(self.sender);
        loop {
            if *self.terminated.borrow_and_update() {
                break;
            }
            if self.terminated.changed().await.is_err() {
                break;
            }
        }
    }

    pub fn is_same(&self, other: &Self) -> bool {
        self.sender.same_channel(&other.sender)
    }
}

impl<R> Clone for AnyAddress<R> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            terminated: self.terminated.clone(),
        }
    }
}

impl<R> PartialEq for AnyAddress<R> {
    fn eq(&self, other: &Self) -> bool {
        self.is_same(other)
    }
}

impl<R> Eq for AnyAddress<R> {}

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
