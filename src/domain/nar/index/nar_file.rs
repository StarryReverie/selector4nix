use async_trait::async_trait;

use crate::domain::nar::model::NarFileName;
use crate::domain::substituter::model::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NarFileEvent {
    Registered {
        nar_file: NarFileName,
        source_url: Url,
    },
    Evicted {
        nar_file: NarFileName,
    },
}

#[async_trait]
pub trait NarFileIndex: Send + Sync {
    async fn get_source_url(&self, nar_file: &NarFileName) -> Option<Url>;
}
