use std::sync::Arc;
use std::time::{Duration, SystemTime};

use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, Context, EmptyInternal};
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::AppError;
use crate::domain::common::expire_at::ExpireAt;
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_info::model::{NarInfo, ProxyNarInfoData, StorePathHash};
use crate::domain::nar_info::{NarInfoRepository, NarInfoService, ResolveNarInfoEvent};

#[derive(Debug)]
pub enum NarInfoRequest {
    ResolveNarInfo {
        reply_to: OneshotSender<ResolveNarInfoResponse>,
        headers: PassthroughHeaders,
    },
}

#[derive(Debug)]
pub struct ResolveNarInfoResponse {
    pub result: Result<Option<ProxyNarInfoData>, AppError>,
    pub events: Vec<ResolveNarInfoEvent>,
}

impl ResolveNarInfoResponse {
    pub fn new(
        result: Result<Option<ProxyNarInfoData>, AppError>,
        events: Vec<ResolveNarInfoEvent>,
    ) -> Self {
        Self { result, events }
    }
}

pub struct NarInfoActor {
    init: Option<StorePathHash>,
    context: Context<NarInfoRequest, EmptyInternal>,
    nar_info_service: Arc<NarInfoService>,
    nar_info_repository: Arc<dyn NarInfoRepository>,
    nar_info_ttl: Duration,
}

impl NarInfoActor {
    pub fn new(
        init: StorePathHash,
        nar_info_service: Arc<NarInfoService>,
        nar_info_repository: Arc<dyn NarInfoRepository>,
        nar_info_ttl: Duration,
    ) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init: Some(init),
            context,
            nar_info_service,
            nar_info_repository,
            nar_info_ttl,
        })
    }

    async fn handle_request_resolve_nar_info(
        &self,
        nar_info: NarInfo,
        reply_to: OneshotSender<ResolveNarInfoResponse>,
        headers: PassthroughHeaders,
    ) -> NarInfo {
        let nar_info = nar_info.check_expiry_and_update(SystemTime::now());

        if let Some(resolution) = nar_info.resolution() {
            let res = Ok(resolution.nar_info().cloned());
            let _ = reply_to.send(ResolveNarInfoResponse::new(res, Vec::new()));
            return nar_info;
        }

        let (res, events) = self
            .nar_info_service
            .resolve(nar_info.hash(), headers)
            .await;
        match res {
            Ok(resolution) => {
                let res = Ok(resolution.nar_info().cloned());
                let expire_at = ExpireAt::since(SystemTime::now(), self.nar_info_ttl);
                let nar_info = nar_info.on_resolved(resolution, expire_at);

                let _ = reply_to.send(ResolveNarInfoResponse::new(res, events));
                if let Err(err) = self.nar_info_repository.save(nar_info.clone()).await {
                    tracing::warn!(hash = %nar_info.hash().value(), %err, "failed to write nar info to persistent cache, ignore");
                }

                nar_info
            }
            Err(err) => {
                let _ = reply_to.send(ResolveNarInfoResponse::new(Err(err), events));
                nar_info
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
        let hash = self.init.take()?;
        let nar_info = match self.nar_info_repository.get(&hash).await {
            Ok(Some(nar_info)) => nar_info,
            Ok(None) => NarInfo::new(hash),
            Err(err) => {
                tracing::warn!(hash = %hash.value(), %err, "failed to get nar info from persistent cache, ignore and use default");
                NarInfo::new(hash)
            }
        };
        Some(nar_info)
    }

    async fn on_request(
        &mut self,
        state: Self::State,
        request: Self::Request,
    ) -> Option<Self::State> {
        match request {
            NarInfoRequest::ResolveNarInfo { reply_to, headers } => Some(
                self.handle_request_resolve_nar_info(state, reply_to, headers)
                    .await,
            ),
        }
    }
}
