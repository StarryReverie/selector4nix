use std::sync::Arc;

use selector4nix_actor::actor::{
    Actor, ActorPre, ActorPreBuilder, AnyAddress, Context, EmptyInternal,
};
use snafu::Snafu;
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::domain::nar::index::NarFileEvent;
use crate::domain::nar::model::{Nar, NarInfoData, NarState};
use crate::domain::nar::service::{NarInfoQueryService, NarQueryEvent};
use crate::domain::substituter::index::SubstituterAvailabilityIndex;

#[derive(Debug)]
pub enum NarRequest {
    ResolveNarInfo(OneshotSender<NarResolveResponse>),
}

#[derive(Debug)]
pub struct NarResolveResponse {
    pub result: Result<NarInfoData, ResolveNarInfoError>,
    pub events: Vec<NarQueryEvent>,
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
    init: Option<Nar>,
    context: Context<NarRequest, EmptyInternal>,
    nar_info_query_service: Arc<NarInfoQueryService>,
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
    nar_file_index_pub: AnyAddress<NarFileEvent>,
    rewrite_nar_url: bool,
    tolerance: u64,
}

impl NarActor {
    pub fn new(
        init: Nar,
        nar_info_query_service: Arc<NarInfoQueryService>,
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
        nar_file_index_pub: AnyAddress<NarFileEvent>,
        rewrite_nar_url: bool,
        tolerance: u64,
    ) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init: Some(init),
            context,
            substituter_availability_index,
            nar_info_query_service,
            nar_file_index_pub,
            rewrite_nar_url,
            tolerance,
        })
    }

    async fn handle_request_resolve_nar_info(
        &self,
        nar: Nar,
        reply: OneshotSender<NarResolveResponse>,
    ) -> Nar {
        match nar.state() {
            NarState::NotFound => {
                let _ = reply.send(NarResolveResponse {
                    result: NotFoundSnafu.fail(),
                    events: vec![],
                });
                nar
            }
            NarState::Resolved { .. } => {
                let result = self.build_resolution_result(&nar);
                let _ = reply.send(NarResolveResponse {
                    result,
                    events: vec![],
                });
                nar
            }
            NarState::Unknown => {
                let substituters = self.substituter_availability_index.query_all();
                let (nar, events) = self
                    .nar_info_query_service
                    .resolve_unknown(nar, substituters, self.rewrite_nar_url, self.tolerance)
                    .await;
                self.publish_nar_file_registration(&nar).await;
                let result = self.build_resolution_result(&nar);
                let _ = reply.send(NarResolveResponse { result, events });
                nar
            }
        }
    }

    fn build_resolution_result(&self, nar: &Nar) -> Result<NarInfoData, ResolveNarInfoError> {
        match nar.state() {
            NarState::Resolved {
                nar_info,
                source_url,
                ..
            } => {
                tracing::debug!(hash = %nar.hash().value(), %source_url, "selected source url from substituter");
                Ok(nar_info.clone())
            }
            NarState::NotFound => NotFoundSnafu.fail(),
            NarState::Unknown => FetchSnafu.fail(),
        }
    }

    async fn publish_nar_file_registration(&self, nar: &Nar) {
        if let NarState::Resolved {
            nar_info,
            source_url,
            ..
        } = nar.state()
        {
            let event = NarFileEvent::Registered {
                nar_file: nar_info.nar_file().clone(),
                source_url: source_url.clone(),
            };
            let _ = self.nar_file_index_pub.tell(event).await;
        }
    }
}

impl Actor for NarActor {
    type Request = NarRequest;
    type Internal = EmptyInternal;
    type State = Nar;

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
        tracing::debug!(hash = %state.hash().value(), "nar actor evicted");
        if let NarState::Resolved { nar_info, .. } = state.state() {
            let _ = self
                .nar_file_index_pub
                .tell(NarFileEvent::Evicted {
                    nar_file: nar_info.nar_file().clone(),
                })
                .await;
        }
    }
}
