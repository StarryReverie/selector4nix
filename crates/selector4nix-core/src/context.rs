use crate::application::usecase::dashboard::get_cache_stats::GetDashboardCacheStatsUseCase;
use crate::application::usecase::dashboard::{
    GetDashboardOverviewUseCase, GetDashboardTransferringUseCase,
};
use crate::application::usecase::nar_file::StreamNarFileUseCase;
use crate::application::usecase::nar_info::ResolveNarInfoUseCase;
use crate::application::usecase::status::QueryStatusUseCase;
use crate::infrastructure::config::CacheInfoConfiguration;

pub struct AppContext {
    pub resolve_nar_info_usecase: ResolveNarInfoUseCase,
    pub stream_nar_file_usecase: StreamNarFileUseCase,
    pub query_status_usecase: QueryStatusUseCase,
    pub get_dashboard_overview_usecase: GetDashboardOverviewUseCase,
    pub get_dashboard_transferring_usecase: GetDashboardTransferringUseCase,
    pub get_dashboard_cache_stats_usecase: GetDashboardCacheStatsUseCase,
    pub cache_info: CacheInfoConfiguration,
}
