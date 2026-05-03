use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use snafu::Snafu;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::domain::nar::model::{
    AbnormalQueryOutcome, Nar, NarInfoData, NarInfoQueryOutcome, NarState, StorePathHash,
};
use crate::domain::nar::port::NarInfoProvider;
use crate::domain::nar::service::DeadlineGroup;
use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::domain::substituter::model::{Substituter, SubstituterMeta, Url};

pub struct NarResolutionService {
    nar_info_provider: Arc<dyn NarInfoProvider>,
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
    rewrite_nar_url: bool,
    tolerance: u64,
}

impl NarResolutionService {
    pub fn new(
        nar_info_provider: Arc<dyn NarInfoProvider>,
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
        rewrite_nar_url: bool,
        tolerance: u64,
    ) -> Self {
        Self {
            nar_info_provider,
            substituter_availability_index,
            rewrite_nar_url,
            tolerance,
        }
    }

    pub async fn resolve(
        &self,
        nar: Nar,
    ) -> (Result<Nar, ResolveNarInfoError>, Vec<NarResolutionEvent>) {
        match nar.state() {
            NarState::NotFound => (NotFoundSnafu.fail(), Vec::new()),
            NarState::Resolved { .. } => (Ok(nar), Vec::new()),
            NarState::Unknown => {
                let (outcome, events) = self.resolve_unknown(nar.hash()).await;
                let nar = nar.on_query_completed(outcome, self.rewrite_nar_url);

                let source_url = nar.source_url().map_or("".into(), ToString::to_string);
                tracing::debug!(hash = %nar.hash().value(), %source_url, "selected source url from substituter");

                (Ok(nar), events)
            }
        }
    }

    async fn resolve_unknown(
        &self,
        hash: &StorePathHash,
    ) -> (
        Result<(NarInfoData, SubstituterMeta), AbnormalQueryOutcome>,
        Vec<NarResolutionEvent>,
    ) {
        let substituters = self.substituter_availability_index.query_all();

        let (events, outcome) = self
            .query_substituters(hash, substituters, self.tolerance)
            .await;
        let outcome = outcome.map(|(substituter, nar_info)| {
            let substituter = substituter.target().clone();
            (nar_info, substituter)
        });
        (outcome, events)
    }

    async fn query_substituters(
        &self,
        hash: &StorePathHash,
        substituters: Arc<Vec<Substituter>>,
        tolerance: u64,
    ) -> (
        Vec<NarResolutionEvent>,
        Result<(Substituter, NarInfoData), AbnormalQueryOutcome>,
    ) {
        let mut substituter_graces = HashMap::new();
        for substituter in substituters.iter() {
            substituter_graces.insert(substituter, substituter.grace(tolerance as i64));
        }

        let start = Instant::now();
        let mut query_tracker = JoinSet::new();
        let mut query_cancellers = HashMap::new();
        let mut query_deadlines: DeadlineGroup<&Substituter> = DeadlineGroup::new();

        for substituter in substituters.iter() {
            let handle = query_tracker.spawn({
                let provider = Arc::clone(&self.nar_info_provider);
                let url = hash.on_substituter(substituter.target());
                let sub = substituter.clone();
                async move { (sub, provider.provide_nar_info(&url).await) }
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
                Some(Ok((substituter, Ok(outcome)))) => {
                    query_cancellers.remove(&substituter);
                    query_deadlines.remove(&substituter);
                    let cur_grace = substituter_graces.remove(&substituter).unwrap();
                    if !substituter.is_normal() {
                        let url = substituter.url().clone();
                        events.push(NarResolutionEvent::SubstituterSucceeded(url));
                    }

                    if let NarInfoQueryOutcome::Found {
                        original_data,
                        latency,
                    } = outcome
                    {
                        let current = NarInfoQueryCandidate {
                            substituter,
                            nar_info: original_data,
                            grace: cur_grace,
                            latency,
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
                Some(Ok((substituter, Err(_)))) => {
                    has_error = true;
                    query_cancellers.remove(&substituter);
                    query_deadlines.remove(&substituter);
                    substituter_graces.remove(&substituter);
                    let url = substituter.url().clone();
                    events.push(NarResolutionEvent::SubstituterFailed(url));
                }
                Some(Err(_)) => (),
                None => break,
            }
        }

        match optimal {
            Some(optimal) => (events, Ok((optimal.substituter, optimal.nar_info))),
            None if has_error => (events, Err(AbnormalQueryOutcome::Error)),
            None => (events, Err(AbnormalQueryOutcome::NotFound)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarResolutionEvent {
    SubstituterSucceeded(Url),
    SubstituterFailed(Url),
}

#[derive(Snafu, Debug)]
#[non_exhaustive]
pub enum ResolveNarInfoError {
    #[snafu(display("could not find narinfo on any substituter"))]
    NotFound,
    #[snafu(display("could not fetch narinfo"))]
    Fetch,
}

struct NarInfoQueryCandidate {
    substituter: Substituter,
    nar_info: NarInfoData,
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
    deadlines: &mut DeadlineGroup<&'a Substituter>,
    graces: &HashMap<&'a Substituter, i64>,
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
