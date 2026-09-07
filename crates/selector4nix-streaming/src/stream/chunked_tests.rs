use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use anyhow::{Error as AnyhowError, Result as AnyhowResult};
use bytes::Bytes;
use futures::{Stream, StreamExt};

use crate::stream::{ChunkConnector, ChunkTrottler, ChunkedStream, ChunkedStreamArgs};
use crate::throttler::{PerHostHttpThrottler, ThrottlerAdapter, ThrottlingOptions};
use crate::{SBoxFuture, SBoxStream};

const PIECE_LEN: usize = 8;

fn make_bytes(len: usize) -> Bytes {
    Bytes::from((0..len).map(|i| (i % 251) as u8).collect::<Vec<u8>>())
}

// Injected failures per chunk index: `u32::MAX` means "always fail". Stream failures are
// relative to each individual (possibly resumed) request, and clamped so that the failure
// still fires before the resumed slice is fully delivered.
#[derive(Clone, Default)]
struct FailSchedule {
    connect_fails_left: Arc<Mutex<HashMap<usize, u32>>>,
    stream_fails_left: Arc<Mutex<HashMap<usize, (u32, usize)>>>,
}

impl FailSchedule {
    fn connect_fail(idx: usize, times: u32) -> Self {
        Self::default().add_connect_fail(idx, times)
    }

    fn stream_fail(idx: usize, times: u32, fail_after: usize) -> Self {
        Self::default().add_stream_fail(idx, times, fail_after)
    }

    fn add_connect_fail(self, idx: usize, times: u32) -> Self {
        self.connect_fails_left.lock().unwrap().insert(idx, times);
        self
    }

    fn add_stream_fail(self, idx: usize, times: u32, fail_after: usize) -> Self {
        self.stream_fails_left
            .lock()
            .unwrap()
            .insert(idx, (times, fail_after));
        self
    }

    fn connect_should_fail(&self, idx: usize) -> bool {
        let mut map = self.connect_fails_left.lock().unwrap();
        match map.get_mut(&idx) {
            Some(n) if *n > 0 => {
                if *n != u32::MAX {
                    *n -= 1;
                }
                true
            }
            _ => false,
        }
    }

    fn stream_fail_at(&self, idx: usize, len: usize) -> Option<usize> {
        let mut map = self.stream_fails_left.lock().unwrap();
        match map.get_mut(&idx) {
            Some((n, fail_after)) if *n > 0 => {
                if *n != u32::MAX {
                    *n -= 1;
                }
                Some((*fail_after).min(len.saturating_sub(PIECE_LEN)))
            }
            _ => None,
        }
    }
}

#[derive(Clone)]
struct MockConnector {
    data: Bytes,
    chunk_max_len: usize,
    schedule: FailSchedule,
}

impl ChunkConnector for MockConnector {
    fn get(
        &self,
        offset: usize,
        len: usize,
    ) -> SBoxFuture<AnyhowResult<SBoxStream<AnyhowResult<Bytes>>>> {
        let idx = offset / self.chunk_max_len;
        let slice = self.data.slice(offset..offset + len);
        let schedule = self.schedule.clone();
        Box::pin(async move {
            if schedule.connect_should_fail(idx) {
                return Err(anyhow::anyhow!("connect error at chunk {idx}"));
            }
            let stream: SBoxStream<AnyhowResult<Bytes>> =
                Box::pin(MockStream::new(slice, schedule.stream_fail_at(idx, len)));
            Ok(stream)
        })
    }
}

struct MockStream {
    data: Bytes,
    pos: usize,
    yielded_this_poll: bool,
    fail_at: Option<usize>,
}

impl MockStream {
    fn new(data: Bytes, fail_at: Option<usize>) -> Self {
        Self {
            data,
            pos: 0,
            yielded_this_poll: false,
            fail_at,
        }
    }
}

impl Stream for MockStream {
    type Item = AnyhowResult<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(fail_at) = this.fail_at
            && this.pos >= fail_at
        {
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
    schedule: FailSchedule,
    retry_attempts: usize,
) -> (ChunkedStream, Bytes) {
    let data = make_bytes(data_len);

    let connector = Box::new(MockConnector {
        data: data.clone(),
        chunk_max_len,
        schedule: schedule.clone(),
    });

    let throttler = Box::new(ThrottlerAdapter::new(
        Arc::new(PerHostHttpThrottler::new(ThrottlingOptions::new(
            NonZeroUsize::new(max_concurrent_requests).unwrap(),
        ))),
        "example.com".to_string(),
    ));
    let initial_permit = throttler
        .try_acquire()
        .expect("a permit must be available at startup");

    let chunk0_len = chunk_max_len.min(data.len());
    let initial_fail_at = schedule.stream_fail_at(0, chunk0_len);
    let initial_chunk_stream: SBoxStream<AnyhowResult<Bytes>> =
        Box::pin(MockStream::new(data.slice(0..chunk0_len), initial_fail_at));

    let args = ChunkedStreamArgs {
        chunk_max_len: NonZeroUsize::new(chunk_max_len).unwrap(),
        bytes_total: data.len(),
        window_max_len: NonZeroUsize::new(window_max_len).unwrap(),
        max_retries: retry_attempts,
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
    let (mut stream, data) = make_stream(100, 256, 4, 8, FailSchedule::default(), 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn multi_chunk_reassembles_in_order() {
    let (mut stream, data) = make_stream(100000, 100, 4, 8, FailSchedule::default(), 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn last_chunk_is_partial() {
    let (mut stream, data) = make_stream(997, 100, 4, 8, FailSchedule::default(), 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out.len(), 997);
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn window_of_one() {
    let (mut stream, data) = make_stream(500, 50, 1, 8, FailSchedule::default(), 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn single_permit() {
    let (mut stream, data) = make_stream(500, 50, 8, 1, FailSchedule::default(), 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn connect_error_propagates_and_terminates() {
    let (mut stream, data) = make_stream(500, 50, 8, 8, FailSchedule::connect_fail(2, u32::MAX), 0);

    let (out, err) = collect_result(&mut stream).await;
    assert!(err.is_some());
    assert!(data.starts_with(&out));
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn stream_error_propagates_partial_then_terminates() {
    let (mut stream, data) =
        make_stream(500, 50, 8, 8, FailSchedule::stream_fail(3, u32::MAX, 1), 0);

    let (out, err) = collect_result(&mut stream).await;
    assert!(err.is_some());
    assert!(data.starts_with(&out));
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn connect_error_retries_then_succeeds() {
    let (mut stream, data) = make_stream(500, 50, 8, 8, FailSchedule::connect_fail(2, 2), 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn connect_error_exhausts_retries_then_terminates() {
    let (mut stream, data) = make_stream(500, 50, 8, 8, FailSchedule::connect_fail(2, u32::MAX), 2);

    let (out, err) = collect_result(&mut stream).await;
    assert!(err.is_some());
    assert!(data.starts_with(&out));
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn stream_error_midway_retries_then_succeeds() {
    let (mut stream, data) = make_stream(500, 50, 8, 8, FailSchedule::stream_fail(3, 1, 30), 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn stream_error_exhausts_retries_then_terminates() {
    let (mut stream, data) =
        make_stream(500, 50, 8, 8, FailSchedule::stream_fail(3, u32::MAX, 30), 2);

    let (out, err) = collect_result(&mut stream).await;
    assert!(err.is_some());
    assert!(data.starts_with(&out));
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn initial_chunk_stream_error_retries_then_succeeds() {
    let (mut stream, data) = make_stream(500, 50, 8, 8, FailSchedule::stream_fail(0, 1, 10), 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn connect_failure_then_stream_failure_still_succeeds() {
    let schedule = FailSchedule::default()
        .add_stream_fail(3, 1, 30)
        .add_connect_fail(3, 1);

    let (mut stream, data) = make_stream(500, 50, 8, 8, schedule, 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

#[tokio::test]
async fn serial_window_retries_each_chunk_independently() {
    let schedule = FailSchedule::default()
        .add_connect_fail(1, 1)
        .add_stream_fail(2, 1, 40)
        .add_connect_fail(3, 1)
        .add_stream_fail(3, 1, 10);

    let (mut stream, data) = make_stream(500, 50, 1, 8, schedule, 3);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out, data.to_vec());
}

// Regression test: under a saturated throttler, a stream whose window momentarily holds only
// finished chunks must NOT release its last permit — the FIFO semaphore would hand it to the
// competing stream and terminate this one early via `Ready(None)`.
#[tokio::test(flavor = "current_thread")]
async fn last_permit_retained_while_other_streams_wait() {
    use crate::throttler::{PerHostHttpThrottler, ThrottlingOptions};

    const DATA_LEN: usize = 500;
    const CHUNK_LEN: usize = 50;

    let data = make_bytes(DATA_LEN);
    let shared_throttler = Arc::new(PerHostHttpThrottler::new(ThrottlingOptions::new(
        NonZeroUsize::new(1).unwrap(),
    )));

    let adapter = Box::new(ThrottlerAdapter::new(
        Arc::clone(&shared_throttler),
        "example.com".to_string(),
    ));
    let initial_permit = adapter
        .try_acquire()
        .expect("a permit must be available at startup");

    let contender = {
        let throttler = Arc::clone(&shared_throttler);
        tokio::spawn(async move {
            let _permit = throttler.acquire("example.com").await;
            futures::future::pending::<()>().await;
        })
    };
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;

    let args = ChunkedStreamArgs {
        chunk_max_len: NonZeroUsize::new(CHUNK_LEN).unwrap(),
        bytes_total: data.len(),
        window_max_len: NonZeroUsize::new(8).unwrap(),
        max_retries: 0,
        connector: Box::new(MockConnector {
            data: data.clone(),
            chunk_max_len: CHUNK_LEN,
            schedule: FailSchedule::default(),
        }),
        throttler: adapter,
        initial_permit,
        initial_chunk_stream: Box::pin(MockStream::new(data.slice(0..CHUNK_LEN), None)),
    };
    let mut stream = ChunkedStream::new(args);

    let out = collect_ok(&mut stream).await;
    assert_eq!(out.len(), DATA_LEN);
    assert_eq!(out, data.to_vec());
    contender.abort();
}
