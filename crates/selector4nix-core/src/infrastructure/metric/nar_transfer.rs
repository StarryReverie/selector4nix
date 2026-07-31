use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;

use crate::domain::common::url::Url;
use crate::domain::nar_info::model::NarFileName;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NarTransferId(u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NarTransferMeta {
    pub nar_file_name: NarFileName,
    pub store_path: Option<String>,
    pub substituter_url: Url,
    pub source_url: Url,
    pub content_length: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NarTransferMetricEntry {
    pub meta: NarTransferMeta,
    pub bytes_transferred: u64,
    pub started_at_unix_ms: u64,
}

pub struct NarTransferMetric {
    next_id: AtomicU64,
    transferring: DashMap<NarTransferId, NarTransferMetricEntry>,
}

impl NarTransferMetric {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            transferring: DashMap::new(),
        }
    }

    pub fn begin(self: &Arc<Self>, meta: NarTransferMeta) -> NarTransferHandle {
        let id = NarTransferId(self.next_id.fetch_add(1, Ordering::Relaxed));

        let started_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        self.transferring.insert(
            id,
            NarTransferMetricEntry {
                meta,
                bytes_transferred: 0,
                started_at_unix_ms,
            },
        );

        NarTransferHandle {
            metric: Arc::clone(self),
            id,
        }
    }

    pub fn transferring(&self) -> Vec<NarTransferMetricEntry> {
        let mut items: Vec<NarTransferMetricEntry> = self
            .transferring
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        items.sort_by_key(|item| item.started_at_unix_ms);
        items
    }

    pub fn transferring_count(&self) -> usize {
        self.transferring.len()
    }

    fn remove(&self, id: NarTransferId) {
        self.transferring.remove(&id);
    }
}

pub struct NarTransferHandle {
    metric: Arc<NarTransferMetric>,
    id: NarTransferId,
}

impl NarTransferHandle {
    pub fn record_bytes(&self, bytes: u64) {
        if let Some(mut entry) = self.metric.transferring.get_mut(&self.id) {
            entry.bytes_transferred += bytes;
        };
    }
}

impl Drop for NarTransferHandle {
    fn drop(&mut self) {
        self.metric.remove(self.id);
    }
}
