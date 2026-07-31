use crate::application::dashboard::usecase::GetDashboardOverviewUseCase;
use crate::application::nar_file::usecase::StreamNarFileUseCase;
use crate::application::nar_info::usecase::ResolveNarInfoUseCase;
use crate::application::status::usecase::QueryStatusUseCase;
use crate::infrastructure::config::CacheInfoConfiguration;

pub struct AppContext {
    pub resolve_nar_info_usecase: ResolveNarInfoUseCase,
    pub stream_nar_file_usecase: StreamNarFileUseCase,
    pub query_status_usecase: QueryStatusUseCase,
    pub get_dashboard_overview_usecase: GetDashboardOverviewUseCase,
    pub cache_info: CacheInfoConfiguration,
}
