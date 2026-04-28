use std::sync::Arc;

use getset::Getters;

use crate::usecase::{NarUseCase, SubstituterUseCase};

#[derive(Getters)]
#[getset(get = "pub")]
pub struct AppContext {
    substituter_usecase: SubstituterUseCase,
    nar_usecase: NarUseCase,
}

impl AppContext {
    pub fn new(substituter_usecase: SubstituterUseCase, nar_usecase: NarUseCase) -> Arc<Self> {
        Arc::new(Self {
            substituter_usecase,
            nar_usecase,
        })
    }
}
