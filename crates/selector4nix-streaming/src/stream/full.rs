use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Stream, StreamExt};

use crate::throttler::ThrottlerPermit;

pub struct FullStream<S> {
    inner: S,
    _permit: ThrottlerPermit,
}

impl<S> FullStream<S> {
    pub fn new(inner: S, permit: ThrottlerPermit) -> Self {
        Self {
            inner,
            _permit: permit,
        }
    }
}

impl<S> Stream for FullStream<S>
where
    S: Stream + Unpin,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().inner.poll_next_unpin(cx)
    }
}
