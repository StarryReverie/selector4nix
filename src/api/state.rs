use std::sync::Arc;

use getset::Getters;

use crate::usecase::substituter::SubstituterUseCase;

#[derive(Getters)]
#[getset(get = "pub")]
pub struct AppContext {
    substituter_usecase: SubstituterUseCase,
}

impl AppContext {
    pub fn new(substituter_usecase: SubstituterUseCase) -> Arc<Self> {
        Arc::new(Self {
            substituter_usecase,
        })
    }
}
