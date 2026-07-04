use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bytes::Bytes;
use tokio::sync::{Mutex, Notify};

/// Reassembles out-of-order segment chunks into an in-order byte stream while
/// keeping memory bounded.
///
/// Admission is windowed: a chunk is accepted immediately when it lies within
/// `max_ahead` bytes of the output cursor (and always when it is at or before
/// the cursor), otherwise the producing worker waits until the cursor advances.
/// This guarantees the next-needed bytes can always be buffered, so the leading
/// segment keeps streaming to the client mid-download, while trailing segments
/// that race too far ahead are throttled to bound memory usage.
pub struct ReorderBuffer {
    total_size: u64,
    max_ahead: u64,
    cursor: AtomicU64,
    pending: Mutex<BTreeMap<u64, Bytes>>,
    data_notify: Notify,
    window_notify: Notify,
    complete: AtomicBool,
    error: Mutex<Option<String>>,
}

impl ReorderBuffer {
    pub fn new(total_size: u64, buffer_bytes: usize) -> Arc<Self> {
        Arc::new(Self {
            total_size,
            max_ahead: (buffer_bytes.max(1)) as u64,
            cursor: AtomicU64::new(0),
            pending: Mutex::new(BTreeMap::new()),
            data_notify: Notify::new(),
            window_notify: Notify::new(),
            complete: AtomicBool::new(false),
            error: Mutex::new(None),
        })
    }

    pub fn cursor(&self) -> u64 {
        self.cursor.load(Ordering::Relaxed)
    }

    pub fn mark_complete(&self) {
        self.complete.store(true, Ordering::Relaxed);
        self.data_notify.notify_waiters();
        self.window_notify.notify_waiters();
    }

    pub async fn fail(&self, err: anyhow::Error) {
        *self.error.lock().await = Some(err.to_string());
        self.complete.store(true, Ordering::Relaxed);
        self.data_notify.notify_waiters();
        self.window_notify.notify_waiters();
    }

    fn is_admitted(&self, offset: u64) -> bool {
        if self.complete.load(Ordering::Relaxed) {
            return true;
        }
        let cursor = self.cursor.load(Ordering::Relaxed);
        offset <= cursor || offset - cursor < self.max_ahead
    }

    pub async fn push(&self, offset: u64, data: Bytes) -> anyhow::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        while !self.is_admitted(offset) {
            let notified = self.window_notify.notified();
            tokio::pin!(notified);
            // Register interest before re-checking so a concurrent cursor
            // advance cannot be missed (lost-wakeup safe).
            notified.as_mut().enable();
            if self.is_admitted(offset) {
                break;
            }
            notified.await;
        }

        self.pending.lock().await.insert(offset, data);
        self.data_notify.notify_waiters();
        Ok(())
    }

    pub async fn pop_next(&self) -> Option<anyhow::Result<Bytes>> {
        loop {
            let notified = self.data_notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            if let Some(err) = self.error.lock().await.clone() {
                return Some(Err(anyhow::anyhow!(err)));
            }

            let cursor = self.cursor.load(Ordering::Relaxed);
            if cursor >= self.total_size {
                return None;
            }

            if let Some(bytes) = self.pending.lock().await.remove(&cursor) {
                let next = cursor + bytes.len() as u64;
                self.cursor.store(next, Ordering::Relaxed);
                self.window_notify.notify_waiters();
                return Some(Ok(bytes));
            }

            if self.complete.load(Ordering::Relaxed) {
                return Some(Err(anyhow::anyhow!(
                    "segmented download ended before all bytes were received"
                )));
            }

            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn emits_chunks_in_offset_order() {
        let buffer = ReorderBuffer::new(10, 1024);
        buffer.push(5, Bytes::from_static(b"56789")).await.unwrap();
        buffer.push(0, Bytes::from_static(b"01234")).await.unwrap();
        buffer.mark_complete();

        assert_eq!(
            buffer.pop_next().await.unwrap().unwrap(),
            Bytes::from_static(b"01234")
        );
        assert_eq!(
            buffer.pop_next().await.unwrap().unwrap(),
            Bytes::from_static(b"56789")
        );
        assert!(buffer.pop_next().await.is_none());
    }

    #[tokio::test]
    async fn reports_error_when_marked_complete_with_missing_bytes() {
        let buffer = ReorderBuffer::new(10, 1024);
        buffer.push(0, Bytes::from_static(b"01234")).await.unwrap();
        buffer.mark_complete();

        assert_eq!(
            buffer.pop_next().await.unwrap().unwrap(),
            Bytes::from_static(b"01234")
        );
        let err = buffer.pop_next().await.unwrap().unwrap_err();
        assert!(err.to_string().contains("ended before all bytes were received"));
    }

    #[tokio::test]
    async fn propagates_failures() {
        let buffer = ReorderBuffer::new(10, 1024);
        buffer.fail(anyhow::anyhow!("worker failed")).await;

        let err = buffer.pop_next().await.unwrap().unwrap_err();
        assert!(err.to_string().contains("worker failed"));
    }

    #[tokio::test]
    async fn streams_leading_bytes_before_trailing_chunk_arrives() {
        let buffer = ReorderBuffer::new(10, 1024);

        // Only the leading chunk is available; the tail has not been fetched.
        buffer.push(0, Bytes::from_static(b"01234")).await.unwrap();

        // The client receives the leading segment immediately, mid-download.
        assert_eq!(
            buffer.pop_next().await.unwrap().unwrap(),
            Bytes::from_static(b"01234")
        );

        // The trailing chunk arrives later and is then emitted in order.
        buffer.push(5, Bytes::from_static(b"56789")).await.unwrap();
        buffer.mark_complete();

        assert_eq!(
            buffer.pop_next().await.unwrap().unwrap(),
            Bytes::from_static(b"56789")
        );
        assert!(buffer.pop_next().await.is_none());
    }

    #[tokio::test]
    async fn leading_chunk_larger_than_window_is_admitted() {
        // Window is only 4 bytes, yet the in-order leading chunk must always be
        // admitted so it can stream to the client without blocking.
        let buffer = ReorderBuffer::new(10, 4);

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            buffer.push(0, Bytes::from_static(b"0123456789")),
        )
        .await
        .expect("leading chunk must not block on a small window")
        .unwrap();

        assert_eq!(
            buffer.pop_next().await.unwrap().unwrap(),
            Bytes::from_static(b"0123456789")
        );
        assert!(buffer.pop_next().await.is_none());
    }

    #[tokio::test]
    async fn far_ahead_chunk_is_admitted_after_cursor_advances() {
        // Window of 5 bytes. The trailing chunk at offset 5 starts outside the
        // window and must wait until the cursor advances past the leading chunk.
        let buffer = ReorderBuffer::new(10, 5);
        buffer.push(0, Bytes::from_static(b"01234")).await.unwrap();

        let writer = {
            let buffer = Arc::clone(&buffer);
            tokio::spawn(async move {
                buffer.push(5, Bytes::from_static(b"56789")).await.unwrap();
            })
        };

        // Draining the leading chunk advances the cursor to 5, opening the
        // window for the trailing chunk.
        assert_eq!(
            buffer.pop_next().await.unwrap().unwrap(),
            Bytes::from_static(b"01234")
        );

        tokio::time::timeout(std::time::Duration::from_secs(1), writer)
            .await
            .expect("trailing push should be admitted once the window opens")
            .unwrap();

        buffer.mark_complete();
        assert_eq!(
            buffer.pop_next().await.unwrap().unwrap(),
            Bytes::from_static(b"56789")
        );
        assert!(buffer.pop_next().await.is_none());
    }
}
