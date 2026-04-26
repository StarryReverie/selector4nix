use std::time::Duration;

use crate::domain::nar::model::NarInfoData;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NarInfoQueryOutcome {
    Found {
        data: NarInfoData,
        latency: Duration,
    },
    NotFound,
}

impl NarInfoQueryOutcome {
    pub fn unwrap_found(self) -> Option<(NarInfoData, Duration)> {
        match self {
            Self::Found { data, latency } => Some((data, latency)),
            Self::NotFound => None,
        }
    }
}
