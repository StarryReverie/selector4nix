use getset::Getters;

use crate::domain::nar::model::{NarInfoData, StorePathHash};
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarState {
    Empty,
    Resolved {
        best: SubstituterMeta,
        nar_info: NarInfoData,
    },
}

impl NarState {
    pub fn into_nar_info(self) -> Option<NarInfoData> {
        match self {
            Self::Resolved { nar_info, .. } => Some(nar_info),
            Self::Empty => None,
        }
    }
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
            state: NarState::Empty,
        }
    }

    pub fn on_resolved(mut self, best: SubstituterMeta, nar_info: NarInfoData) -> Self {
        self.state = NarState::Resolved { best, nar_info };
        self
    }
}
