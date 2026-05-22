use std::sync::Arc;

use crate::application::nar_file::actor::{NarFileActorRegistry, NarFileRequest};
use crate::application::{AppErrorKind, AppOptionExt, AppResult, AppResultExt};
use crate::domain::nar_file::model::NarFileKey;
use crate::domain::nar_file::port::NarStreamData;

pub struct NarFileStreamingUseCase {
    nar_file_registry: Arc<NarFileActorRegistry>,
}

impl NarFileStreamingUseCase {
    pub fn new(nar_file_registry: Arc<NarFileActorRegistry>) -> Self {
        Self { nar_file_registry }
    }

    pub async fn stream_nar(&self, key: NarFileKey) -> AppResult<NarStreamData> {
        let address = self.nar_file_registry.get(&key).await;

        let response = address
            .ask(|reply_to| NarFileRequest::StreamNarFile(reply_to))
            .await
            .map_err(|_| anyhow::anyhow!("nar file actor terminated unexpectedly"))
            .wrap(AppErrorKind::Unknown)?;

        response?.flat()
    }
}
