use std::sync::Arc;

use getset::Getters;

use crate::application::nar_file::usecase::NarFileStreamingUseCase;
use crate::application::nar_info::usecase::NarInfoResolutionUseCase;
use crate::application::status::usecase::StatusQueryUseCase;
use crate::application::substituter::usecase::SubstituterQueryUseCase;
use crate::infrastructure::config::CacheInfoConfiguration;

#[derive(Getters)]
#[getset(get = "pub")]
pub struct AppContext {
    substituter_query_usecase: SubstituterQueryUseCase,
    nar_info_resolution_usecase: NarInfoResolutionUseCase,
    nar_file_streaming_usecase: NarFileStreamingUseCase,
    status_query_usecase: StatusQueryUseCase,
    cache_info: CacheInfoConfiguration,
}

impl AppContext {
    pub fn new(
        substituter_query_usecase: SubstituterQueryUseCase,
        nar_info_resolution_usecase: NarInfoResolutionUseCase,
        nar_file_streaming_usecase: NarFileStreamingUseCase,
        status_query_usecase: StatusQueryUseCase,
        cache_info: CacheInfoConfiguration,
    ) -> Arc<Self> {
        Arc::new(Self {
            substituter_query_usecase,
            nar_info_resolution_usecase,
            nar_file_streaming_usecase,
            status_query_usecase,
            cache_info,
        })
    }
}
