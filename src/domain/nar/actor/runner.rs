use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use selector4nix_actor::actor::{
    Actor, ActorPre, ActorPreBuilder, AnyAddress, Context, EmptyInternal,
};
use snafu::Snafu;
use tokio::sync::oneshot::Sender as OneshotSender;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::domain::nar::actor::{
    AbnormalQueryOutcome, DeadlineGroup, NarActorEffect, NarActorState,
};
use crate::domain::nar::index::NarFileEvent;
use crate::domain::nar::model::{NarInfoData, NarInfoQueryOutcome, NarState};
use crate::domain::nar::port::NarInfoProvider;
use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::domain::substituter::model::Substituter;

#[derive(Debug)]
pub enum NarRequest {
    ResolveNarInfo(OneshotSender<NarResolveResponse>),
}

#[derive(Debug)]
pub struct NarResolveResponse {
    pub result: Result<NarInfoData, ResolveNarInfoError>,
    pub effects: Vec<NarActorEffect>,
}

#[derive(Snafu, Debug)]
#[non_exhaustive]
pub enum ResolveNarInfoError {
    #[snafu(display("could not find narinfo on any substituter"))]
    NotFound,
    #[snafu(display("could not fetch narinfo"))]
    Fetch,
}

pub struct NarActor {
    init: Option<NarActorState>,
    context: Context<NarRequest, EmptyInternal>,
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
    nar_info_provider: Arc<dyn NarInfoProvider>,
    nar_file_index_pub: AnyAddress<NarFileEvent>,
    rewrite_nar_url: bool,
}

impl NarActor {
    pub fn new(
        init: impl Into<NarActorState>,
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
        nar_info_provider: Arc<dyn NarInfoProvider>,
        nar_file_index_pub: AnyAddress<NarFileEvent>,
        rewrite_nar_url: bool,
    ) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init: Some(init.into()),
            context,
            substituter_availability_index,
            nar_info_provider,
            nar_file_index_pub,
            rewrite_nar_url,
        })
    }

    async fn handle_request_resolve_nar_info(
        &mut self,
        state: NarActorState,
        reply: OneshotSender<NarResolveResponse>,
    ) -> NarActorState {
        match state.inner().state() {
            NarState::NotFound => {
                let _ = reply.send(NarResolveResponse {
                    result: NotFoundSnafu.fail(),
                    effects: vec![],
                });
                state
            }
            NarState::Resolved { nar_info, .. } => {
                let _ = reply.send(NarResolveResponse {
                    result: Ok(nar_info.clone()),
                    effects: vec![],
                });
                state
            }
            NarState::Unknown => {
                let substituters = self.substituter_availability_index.query_all();
                let (effects, outcome) =
                    query_nar_info(&state, substituters, Arc::clone(&self.nar_info_provider)).await;
                let state = NarActorState::on_query_completed(state, outcome, self.rewrite_nar_url);
                self.publish_and_reply_nar_resolution(reply, effects, &state)
                    .await;
                state
            }
        }
    }

    async fn publish_and_reply_nar_resolution(
        &mut self,
        reply: OneshotSender<NarResolveResponse>,
        effects: Vec<NarActorEffect>,
        state: &NarActorState,
    ) {
        let result = match state.inner().state() {
            NarState::Resolved {
                nar_info,
                source_url,
                ..
            } => {
                tracing::info!(hash = %state.inner().hash().value(), substituter = %source_url, "selected substituter");
                let event = NarFileEvent::Registered {
                    nar_file: nar_info.nar_file().clone(),
                    source_url: source_url.clone(),
                };
                let _ = self.nar_file_index_pub.tell(event).await;
                Ok(nar_info.clone())
            }
            NarState::NotFound => NotFoundSnafu.fail(),
            NarState::Unknown => FetchSnafu.fail(),
        };
        let _ = reply.send(NarResolveResponse { result, effects });
    }
}

impl Actor for NarActor {
    type Request = NarRequest;
    type Internal = EmptyInternal;
    type State = NarActorState;

    fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
        &mut self.context
    }

    async fn on_start(&mut self) -> Option<Self::State> {
        self.init.take()
    }

    async fn on_request(
        &mut self,
        state: Self::State,
        request: Self::Request,
    ) -> Option<Self::State> {
        match request {
            NarRequest::ResolveNarInfo(reply) => {
                Some(self.handle_request_resolve_nar_info(state, reply).await)
            }
        }
    }

    async fn on_shutdown(&mut self, state: Self::State) {
        tracing::debug!(hash = %state.inner().hash().value(), "nar actor evicted");
        if let NarState::Resolved { nar_info, .. } = state.inner().state() {
            let _ = self
                .nar_file_index_pub
                .tell(NarFileEvent::Evicted {
                    nar_file: nar_info.nar_file().clone(),
                })
                .await;
        }
    }
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

async fn query_nar_info(
    state: &NarActorState,
    substituters: Arc<Vec<Substituter>>,
    nar_info_provider: Arc<dyn NarInfoProvider>,
) -> (
    Vec<NarActorEffect>,
    Result<(Substituter, NarInfoData), AbnormalQueryOutcome>,
) {
    const TOLERANCE: i64 = 50;
    let mut substituter_graces = HashMap::new();
    for substituter in substituters.iter() {
        substituter_graces.insert(substituter, substituter.grace(TOLERANCE));
    }

    let start = Instant::now();
    let mut query_tracker = JoinSet::new();
    let mut query_cancellers = HashMap::new();
    let mut query_deadlines: DeadlineGroup<&Substituter> = DeadlineGroup::new();

    for substituter in substituters.iter() {
        let handle = query_tracker.spawn({
            let provider = Arc::clone(&nar_info_provider);
            let url = state.inner().hash().on_substituter(substituter.target());
            let sub = substituter.clone();
            async move { (sub, provider.provide_nar_info(&url).await) }
        });
        query_cancellers.insert(substituter, handle);
    }

    let mut has_error = false;
    let mut effects = Vec::new();
    let mut optimal = None;
    loop {
        let query_res = tokio::select! {
            Some(substituter) = query_deadlines.wait_earliest(), if !query_deadlines.is_empty() => {
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
                    effects.push(NarActorEffect::ReportSubstituterSuccess(url));
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
                    );
                }
            }
            Some(Ok((substituter, Err(_)))) => {
                has_error = true;
                query_cancellers.remove(&substituter);
                query_deadlines.remove(&substituter);
                substituter_graces.remove(&substituter);
                let url = substituter.url().clone();
                effects.push(NarActorEffect::ReportSubstituterFailure(url));
            }
            Some(Err(_)) => (),
            None => break,
        }
    }

    match optimal {
        Some(optimal) => (effects, Ok((optimal.substituter, optimal.nar_info))),
        None if has_error => (effects, Err(AbnormalQueryOutcome::Error)),
        None => (effects, Err(AbnormalQueryOutcome::NotFound)),
    }
}

fn update_optimal_and_deadlines<'a>(
    current: NarInfoQueryCandidate,
    optimal: &mut Option<NarInfoQueryCandidate>,
    start: Instant,
    deadlines: &mut DeadlineGroup<&'a Substituter>,
    graces: &HashMap<&'a Substituter, i64>,
) {
    match optimal {
        Some(optimal) if optimal.calc_preference() > current.calc_preference() => (),
        _ => {
            for (substituter, grace) in graces {
                let max_latency = 0.max(grace - current.calc_preference()) as u64;
                let deadline = start + Duration::from_millis(max_latency);
                deadlines.insert_or_set_earlier(substituter, deadline);
            }
            *optimal = Some(current);
        }
    }
}
