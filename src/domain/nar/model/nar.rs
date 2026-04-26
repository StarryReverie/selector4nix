use getset::Getters;

use crate::domain::nar::model::{NarInfoData, StorePathHash};
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarState {
    Unknown,
    NotFound,
    Resolved {
        best: SubstituterMeta,
        nar_info: NarInfoData,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Getters)]
#[getset(get = "pub")]
pub struct Nar {
    hash: StorePathHash,
    state: NarState,
}

impl Nar {
    pub fn new(hash: StorePathHash) -> Self {
        Self {
            hash,
            state: NarState::Unknown,
        }
    }

    pub fn on_resolved(mut self, best: SubstituterMeta, nar_info: NarInfoData) -> Self {
        self.state = NarState::Resolved { best, nar_info };
        self
    }

    pub fn on_not_found(mut self) -> Self {
        self.state = NarState::NotFound;
        self
    }
}
