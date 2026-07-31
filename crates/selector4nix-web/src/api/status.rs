use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use selector4nix_core::AppContext;
use selector4nix_core::application::usecase::status::{
    CacheMode, StatusSnapshot, availability_status,
};
use selector4nix_core::domain::nar_info::model::NarUrlRewriteOption;
use selector4nix_core::domain::substituter::model::PeriodicProbingOption;
use serde::Serialize;

#[derive(Serialize)]
pub struct StatusResponse {
    version: &'static str,
    cache_mode: &'static str,
    network: NetworkStatus,
    proxy: ProxyStatus,
    substituters: SubstitutersStatus,
    cache_stats: CacheStatsStatus,
    transferring: TransferringStatus,
}

#[derive(Serialize)]
struct NetworkStatus {
    periodic_probing: bool,
    tolerance_msecs: u64,
    nar_info_timeout_secs: u64,
    nar_timeout_secs: u64,
    max_concurrent_requests: usize,
    ignore_nar_info_error: bool,
    chunked_streaming: bool,
    streaming_chunk_max_len: usize,
    streaming_window_max_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct ProxyStatus {
    rewrite_to_target: &'static str,
}

#[derive(Serialize)]
struct SubstitutersStatus {
    total: usize,
    available: usize,
    items: Vec<SubstituterStatusItem>,
}

#[derive(Serialize)]
struct SubstituterStatusItem {
    url: String,
    priority: u32,
    status: &'static str,
    prev_failures: usize,
    has_credential: bool,
}

#[derive(Serialize)]
struct CacheStatsStatus {
    nar_info_cache: CacheStatus,
    nar_file_cache: CacheStatus,
    nar_info_store: StoreStatus,
    nar_file_store: StoreStatus,
}

#[derive(Serialize)]
struct CacheStatus {
    entries: usize,
    capacity: usize,
}

#[derive(Serialize)]
struct StoreStatus {
    entries: usize,
    ttl_secs: u64,
}

#[derive(Serialize)]
struct TransferringStatus {
    total: usize,
    items: Vec<TransferringStatusItem>,
}

#[derive(Serialize)]
struct TransferringStatusItem {
    store_path: Option<String>,
    nar_file_name: String,
    substituter: String,
    source_url: String,
    content_length: Option<u64>,
    bytes_transferred: u64,
    started_at_unix_ms: u64,
}

pub async fn get_status(State(ctx): State<Arc<AppContext>>) -> Json<StatusResponse> {
    Json(to_response(ctx.query_status_usecase.run().await))
}

fn to_response(snapshot: StatusSnapshot) -> StatusResponse {
    let runtime = &snapshot.runtime;
    let config = &runtime.config;
    let mut substituters: Vec<SubstituterStatusItem> = snapshot
        .substituters
        .iter()
        .map(|sub| SubstituterStatusItem {
            url: sub.url().to_string(),
            priority: sub.priority().value(),
            status: availability_status(sub.availability()),
            prev_failures: sub.prev_failures(),
            has_credential: runtime.authenticated_substituter_urls.contains(sub.url()),
        })
        .collect();
    substituters.sort_by_key(|item| item.priority);

    StatusResponse {
        version: runtime.version,
        cache_mode: match runtime.cache_mode {
            CacheMode::Persistent => "persistent",
            CacheMode::InMemory => "in_memory",
        },
        network: NetworkStatus {
            periodic_probing: config.network.periodic_probing == PeriodicProbingOption::Enabled,
            tolerance_msecs: config.network.tolerance,
            nar_info_timeout_secs: config.network.nar_info_timeout.as_secs(),
            nar_timeout_secs: config.network.nar_timeout.as_secs(),
            max_concurrent_requests: config.network.max_concurrent_requests,
            ignore_nar_info_error: config.network.ignore_nar_info_error,
            chunked_streaming: config.network.chunked_streaming,
            streaming_chunk_max_len: usize::from(config.network.streaming_chunk_max_len),
            streaming_window_max_len: usize::from(config.network.streaming_window_max_len),
        },
        proxy: proxy_status(config.proxy.rewrite_nar_url),
        substituters: SubstitutersStatus {
            total: substituters.len(),
            available: snapshot.available_substituter_count(),
            items: substituters,
        },
        cache_stats: CacheStatsStatus {
            nar_info_cache: CacheStatus {
                entries: snapshot.nar_info_actor_entries,
                capacity: config.cache.nar_info_lookup_capacity,
            },
            nar_file_cache: CacheStatus {
                entries: snapshot.nar_file_actor_entries,
                capacity: config.cache.nar_location_capacity,
            },
            nar_info_store: StoreStatus {
                entries: snapshot.nar_info_persistent_entries,
                ttl_secs: config.cache.nar_info_lookup_ttl.as_secs(),
            },
            nar_file_store: StoreStatus {
                entries: snapshot.nar_file_persistent_entries,
                ttl_secs: config.cache.nar_location_ttl.as_secs(),
            },
        },
        transferring: TransferringStatus {
            total: snapshot.transferring.len(),
            items: snapshot
                .transferring
                .into_iter()
                .map(|item| TransferringStatusItem {
                    store_path: item.meta.store_path,
                    nar_file_name: item.meta.nar_file_name.to_string(),
                    substituter: item.meta.substituter_url.to_string(),
                    source_url: item.meta.source_url.to_string(),
                    content_length: item.meta.content_length,
                    bytes_transferred: item.bytes_transferred,
                    started_at_unix_ms: item.started_at_unix_ms,
                })
                .collect(),
        },
    }
}

fn proxy_status(proxy: NarUrlRewriteOption) -> ProxyStatus {
    ProxyStatus {
        rewrite_to_target: match proxy {
            NarUrlRewriteOption::Keep => "keep",
            NarUrlRewriteOption::ToSelf => "self",
            NarUrlRewriteOption::ToUpstream => "upstream",
        },
    }
}
