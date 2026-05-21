use std::sync::Arc;

use getset::Getters;

use crate::application::nar_info::usecase::{NarInfoResolutionUseCase, NarStreamingUseCase};
use crate::application::substituter::usecase::SubstituterQueryUseCase;
use crate::infrastructure::config::CacheInfoConfiguration;

#[derive(Getters)]
#[getset(get = "pub")]
pub struct AppContext {
    substituter_query_usecase: SubstituterQueryUseCase,
    nar_resolution_usecase: NarInfoResolutionUseCase,
    nar_streaming_usecase: NarStreamingUseCase,
    cache_info: CacheInfoConfiguration,
}

impl AppContext {
    pub fn new(
        substituter_query_usecase: SubstituterQueryUseCase,
        nar_resolution_usecase: NarInfoResolutionUseCase,
        nar_streaming_usecase: NarStreamingUseCase,
        cache_info: CacheInfoConfiguration,
    ) -> Arc<Self> {
        Arc::new(Self {
            substituter_query_usecase,
            nar_resolution_usecase,
            nar_streaming_usecase,
            cache_info,
        })
    }
}
