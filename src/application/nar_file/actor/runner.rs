use std::sync::Arc;

use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, Context, EmptyInternal};
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::domain::nar_file::model::{NarFile, NarFileKey, NarFileLocation};
use crate::domain::nar_file::port::NarStreamData;
use crate::domain::nar_file::{NarFileService, StreamNarFileError};

pub enum NarFileRequest {
    StreamNarFile(OneshotSender<Result<Option<NarStreamData>, StreamNarFileError>>),
    SetLocation(NarFileLocation),
}

pub struct NarFileActor {
    init: Option<NarFileKey>,
    context: Context<NarFileRequest, EmptyInternal>,
    nar_file_service: Arc<NarFileService>,
}

impl NarFileActor {
    pub fn new(key: NarFileKey, nar_file_service: Arc<NarFileService>) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init: Some(key),
            context,
            nar_file_service,
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
        Some(NarFile::new(self.init.take()?))
    }

    async fn on_request(
        &mut self,
        state: Self::State,
        request: Self::Request,
    ) -> Option<Self::State> {
        match request {
            NarFileRequest::StreamNarFile(reply_to) => {
                let (state, result) = self.nar_file_service.stream(state).await;
                let _ = reply_to.send(result);
                Some(state)
            }
            NarFileRequest::SetLocation(location) => {
                let state = state.with_location(location);
                Some(state)
            }
        }
    }
}
