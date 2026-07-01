use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use serde::Serialize;

use crate::api::state::AppContext;
use crate::application::status::usecase::{CacheMode, StatusSnapshot, availability_status};
use crate::domain::nar_info::model::NarUrlRewriteOption;

#[derive(Serialize)]
pub struct StatusResponse {
    version: &'static str,
    cache_mode: &'static str,
    network: NetworkStatus,
    proxy: ProxyStatus,
    substituters: SubstitutersStatus,
    cache_stats: CacheStatsStatus,
}

#[derive(Serialize)]
struct NetworkStatus {
    periodic_probing: bool,
    tolerance_msecs: u64,
    nar_info_timeout_secs: u64,
    nar_timeout_secs: u64,
    max_concurrent_requests: usize,
    ignore_nar_info_error: bool,
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
    nar_info_lookup: LookupCacheStatus,
    nar_location: LookupCacheStatus,
    nar_info_store: StoreCacheStatus,
    nar_file_store: StoreCacheStatus,
}

#[derive(Serialize)]
struct LookupCacheStatus {
    entries: usize,
    capacity: usize,
    ttl_secs: u64,
}

#[derive(Serialize)]
struct StoreCacheStatus {
    entries: usize,
}

pub async fn get_status(State(ctx): State<Arc<AppContext>>) -> Json<StatusResponse> {
    Json(to_response(ctx.status_query_usecase().snapshot().await))
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
            periodic_probing: config.network.periodic_probing
                == crate::domain::substituter::model::PeriodicProbingOption::Enabled,
            tolerance_msecs: config.network.tolerance,
            nar_info_timeout_secs: config.network.nar_info_timeout.as_secs(),
            nar_timeout_secs: config.network.nar_timeout.as_secs(),
            max_concurrent_requests: config.network.max_concurrent_requests,
            ignore_nar_info_error: config.network.ignore_nar_info_error,
        },
        proxy: proxy_status(config.proxy.rewrite_nar_url),
        substituters: SubstitutersStatus {
            total: substituters.len(),
            available: snapshot.available_substituter_count(),
            items: substituters,
        },
        cache_stats: CacheStatsStatus {
            nar_info_lookup: LookupCacheStatus {
                entries: snapshot.nar_info_actor_entries,
                capacity: config.cache.nar_info_lookup_capacity,
                ttl_secs: config.cache.nar_info_lookup_ttl.as_secs(),
            },
            nar_location: LookupCacheStatus {
                entries: snapshot.nar_file_actor_entries,
                capacity: config.cache.nar_location_capacity,
                ttl_secs: config.cache.nar_location_ttl.as_secs(),
            },
            nar_info_store: StoreCacheStatus {
                entries: snapshot.nar_info_persistent_entries,
            },
            nar_file_store: StoreCacheStatus {
                entries: snapshot.nar_file_persistent_entries,
            },
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
