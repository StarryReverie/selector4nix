pub mod stream;
pub mod throttler;

mod client;

pub use client::{StreamHttpBodyError, StreamingClient, StreamingRequest, StreamingResponse};

use std::pin::Pin;

use futures::Stream;

type SBoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
type SBoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;
