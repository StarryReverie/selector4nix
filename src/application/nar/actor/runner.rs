use std::sync::Arc;

use selector4nix_actor::actor::{
    Actor, ActorPre, ActorPreBuilder, AnyAddress, Context, EmptyInternal,
};
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::domain::nar::index::NarFileEvent;
use crate::domain::nar::model::{Nar, NarInfoData, NarInfoResolution};
use crate::domain::nar::service::{NarResolutionEvent, NarResolutionService, ResolveNarInfoError};

#[derive(Debug)]
pub enum NarRequest {
    ResolveNarInfo(OneshotSender<ResolveNarInfoResponse>),
}

#[derive(Debug)]
pub struct ResolveNarInfoResponse {
    pub result: Result<Option<NarInfoData>, ResolveNarInfoError>,
    pub events: Vec<NarResolutionEvent>,
}

impl ResolveNarInfoResponse {
    pub fn new(
        result: Result<Option<NarInfoData>, ResolveNarInfoError>,
        events: Vec<NarResolutionEvent>,
    ) -> Self {
        Self { result, events }
    }
}

pub struct NarActor {
    init: Option<Nar>,
    context: Context<NarRequest, EmptyInternal>,
    nar_info_query_service: Arc<NarResolutionService>,
    nar_file_index_pub: AnyAddress<NarFileEvent>,
}

impl NarActor {
    pub fn new(
        init: Nar,
        nar_info_query_service: Arc<NarResolutionService>,
        nar_file_index_pub: AnyAddress<NarFileEvent>,
    ) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init: Some(init),
            context,
            nar_info_query_service,
            nar_file_index_pub,
        })
    }

    async fn handle_request_resolve_nar_info(
        &self,
        nar: Nar,
        reply_to: OneshotSender<ResolveNarInfoResponse>,
    ) -> Nar {
        if let Some(resolution) = nar.resolution() {
            let res = Ok(resolution.nar_info().cloned());
            let _ = reply_to.send(ResolveNarInfoResponse::new(res, Vec::new()));
            return nar;
        }

        let (res, events) = self.nar_info_query_service.resolve(nar.hash()).await;
        match res {
            Ok(resolution) => {
                self.publish_nar_file_registration(&resolution).await;
                let res = Ok(resolution.nar_info().cloned());
                let nar = nar.on_resolved(resolution);
                let _ = reply_to.send(ResolveNarInfoResponse::new(res, events));
                nar
            }
            Err(err) => {
                let _ = reply_to.send(ResolveNarInfoResponse::new(Err(err), events));
                nar
            }
        }
    }

    async fn publish_nar_file_registration(&self, resolution: &NarInfoResolution) {
        if let NarInfoResolution::Resolved {
            nar_info,
            source_url,
            ..
        } = resolution
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
        if let Some(NarInfoResolution::Resolved { nar_info, .. }) = state.resolution() {
            let _ = self
                .nar_file_index_pub
                .tell(NarFileEvent::Evicted {
                    nar_file: nar_info.nar_file().clone(),
                })
                .await;
        }
    }
}
