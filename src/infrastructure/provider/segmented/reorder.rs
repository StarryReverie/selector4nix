use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bytes::Bytes;
use tokio::sync::{Mutex, Notify, Semaphore};

pub struct ReorderBuffer {
    total_size: u64,
    cursor: AtomicU64,
    pending: Mutex<BTreeMap<u64, PendingChunk>>,
    byte_budget: Arc<Semaphore>,
    notify: Notify,
    complete: AtomicBool,
    error: Mutex<Option<String>>,
}

struct PendingChunk {
    bytes: Bytes,
    permit_count: u32,
}

impl ReorderBuffer {
    pub fn new(total_size: u64, buffer_bytes: usize) -> Arc<Self> {
        Arc::new(Self {
            total_size,
            cursor: AtomicU64::new(0),
            pending: Mutex::new(BTreeMap::new()),
            byte_budget: Arc::new(Semaphore::new(buffer_bytes.max(1))),
            notify: Notify::new(),
            complete: AtomicBool::new(false),
            error: Mutex::new(None),
        })
    }

    pub fn cursor(&self) -> u64 {
        self.cursor.load(Ordering::Relaxed)
    }

    pub fn mark_complete(&self) {
        self.complete.store(true, Ordering::Relaxed);
        self.notify.notify_waiters();
    }

    pub async fn fail(&self, err: anyhow::Error) {
        *self.error.lock().await = Some(err.to_string());
        self.complete.store(true, Ordering::Relaxed);
        self.notify.notify_waiters();
    }

    pub async fn push(&self, offset: u64, data: Bytes) -> anyhow::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let permit_count = data.len() as u32;
        self.byte_budget
            .acquire_many(permit_count)
            .await
            .map_err(|_| anyhow::anyhow!("byte budget semaphore closed"))?
            .forget();

        self.pending.lock().await.insert(
            offset,
            PendingChunk {
                bytes: data,
                permit_count,
            },
        );
        self.notify.notify_waiters();
        Ok(())
    }

    pub async fn pop_next(&self) -> Option<anyhow::Result<Bytes>> {
        loop {
            if let Some(err) = self.error.lock().await.clone() {
                return Some(Err(anyhow::anyhow!(err)));
            }

            let cursor = self.cursor.load(Ordering::Relaxed);
            if cursor >= self.total_size {
                return None;
            }

            if let Some(chunk) = self.pending.lock().await.remove(&cursor) {
                let next = cursor + chunk.bytes.len() as u64;
                self.cursor.store(next, Ordering::Relaxed);
                self.byte_budget.add_permits(chunk.permit_count as usize);
                return Some(Ok(chunk.bytes));
            }

            if self.complete.load(Ordering::Relaxed) {
                return Some(Err(anyhow::anyhow!(
                    "segmented download ended before all bytes were received"
                )));
            }

            self.notify.notified().await;
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
}
