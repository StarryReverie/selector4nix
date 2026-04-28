use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use selector4nix_actor::actor::{
    Actor, ActorPre, ActorPreBuilder, AnyAddress, Context, EmptyInternal,
};
use snafu::Snafu;
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::domain::nar::actor::{NarActorEffect, NarActorState};
use crate::domain::nar::index::NarFileEvent;
use crate::domain::nar::model::{NarInfoData, NarInfoQueryOutcome, NarState};
use crate::domain::nar::port::NarInfoProvider;
use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::domain::substituter::model::SubstituterMeta;

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
}

impl NarActor {
    pub fn new(
        init: impl Into<NarActorState>,
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
        nar_info_provider: Arc<dyn NarInfoProvider>,
        nar_file_index_pub: AnyAddress<NarFileEvent>,
    ) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init: Some(init.into()),
            context,
            substituter_availability_index,
            nar_info_provider,
            nar_file_index_pub,
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
                let outcomes_fut = substituters
                    .iter()
                    .map(|meta| self.start_nar_info_query(&state, meta));
                let outcomes = futures::future::join_all(outcomes_fut).await;

                let (effects, state) =
                    NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters);

                let result = match state.inner().state() {
                    NarState::NotFound => {
                        tracing::info!(hash = %state.inner().hash().value(), "no substituter has narinfo");
                        NotFoundSnafu.fail()
                    }
                    NarState::Resolved { best, nar_info } => {
                        tracing::info!(
                            hash = %state.inner().hash().value(),
                            substituter = %best.url(),
                            "selected substituter"
                        );
                        // FIXME: Make storage prefix configurable, as the following code is broken for garnix
                        let storage_prefix = best.url().as_dir().join("nar").unwrap();
                        let _ = self
                            .nar_file_index_pub
                            .tell(NarFileEvent::Registered {
                                nar_file: nar_info.nar_file().to_string(),
                                storage_prefix,
                            })
                            .await;
                        Ok(nar_info.clone())
                    }
                    NarState::Unknown => {
                        tracing::info!(
                            hash = %state.inner().hash().value(),
                            "no substituter replied normally"
                        );
                        FetchSnafu.fail()
                    }
                };
                let _ = reply.send(NarResolveResponse { result, effects });
                state
            }
        }
    }

    async fn start_nar_info_query(
        &self,
        state: &NarActorState,
        meta: &SubstituterMeta,
    ) -> AnyhowResult<NarInfoQueryOutcome> {
        let provider = Arc::clone(&self.nar_info_provider);
        let url = state.inner().hash().on_substituter(meta);
        provider.provide_nar_info(&url).await
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
                    nar_file: nar_info.nar_file().to_string(),
                })
                .await;
        }
    }
}
