mod nar_info_provider;
mod nar_stream_provider;
mod substituter_probing_provider;

mod segmented;

pub use nar_info_provider::ReqwestNarInfoProvider;
pub use nar_stream_provider::ReqwestNarStreamProvider;
pub use substituter_probing_provider::ReqwestSubstituterProbingProvider;
