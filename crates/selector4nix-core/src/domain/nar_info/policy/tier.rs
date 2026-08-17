use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use futures::stream::FuturesUnordered;

use crate::AppError;
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_info::model::{StorePathHash, UpstreamNarInfoData};
use crate::domain::nar_info::port::NarInfoProvider;
use crate::domain::nar_info::service::{
    NarInfoResolutionPolicy, QueryOutcome, ResolveNarInfoEvent, indeterminate_existence_error,
    query_substituter,
};
use crate::domain::substituter::SubstituterCandidate;
use crate::domain::substituter::model::SubstituterMeta;

pub struct TierPolicy {
    provider: Arc<dyn NarInfoProvider>,
    ignore_query_error: bool,
}

impl TierPolicy {
    pub fn new(provider: Arc<dyn NarInfoProvider>, ignore_query_error: bool) -> Self {
        Self {
            provider,
            ignore_query_error,
        }
    }
    async fn query_tier(
        &self,
        hash: &StorePathHash,
        headers: &PassthroughHeaders,
        tier: Vec<&SubstituterCandidate>,
    ) -> (TierOutcome, Vec<ResolveNarInfoEvent>) {
        let headers = Arc::new(headers.clone());
        let mut queries = FuturesUnordered::new();
        for substituter in tier {
            let sub = substituter.clone();
            let provider = Arc::clone(&self.provider);
            let headers = Arc::clone(&headers);
            let hash = hash.clone();
            queries.push(async move {
                let (outcome, event) =
                    query_substituter(provider.as_ref(), &hash, headers.as_ref(), &sub).await;
                (sub, outcome, event)
            });
        }

        let mut events = Vec::new();
        let mut has_error = false;
        while let Some((substituter, outcome, event)) = queries.next().await {
            events.extend(event);
            if let QueryOutcome::Responded(Some(data)) = outcome {
                // Dropping the stream aborts the remaining tier queries.
                return (
                    TierOutcome::Found(data.upstream_data, substituter.meta().clone()),
                    events,
                );
            }
            if matches!(outcome, QueryOutcome::Error) {
                has_error = true;
            }
        }

        if has_error && !self.ignore_query_error {
            (TierOutcome::Error, events)
        } else {
            (TierOutcome::NotFound, events)
        }
    }
}

#[async_trait]
impl NarInfoResolutionPolicy for TierPolicy {
    async fn resolve(
        &self,
        hash: &StorePathHash,
        headers: &PassthroughHeaders,
        substituters: Arc<Vec<SubstituterCandidate>>,
    ) -> (
        Result<Option<(UpstreamNarInfoData, SubstituterMeta)>, AppError>,
        Vec<ResolveNarInfoEvent>,
    ) {
        let mut all_events = Vec::new();
        let mut sticky_error = false;

        for tier in group_by_priority(&substituters) {
            let tier_priority = tier[0].priority().value();
            tracing::trace!(hash = %hash.value(), tier_priority, tier_len = tier.len(), "querying priority tier");
            let (outcome, events) = self.query_tier(hash, headers, tier).await;
            all_events.extend(events);
            match outcome {
                TierOutcome::Found(data, meta) => {
                    tracing::trace!(hash = %hash.value(), tier_priority, "tier found nar info");
                    return (Ok(Some((data, meta))), all_events);
                }
                TierOutcome::NotFound => {
                    tracing::trace!(hash = %hash.value(), tier_priority, "tier exhausted with not-found or ignorable errors, falling to next tier");
                }
                TierOutcome::Error => {
                    tracing::trace!(hash = %hash.value(), tier_priority, "tier exhausted with errors, falling to next tier");
                    sticky_error = true;
                }
            }
        }

        if sticky_error {
            (Err(indeterminate_existence_error()), all_events)
        } else {
            (Ok(None), all_events)
        }
    }
}

enum TierOutcome {
    Found(UpstreamNarInfoData, SubstituterMeta),
    NotFound,
    Error,
}
fn group_by_priority(substituters: &[SubstituterCandidate]) -> Vec<Vec<&SubstituterCandidate>> {
    let mut sorted = substituters.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|s| s.priority().value());
    let mut tiers: Vec<Vec<&SubstituterCandidate>> = Vec::new();
    for sub in sorted {
        match tiers.last_mut() {
            Some(tier) if tier[0].priority() == sub.priority() => tier.push(sub),
            _ => tiers.push(vec![sub]),
        }
    }
    tiers
}
