use getset::Getters;

use crate::application::dashboard::usecase::DashboardOverviewQueryUseCase;
use crate::application::nar_file::usecase::NarFileStreamingUseCase;
use crate::application::nar_info::usecase::NarInfoResolutionUseCase;
use crate::application::status::usecase::StatusQueryUseCase;
use crate::infrastructure::config::CacheInfoConfiguration;

#[derive(Getters)]
#[getset(get = "pub")]
pub struct AppContext {
    pub nar_info_resolution_usecase: NarInfoResolutionUseCase,
    pub nar_file_streaming_usecase: NarFileStreamingUseCase,
    pub status_query_usecase: StatusQueryUseCase,
    pub dashboard_overview_query_usecase: DashboardOverviewQueryUseCase,
    pub cache_info: CacheInfoConfiguration,
}
