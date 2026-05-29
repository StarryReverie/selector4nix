use std::sync::Arc;
use std::time::{Duration, SystemTime};

use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, Context, EmptyInternal};
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::domain::common::expire_at::ExpireAt;
use crate::domain::nar_info::model::{NarInfo, ProxyNarInfoData};
use crate::domain::nar_info::{NarInfoService, ResolveNarInfoError, ResolveNarInfoEvent};

#[derive(Debug)]
pub enum NarInfoRequest {
    ResolveNarInfo(OneshotSender<ResolveNarInfoResponse>),
}

#[derive(Debug)]
pub struct ResolveNarInfoResponse {
    pub result: Result<Option<ProxyNarInfoData>, ResolveNarInfoError>,
    pub events: Vec<ResolveNarInfoEvent>,
}

impl ResolveNarInfoResponse {
    pub fn new(
        result: Result<Option<ProxyNarInfoData>, ResolveNarInfoError>,
        events: Vec<ResolveNarInfoEvent>,
    ) -> Self {
        Self { result, events }
    }
}

pub struct NarInfoActor {
    init: Option<NarInfo>,
    context: Context<NarInfoRequest, EmptyInternal>,
    nar_info_service: Arc<NarInfoService>,
    nar_info_ttl: Duration,
}

impl NarInfoActor {
    pub fn new(
        init: NarInfo,
        nar_info_service: Arc<NarInfoService>,
        nar_info_ttl: Duration,
    ) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init: Some(init),
            context,
            nar_info_service,
            nar_info_ttl,
        })
    }

    async fn handle_request_resolve_nar_info(
        &self,
        nar: NarInfo,
        reply_to: OneshotSender<ResolveNarInfoResponse>,
    ) -> NarInfo {
        let nar = nar.check_expiry_and_update(SystemTime::now());

        if let Some(resolution) = nar.resolution() {
            let res = Ok(resolution.nar_info().cloned());
            let _ = reply_to.send(ResolveNarInfoResponse::new(res, Vec::new()));
            return nar;
        }

        let (res, events) = self.nar_info_service.resolve(nar.hash()).await;
        match res {
            Ok(resolution) => {
                let res = Ok(resolution.nar_info().cloned());
                let expire_at = ExpireAt::since(SystemTime::now(), self.nar_info_ttl);
                let nar = nar.on_resolved(resolution, expire_at);
                let _ = reply_to.send(ResolveNarInfoResponse::new(res, events));
                nar
            }
            Err(err) => {
                let _ = reply_to.send(ResolveNarInfoResponse::new(Err(err), events));
                nar
            }
        }
    }
}

impl Actor for NarInfoActor {
    type Request = NarInfoRequest;
    type Internal = EmptyInternal;
    type State = NarInfo;

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
            NarInfoRequest::ResolveNarInfo(reply) => {
                Some(self.handle_request_resolve_nar_info(state, reply).await)
            }
        }
    }
}
