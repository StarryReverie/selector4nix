use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::{Context as _, Result as AnyhowResult};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use http::{StatusCode, header};
use reqwest::{Client, Response};
use tokio::sync::mpsc;
use tokio::time;

use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::common::url::Url;
use crate::domain::nar_file::port::{NarStreamData, NarStreamHeaders};
use crate::infrastructure::config::{AppCredential, DownloadConfiguration};
use crate::infrastructure::util::{
    DownloadLoadTracker, LoadGuard, PerHostHttpThrottler, ThrottlerPermit,
};

use super::reorder::ReorderBuffer;

struct SegmentHandle {
    start: u64,
    end: u64,
    cancel: Arc<AtomicBool>,
}

struct CoordinatorContext {
    reorder: Arc<ReorderBuffer>,
    total_size: u64,
    config: DownloadConfiguration,
    load_tracker: DownloadLoadTracker,
    active_workers: AtomicUsize,
    segments: std::sync::Mutex<Vec<SegmentHandle>>,
    workers_done: AtomicUsize,
    expected_workers: AtomicUsize,
}

pub fn start_segmented_download(
    client: Client,
    throttler: Arc<PerHostHttpThrottler>,
    credentials: Arc<AppCredential>,
    config: DownloadConfiguration,
    load_tracker: DownloadLoadTracker,
    load_guard: LoadGuard,
    initial_permit: ThrottlerPermit,
    url: Url,
    headers: PassthroughHeaders,
    total_size: u64,
    stream_headers: NarStreamHeaders,
    initial_response: Response,
) -> AnyhowResult<NarStreamData> {
    let reorder = ReorderBuffer::new(total_size, config.segmented_buffer_bytes);
    let ctx = Arc::new(CoordinatorContext {
        reorder: Arc::clone(&reorder),
        total_size,
        config: config.clone(),
        load_tracker,
        active_workers: AtomicUsize::new(0),
        segments: std::sync::Mutex::new(Vec::new()),
        workers_done: AtomicUsize::new(0),
        expected_workers: AtomicUsize::new(0),
    });

    let initial_end = (total_size / config.segmented_max_connections as u64).max(1);
    spawn_initial_worker(
        Arc::clone(&ctx),
        initial_response,
        0,
        initial_end.min(total_size),
        initial_permit,
    );

    if initial_end < total_size {
        spawn_range_segments(
            Arc::clone(&ctx),
            client.clone(),
            throttler.clone(),
            credentials.clone(),
            url.clone(),
            headers.clone(),
            initial_end,
            total_size,
            config.segmented_max_connections.saturating_sub(1),
        );
    }

    spawn_supervisor(
        Arc::clone(&ctx),
        client,
        throttler,
        credentials,
        url.clone(),
        headers,
    );

    let stream = SegmentedStream::new(reorder, load_guard);
    Ok(NarStreamData::new(stream_headers, Box::pin(stream), url))
}

fn spawn_initial_worker(
    ctx: Arc<CoordinatorContext>,
    response: Response,
    start: u64,
    end: u64,
    _permit: ThrottlerPermit,
) {
    let cancel = Arc::new(AtomicBool::new(false));
    ctx.segments
        .lock()
        .expect("segment mutex poisoned")
        .push(SegmentHandle {
            start,
            end,
            cancel: Arc::clone(&cancel),
        });
    ctx.expected_workers.fetch_add(1, Ordering::Relaxed);

    let reorder = Arc::clone(&ctx.reorder);
    let ctx = Arc::clone(&ctx);
    tokio::spawn(async move {
        ctx.active_workers.fetch_add(1, Ordering::Relaxed);
        let result = read_response_segment(response, reorder, start, end, cancel).await;
        ctx.active_workers.fetch_sub(1, Ordering::Relaxed);
        on_worker_finished(&ctx, result).await;
    });
}

fn spawn_range_segments(
    ctx: Arc<CoordinatorContext>,
    client: Client,
    throttler: Arc<PerHostHttpThrottler>,
    credentials: Arc<AppCredential>,
    url: Url,
    headers: PassthroughHeaders,
    start: u64,
    end: u64,
    max_segments: usize,
) {
    if start >= end || max_segments == 0 {
        return;
    }

    let count = max_segments.min(((end - start) as usize).max(1));
    let chunk_size = ((end - start) + count as u64 - 1) / count as u64;

    let mut offset = start;
    while offset < end {
        let segment_end = (offset + chunk_size).min(end);
        spawn_range_worker(
            Arc::clone(&ctx),
            client.clone(),
            throttler.clone(),
            credentials.clone(),
            url.clone(),
            headers.clone(),
            offset,
            segment_end,
        );
        offset = segment_end;
    }
}

fn spawn_range_worker(
    ctx: Arc<CoordinatorContext>,
    client: Client,
    throttler: Arc<PerHostHttpThrottler>,
    credentials: Arc<AppCredential>,
    url: Url,
    headers: PassthroughHeaders,
    start: u64,
    end: u64,
) {
    let cancel = Arc::new(AtomicBool::new(false));
    ctx.segments
        .lock()
        .expect("segment mutex poisoned")
        .push(SegmentHandle {
            start,
            end,
            cancel: Arc::clone(&cancel),
        });
    ctx.expected_workers.fetch_add(1, Ordering::Relaxed);

    let reorder = Arc::clone(&ctx.reorder);
    let host = url.host().to_string();
    let ctx = Arc::clone(&ctx);
    tokio::spawn(async move {
        ctx.active_workers.fetch_add(1, Ordering::Relaxed);
        let permit = throttler.acquire(&host).await;
        let result = read_range_segment(
            client,
            credentials,
            url,
            headers,
            reorder,
            start,
            end,
            cancel,
        )
        .await;
        drop(permit);
        ctx.active_workers.fetch_sub(1, Ordering::Relaxed);
        on_worker_finished(&ctx, result).await;
    });
}

fn spawn_supervisor(
    ctx: Arc<CoordinatorContext>,
    client: Client,
    throttler: Arc<PerHostHttpThrottler>,
    credentials: Arc<AppCredential>,
    url: Url,
    headers: PassthroughHeaders,
) {
    let min_tail_bytes = ctx
        .config
        .segmented_min_file_bytes
        .saturating_div(ctx.config.segmented_max_connections as u64)
        .max(1);

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;

            if ctx.reorder.cursor() >= ctx.total_size {
                break;
            }

            if ctx.load_tracker.current() > ctx.config.segmented_load_threshold {
                continue;
            }

            let delivered = ctx.reorder.cursor();
            let tail = ctx.total_size.saturating_sub(delivered);
            if tail <= min_tail_bytes {
                continue;
            }

            if ctx.active_workers.load(Ordering::Relaxed) >= ctx.config.segmented_max_connections {
                continue;
            }

            let split_plan = {
                let segments = ctx.segments.lock().expect("segment mutex poisoned");
                find_splittable(&segments, delivered, min_tail_bytes)
            };

            let Some((index, split_at, old_end)) = split_plan else {
                continue;
            };

            {
                let mut segments = ctx.segments.lock().expect("segment mutex poisoned");
                segments[index].cancel.store(true, Ordering::Relaxed);
                segments.remove(index);
            }

            spawn_range_segments(
                Arc::clone(&ctx),
                client.clone(),
                throttler.clone(),
                credentials.clone(),
                url.clone(),
                headers.clone(),
                split_at,
                old_end,
                ctx.config
                    .segmented_max_connections
                    .saturating_sub(ctx.active_workers.load(Ordering::Relaxed)),
            );

            tracing::debug!(
                split_at,
                tail = ctx.total_size.saturating_sub(split_at),
                "spawned additional segmented download workers"
            );
        }
    });
}

fn find_splittable(
    segments: &[SegmentHandle],
    delivered: u64,
    min_tail_bytes: u64,
) -> Option<(usize, u64, u64)> {
    segments
        .iter()
        .enumerate()
        .filter(|(_, segment)| {
            let progress = delivered.max(segment.start);
            segment.end > progress && segment.end.saturating_sub(progress) > min_tail_bytes
        })
        .max_by_key(|(_, segment)| segment.end.saturating_sub(delivered.max(segment.start)))
        .map(|(index, segment)| (index, delivered.max(segment.start), segment.end))
}

async fn on_worker_finished(ctx: &CoordinatorContext, result: AnyhowResult<()>) {
    if let Err(err) = result {
        ctx.reorder.fail(err).await;
    }

    let done = ctx.workers_done.fetch_add(1, Ordering::Relaxed) + 1;
    let expected = ctx.expected_workers.load(Ordering::Relaxed);
    if done >= expected && ctx.active_workers.load(Ordering::Relaxed) == 0 {
        ctx.reorder.mark_complete();
    }
}

async fn read_response_segment(
    response: Response,
    reorder: Arc<ReorderBuffer>,
    start: u64,
    end: u64,
    cancel: Arc<AtomicBool>,
) -> AnyhowResult<()> {
    let mut offset = start;
    let mut stream = response.bytes_stream();

    while offset < end && !cancel.load(Ordering::Relaxed) {
        let chunk = match stream.next().await {
            Some(Ok(chunk)) => chunk,
            Some(Err(err)) => {
                return Err(err).context("failed to read initial segmented nar stream");
            }
            None => break,
        };

        let remaining = end - offset;
        if chunk.len() as u64 > remaining {
            reorder
                .push(offset, chunk.slice(..remaining as usize))
                .await?;
            break;
        }

        let chunk_len = chunk.len() as u64;
        reorder.push(offset, chunk).await?;
        offset += chunk_len;
    }

    Ok(())
}

async fn read_range_segment(
    client: Client,
    credentials: Arc<AppCredential>,
    url: Url,
    headers: PassthroughHeaders,
    reorder: Arc<ReorderBuffer>,
    start: u64,
    end: u64,
    cancel: Arc<AtomicBool>,
) -> AnyhowResult<()> {
    let range_header = format!("bytes={start}-{}", end.saturating_sub(1));

    let mut request = client
        .get(url.value())
        .headers(headers.to_headers())
        .header(header::RANGE, range_header);

    if let Some(credential) = credentials.lookup(&url) {
        request = request.basic_auth(credential.login.clone(), credential.secret.clone());
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("failed to request nar range from {url}"))?;

    if response.status() != StatusCode::PARTIAL_CONTENT {
        return Err(anyhow::anyhow!(
            "substituter returned {} instead of 206 Partial Content for range request",
            response.status()
        ));
    }

    let mut offset = start;
    let mut stream = response.bytes_stream();

    while offset < end && !cancel.load(Ordering::Relaxed) {
        let chunk = match stream.next().await {
            Some(Ok(chunk)) => chunk,
            Some(Err(err)) => {
                return Err(err).context("failed to read ranged segmented nar stream");
            }
            None => break,
        };

        let remaining = end - offset;
        if chunk.len() as u64 > remaining {
            reorder
                .push(offset, chunk.slice(..remaining as usize))
                .await?;
            break;
        }

        let chunk_len = chunk.len() as u64;
        reorder.push(offset, chunk).await?;
        offset += chunk_len;
    }

    if offset < end && !cancel.load(Ordering::Relaxed) {
        return Err(anyhow::anyhow!(
            "range worker ended before segment was fully received"
        ));
    }

    Ok(())
}

struct SegmentedStream {
    receiver: mpsc::Receiver<anyhow::Result<Bytes>>,
    _load_guard: LoadGuard,
}

impl SegmentedStream {
    fn new(reorder: Arc<ReorderBuffer>, load_guard: LoadGuard) -> Self {
        let (sender, receiver) = mpsc::channel(1);
        tokio::spawn(async move {
            while let Some(item) = reorder.pop_next().await {
                if sender.send(item).await.is_err() {
                    break;
                }
            }
        });

        Self {
            receiver,
            _load_guard: load_guard,
        }
    }
}

impl Stream for SegmentedStream {
    type Item = AnyhowResult<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}
