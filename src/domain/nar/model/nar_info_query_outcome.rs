use std::time::Duration;

use crate::domain::nar::model::NarInfoData;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NarInfoQueryOutcome {
    Found {
        original_data: NarInfoData,
        latency: Duration,
    },
    NotFound,
}

impl NarInfoQueryOutcome {
    pub fn unwrap_found(self) -> Option<(NarInfoData, Duration)> {
        match self {
            Self::Found {
                original_data,
                latency,
            } => Some((original_data, latency)),
            Self::NotFound => None,
        }
    }
}
