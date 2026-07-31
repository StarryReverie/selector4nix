use crate::application::usecase::dashboard::GetDashboardOverviewUseCase;
use crate::application::usecase::nar_file::StreamNarFileUseCase;
use crate::application::usecase::nar_info::ResolveNarInfoUseCase;
use crate::application::usecase::status::QueryStatusUseCase;
use crate::infrastructure::config::CacheInfoConfiguration;

pub struct AppContext {
    pub resolve_nar_info_usecase: ResolveNarInfoUseCase,
    pub stream_nar_file_usecase: StreamNarFileUseCase,
    pub query_status_usecase: QueryStatusUseCase,
    pub get_dashboard_overview_usecase: GetDashboardOverviewUseCase,
    pub cache_info: CacheInfoConfiguration,
}
