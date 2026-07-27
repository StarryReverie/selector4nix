use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{Error as AnyhowError, Result as AnyhowResult};
use bytes::Bytes;
use futures::{Stream, StreamExt};

use crate::stream::{ChunkConnector, ChunkTrottler, ChunkedStream, ChunkedStreamArgs};
use crate::throttler::{PerHostHttpThrottler, ThrottlerAdapter};
use crate::{SBoxFuture, SBoxStream};

const PIECE_LEN: usize = 8;

fn make_bytes(len: usize) -> Bytes {
    Bytes::from((0..len).map(|i| (i % 251) as u8).collect::<Vec<u8>>())
}

#[derive(Clone)]
struct MockConnector {
    data: Bytes,
    chunk_max_len: usize,
    fail_connect_at: Option<usize>,
    fail_stream_at: Option<usize>,
}

impl ChunkConnector for MockConnector {
    fn get(
        &self,
        offset: usize,
        len: usize,
    ) -> SBoxFuture<AnyhowResult<SBoxStream<AnyhowResult<Bytes>>>> {
        let idx = offset / self.chunk_max_len;

        if self.fail_connect_at == Some(idx) {
            return Box::pin(async move { Err(anyhow::anyhow!("connect error at chunk {idx}")) });
        }

        let slice = self.data.slice(offset..offset + len);
        let fail = self.fail_stream_at == Some(idx);
        Box::pin(async move {
            let stream: SBoxStream<AnyhowResult<Bytes>> = Box::pin(MockStream::new(slice, fail));
            Ok(stream)
        })
    }
}

struct MockStream {
    data: Bytes,
    pos: usize,
    yielded_this_poll: bool,
    fail: bool,
}

impl MockStream {
    fn new(data: Bytes, fail: bool) -> Self {
        Self {
            data,
            pos: 0,
            yielded_this_poll: false,
            fail,
        }
    }
}

impl Stream for MockStream {
    type Item = AnyhowResult<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.fail && this.pos > 0 {
            return Poll::Ready(Some(Err(anyhow::anyhow!(
                "stream error after partial data"
            ))));
        }
        if this.pos >= this.data.len() {
            return Poll::Ready(None);
        }
        if this.yielded_this_poll {
            this.yielded_this_poll = false;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }

        let end = (this.pos + PIECE_LEN).min(this.data.len());
        let piece = this.data.slice(this.pos..end);
        this.pos = end;
        this.yielded_this_poll = true;
        Poll::Ready(Some(Ok(piece)))
    }
}

fn make_stream(
    data_len: usize,
    chunk_max_len: usize,
    window_max_len: usize,
    max_concurrent_requests: usize,
    fail_connect_at: Option<usize>,
    fail_stream_at: Option<usize>,
) -> (ChunkedStream, Bytes) {
    let data = make_bytes(data_len);

    let connector = Box::new(MockConnector {
        data: data.clone(),
        chunk_max_len,
        fail_connect_at,
        fail_stream_at,
    });

    let throttler = Box::new(ThrottlerAdapter::new(
        Arc::new(PerHostHttpThrottler::new(max_concurrent_requests)),
        "example.com".to_string(),
    ));
    let initial_permit = throttler
        .try_acquire()
        .expect("a permit must be available at startup");

    let chunk0_len = chunk_max_len.min(data.len());
    let initial_chunk_stream: SBoxStream<AnyhowResult<Bytes>> = Box::pin(MockStream::new(
        data.slice(0..chunk0_len),
        fail_stream_at == Some(0),
    ));

    let args = ChunkedStreamArgs {
        chunk_max_len: NonZeroUsize::new(chunk_max_len).unwrap(),
        bytes_total: data.len(),
        window_max_len: NonZeroUsize::new(window_max_len).unwrap(),
        connector,
        throttler,
        initial_permit,
        initial_chunk_stream,
    };

    (ChunkedStream::new(args), data)
}

async fn collect_ok(stream: &mut ChunkedStream) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(bytes) => out.extend_from_slice(&bytes),
            Err(err) => panic!("unexpected error: {err}"),
        }
    }
    out
}

async fn collect_result(stream: &mut ChunkedStream) -> (Vec<u8>, Option<AnyhowError>) {
    let mut out = Vec::new();
    let mut err = None;
    while let Some(item) = stream.next().await {
        match item {
            Ok(bytes) => out.extend_from_slice(&bytes),
            Err(e) => {
                err = Some(e);
                break;
            }
        }
    }
    (out, err)
}

#[tokio::test]
async fn single_chunk_when_chunk_larger_than_total() {
    let (mut stream, data) = make_stream(100, 256, 4, 8, None, None);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn multi_chunk_reassembles_in_order() {
    let (mut stream, data) = make_stream(100000, 100, 4, 8, None, None);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn last_chunk_is_partial() {
    let (mut stream, data) = make_stream(997, 100, 4, 8, None, None);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out.len(), 997);
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn window_of_one() {
    let (mut stream, data) = make_stream(500, 50, 1, 8, None, None);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn single_permit() {
    let (mut stream, data) = make_stream(500, 50, 8, 1, None, None);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn connect_error_propagates_and_terminates() {
    let (mut stream, data) = make_stream(500, 50, 8, 8, Some(2), None);

    let (out, err) = collect_result(&mut stream).await;
    assert!(err.is_some());
    assert!(data.starts_with(&out));
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn stream_error_propagates_partial_then_terminates() {
    let (mut stream, data) = make_stream(500, 50, 8, 8, None, Some(3));

    let (out, err) = collect_result(&mut stream).await;
    assert!(err.is_some());
    assert!(data.starts_with(&out));
    assert!(stream.next().await.is_none());
}
