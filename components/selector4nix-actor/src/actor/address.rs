use tokio::sync::mpsc::error::{SendError, TrySendError};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::actor::{Actor, Message};

#[derive(Debug)]
pub struct Address<A: Actor> {
    sender: Sender<Message<A::Request>>,
}

impl<A: Actor> Address<A> {
    pub fn mock() -> (Self, Receiver<Message<A::Request>>) {
        let (sender, receiver) = mpsc::channel(64);
        (sender.into(), receiver)
    }

    pub async fn tell(&self, request: A::Request) -> Result<(), TellError<A::Request>> {
        match self.sender.send(Message::Main(request)).await {
            Ok(()) => Ok(()),
            Err(SendError(Message::Main(request))) => Err(TellError(request)),
            Err(_) => unreachable!(
                "`Address::tell(..)` should not send messages other than `Message::Main(..)`"
            ),
        }
    }

    pub fn try_tell(&self, request: A::Request) -> Result<(), TryTellError<A::Request>> {
        match self.sender.try_send(Message::Main(request)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(Message::Main(request))) => Err(TryTellError::Full(request)),
            Err(TrySendError::Closed(Message::Main(request))) => Err(TryTellError::Closed(request)),
            Err(_) => unreachable!(
                "`Address::try_tell(..)` should not send messages other than `Message::Main(..)`"
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

impl<A: Actor> Clone for Address<A> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
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
