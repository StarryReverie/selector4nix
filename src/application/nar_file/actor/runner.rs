use std::sync::Arc;
use std::time::{Duration, SystemTime};

use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, Context, EmptyInternal};
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::domain::common::expire_at::ExpireAt;
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_file::model::{NarFile, NarFileKey, NarFileLocation};
use crate::domain::nar_file::port::NarStreamData;
use crate::domain::nar_file::{NarFileRepository, NarFileService, StreamNarFileError};

pub enum NarFileRequest {
    StreamNarFile {
        reply_to: OneshotSender<Result<Option<NarStreamData>, StreamNarFileError>>,
        headers: PassthroughHeaders,
    },
    SetLocation(NarFileLocation),
}

pub struct NarFileActor {
    init: Option<NarFileKey>,
    context: Context<NarFileRequest, EmptyInternal>,
    nar_file_service: Arc<NarFileService>,
    nar_file_repository: Arc<dyn NarFileRepository>,
    nar_file_ttl: Duration,
}

impl NarFileActor {
    pub fn new(
        key: NarFileKey,
        nar_file_service: Arc<NarFileService>,
        nar_file_repository: Arc<dyn NarFileRepository>,
        nar_file_ttl: Duration,
    ) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init: Some(key),
            context,
            nar_file_service,
            nar_file_repository,
            nar_file_ttl,
        })
    }
}

impl Actor for NarFileActor {
    type Request = NarFileRequest;
    type Internal = EmptyInternal;
    type State = NarFile;

    fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
        &mut self.context
    }

    async fn on_start(&mut self) -> Option<Self::State> {
        let key = self.init.take()?;
        let nar_file = match self.nar_file_repository.get(&key).await {
            Ok(Some(nar_file)) => nar_file,
            Ok(None) => NarFile::new(key),
            Err(err) => {
                tracing::warn!(file_hash = %key.file_hash(), %err, "failed to get nar file from persistent cache, ignore and use default");
                NarFile::new(key)
            }
        };
        Some(nar_file)
    }

    async fn on_request(
        &mut self,
        state: Self::State,
        request: Self::Request,
    ) -> Option<Self::State> {
        match request {
            NarFileRequest::StreamNarFile { reply_to, headers } => {
                let now = SystemTime::now();
                let state = state.check_expiry_and_update(now);
                let (state, result) = self.nar_file_service.stream(state, headers, now).await;

                let _ = reply_to.send(result);
                if let Err(err) = self.nar_file_repository.save(state.clone()).await {
                    tracing::warn!(file_hash = %state.key().file_hash(), %err, "failed to write nar file to persistent cache, ignore");
                }

                Some(state)
            }
            NarFileRequest::SetLocation(location) => {
                let now = SystemTime::now();
                let expire_at = ExpireAt::since(now, self.nar_file_ttl);
                let state = state.on_located(location, expire_at);

                if let Err(err) = self.nar_file_repository.save(state.clone()).await {
                    tracing::warn!(file_hash = %state.key().file_hash(), %err, "failed to write nar file to persistent cache, ignore");
                }

                Some(state)
            }
        }
    }
}
