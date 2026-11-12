use std::collections::HashMap;
use std::sync::Arc;

use tokio::task::JoinSet;

use crate::AppError;
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_info::model::{StorePathHash, UpstreamNarInfoData};
use crate::domain::nar_info::port::NarInfoProvider;
use crate::domain::nar_info::service::{
    QueryOutcome, indeterminate_existence_error, query_substituter,
};
use crate::domain::nar_info::{NarInfoResolutionPolicy, ResolveNarInfoEvent};
use crate::domain::substituter::SubstituterCandidate;
use crate::domain::substituter::model::SubstituterMeta;

pub struct PriorityPolicy {
    provider: Arc<dyn NarInfoProvider>,
    ignore_query_error: bool,
}

impl PriorityPolicy {
    pub fn new(provider: Arc<dyn NarInfoProvider>, ignore_query_error: bool) -> Self {
        Self {
            provider,
            ignore_query_error,
        }
    }
}

#[async_trait::async_trait]
impl NarInfoResolutionPolicy for PriorityPolicy {
    async fn resolve(
        &self,
        hash: &StorePathHash,
        headers: &PassthroughHeaders,
        substituters: Arc<Vec<SubstituterCandidate>>,
    ) -> (
        Result<Option<(UpstreamNarInfoData, SubstituterMeta)>, AppError>,
        Vec<ResolveNarInfoEvent>,
    ) {
        let headers = Arc::new(headers.clone());
        let mut queries = JoinSet::new();
        let mut query_cancellers = HashMap::new();

        for substituter in substituters.iter() {
            let handle = queries.spawn({
                let provider = Arc::clone(&self.provider);
                let hash = hash.clone();
                let headers = Arc::clone(&headers);
                let sub = substituter.clone();
                async move {
                    let (outcome, event) =
                        query_substituter(provider.as_ref(), &hash, &headers, &sub).await;
                    (sub, outcome, event)
                }
            });
            query_cancellers.insert(substituter, handle);
        }

        let mut optimal = None;
        let mut has_error = false;
        let mut events = Vec::new();
        while let Some(res) = queries.join_next().await {
            let Ok((substituter, outcome, event)) = res else {
                continue;
            };
            query_cancellers.remove(&substituter);
            match outcome {
                QueryOutcome::Responded(Some(data)) => {
                    // The current outcome is preferred over unfinished requests
                    // that have lower priority, which can be cancelled now.
                    query_cancellers.retain(|other_substituter, handle| {
                        if substituter.priority() <= other_substituter.priority() {
                            handle.abort();
                            false
                        } else {
                            true
                        }
                    });
                    // Even if requests of lower priority are cancelled eagerly, some
                    // may still finish before cancellation happens.
                    if optimal.as_ref().is_none_or(|prev: &NarInfoQueryCandidate| {
                        substituter.priority() < prev.substituter.priority()
                    }) {
                        optimal = Some(NarInfoQueryCandidate {
                            substituter,
                            nar_info: data.upstream_data,
                        });
                    }
                }
                QueryOutcome::Responded(None) => {}
                QueryOutcome::Offline => {}
                QueryOutcome::Error => {
                    if !self.ignore_query_error {
                        has_error = true;
                    }
                }
            }
            events.extend(event);
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
}
