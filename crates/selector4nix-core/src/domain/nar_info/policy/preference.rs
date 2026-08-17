use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::AppError;
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_info::DeadlineGroup;
use crate::domain::nar_info::model::{StorePathHash, UpstreamNarInfoData};
use crate::domain::nar_info::port::NarInfoProvider;
use crate::domain::nar_info::service::{
    NarInfoResolutionPolicy, QueryOutcome, ResolveNarInfoEvent, indeterminate_existence_error,
    query_substituter,
};
use crate::domain::substituter::SubstituterCandidate;
use crate::domain::substituter::model::SubstituterMeta;

pub struct PreferencePolicy {
    provider: Arc<dyn NarInfoProvider>,
    tolerance: Duration,
    ignore_query_error: bool,
}

impl PreferencePolicy {
    pub fn new(
        provider: Arc<dyn NarInfoProvider>,
        tolerance: Duration,
        ignore_query_error: bool,
    ) -> Self {
        Self {
            provider,
            tolerance,
            ignore_query_error,
        }
    }
}

#[async_trait]
impl NarInfoResolutionPolicy for PreferencePolicy {
    async fn resolve(
        &self,
        hash: &StorePathHash,
        headers: &PassthroughHeaders,
        substituters: Arc<Vec<SubstituterCandidate>>,
    ) -> (
        Result<Option<(UpstreamNarInfoData, SubstituterMeta)>, AppError>,
        Vec<ResolveNarInfoEvent>,
    ) {
        let tolerance = self.tolerance.as_millis() as i64;
        let headers = Arc::new(headers.clone());
        let mut substituter_graces = HashMap::new();
        for substituter in substituters.iter() {
            substituter_graces.insert(substituter, substituter.grace(tolerance));
        }

        let start = Instant::now();
        let mut query_tracker = JoinSet::new();
        let mut query_cancellers = HashMap::new();
        let mut query_deadlines: DeadlineGroup<&SubstituterCandidate> = DeadlineGroup::new();

        for substituter in substituters.iter() {
            let handle = query_tracker.spawn({
                let provider = Arc::clone(&self.provider);
                let sub = substituter.clone();
                let headers = Arc::clone(&headers);
                let hash = hash.clone();
                async move {
                    let (outcome, event) =
                        query_substituter(provider.as_ref(), &hash, headers.as_ref(), &sub).await;
                    (sub, outcome, event)
                }
            });
            query_cancellers.insert(substituter, handle);
        }

        let mut has_error = false;
        let mut events = Vec::new();
        let mut optimal = None;
        loop {
            let query_res = tokio::select! {
                Some(substituter) = query_deadlines.wait_earliest(), if !query_deadlines.is_empty() => {
                    tracing::trace!(hash = %hash.value(), substituter = %substituter.url(), elapsed = ?start.elapsed(), "prune substituter query");
                    if let Some(canceller) = query_cancellers.remove(substituter) {
                        canceller.abort()
                    };
                    query_deadlines.remove(substituter);
                    substituter_graces.remove(substituter);
                    continue;
                }
                res = query_tracker.join_next() => res,
            };

            match query_res {
                Some(Ok((substituter, outcome, event))) => {
                    query_cancellers.remove(&substituter);
                    query_deadlines.remove(&substituter);
                    match outcome {
                        QueryOutcome::Responded(data) => {
                            // A pruned substituter's late response contributes nothing.
                            let Some(current_grace) = substituter_graces.remove(&substituter)
                            else {
                                continue;
                            };
                            events.extend(event);
                            if let Some(data) = data {
                                let current = NarInfoQueryCandidate {
                                    substituter,
                                    nar_info: data.upstream_data,
                                    grace: current_grace,
                                    latency: data.latency,
                                };
                                update_optimal_and_deadlines(
                                    current,
                                    &mut optimal,
                                    start,
                                    &mut query_deadlines,
                                    &substituter_graces,
                                    hash.value(),
                                );
                            }
                        }
                        QueryOutcome::Offline => {
                            substituter_graces.remove(&substituter);
                            events.extend(event);
                        }
                        QueryOutcome::Error => {
                            substituter_graces.remove(&substituter);
                            if !self.ignore_query_error {
                                has_error = true;
                            }
                            events.extend(event);
                        }
                    }
                }
                Some(Err(_)) => (),
                None => break,
            }
        }

        match optimal {
            Some(optimal) => {
                let meta = optimal.substituter.meta().clone();
                (Ok(Some((optimal.nar_info, meta))), events)
            }
            None if !has_error => (Ok(None), events),
            None => (Err(indeterminate_existence_error()), events),
        }
    }
}

struct NarInfoQueryCandidate {
    substituter: SubstituterCandidate,
    nar_info: UpstreamNarInfoData,
    grace: i64,
    latency: Duration,
}

impl NarInfoQueryCandidate {
    fn calc_preference(&self) -> i64 {
        self.grace - self.latency.as_millis() as i64
    }
}

fn update_optimal_and_deadlines<'a>(
    current: NarInfoQueryCandidate,
    optimal: &mut Option<NarInfoQueryCandidate>,
    start: Instant,
    deadlines: &mut DeadlineGroup<&'a SubstituterCandidate>,
    graces: &HashMap<&'a SubstituterCandidate, i64>,
    hash: &str,
) {
    match optimal {
        Some(prev) if prev.calc_preference() > current.calc_preference() => (),
        _ => {
            tracing::trace!(%hash, substituter = %current.substituter.url().value(), preference = %current.calc_preference(), latency = ?current.latency, elapsed = ?start.elapsed(), "update optimal candidate");
            for (substituter, grace) in graces {
                let max_latency = 0.max(grace - current.calc_preference()) as u64;
                let deadline = start + Duration::from_millis(max_latency);
                deadlines.insert_or_set_earlier(substituter, deadline);
            }
            *optimal = Some(current);
        }
    }
}
