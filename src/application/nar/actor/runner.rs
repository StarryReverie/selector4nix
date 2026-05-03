use std::sync::Arc;

use selector4nix_actor::actor::{
    Actor, ActorPre, ActorPreBuilder, AnyAddress, Context, EmptyInternal,
};
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::domain::nar::index::NarFileEvent;
use crate::domain::nar::model::{Nar, NarInfoData, NarState};
use crate::domain::nar::service::{
    NarResolutionService, NarResolutionEvent, ResolveNarInfoError,
};

#[derive(Debug)]
pub enum NarRequest {
    ResolveNarInfo(OneshotSender<NarResolveResponse>),
}

#[derive(Debug)]
pub struct NarResolveResponse {
    pub result: Result<NarInfoData, ResolveNarInfoError>,
    pub events: Vec<NarResolutionEvent>,
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
        reply: OneshotSender<NarResolveResponse>,
    ) -> Nar {
        let (res, events) = self.nar_info_query_service.resolve(nar.clone()).await;
        match res {
            Ok(nar_next) => {
                self.publish_nar_file_registration(&nar_next).await;
                let _ = reply.send(NarResolveResponse {
                    result: Ok(nar_next
                        .nar_info()
                        .cloned()
                        .expect("the nar info should have been resolved")),
                    events,
                });
                nar_next
            }
            Err(err) => {
                let _ = reply.send(NarResolveResponse {
                    result: Err(err),
                    events,
                });
                nar
            }
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
