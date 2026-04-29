use std::sync::Arc;

use getset::Getters;

use crate::infrastructure::config::CacheInfoConfiguration;
use crate::usecase::{NarUseCase, SubstituterUseCase};

#[derive(Getters)]
#[getset(get = "pub")]
pub struct AppContext {
    substituter_usecase: SubstituterUseCase,
    nar_usecase: NarUseCase,
    cache_info: CacheInfoConfiguration,
}

impl AppContext {
    pub fn new(
        substituter_usecase: SubstituterUseCase,
        nar_usecase: NarUseCase,
        cache_info: CacheInfoConfiguration,
    ) -> Arc<Self> {
        Arc::new(Self {
            substituter_usecase,
            nar_usecase,
            cache_info,
        })
    }
}
