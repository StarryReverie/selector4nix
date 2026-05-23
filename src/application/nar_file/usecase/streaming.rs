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
        tracing::info!(nar_file = %key.to_file_name().value(), "acquiring nar stream from substituter");

        let address = self.nar_file_registry.get(&key).await;

        let response = address
            .ask(|reply_to| NarFileRequest::StreamNarFile(reply_to))
            .await
            .map_err(|_| anyhow::anyhow!("nar file actor terminated unexpectedly"))
            .wrap(AppErrorKind::Unknown)?;

        if let Ok(Some(data)) = &response {
            tracing::info!(nar_file = %key.to_file_name().value(), source_url = %data.source_url, "streamed nar from substituter")
        } else if let Ok(None) = &response {
            tracing::warn!(nar_file = %key.to_file_name().value(), "failed to find nar file on any substituter")
        } else {
            tracing::warn!(nar_file = %key.to_file_name().value(), "failed to stream nar")
        }

        response?.flat()
    }
}
