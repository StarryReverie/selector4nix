use std::sync::Arc;

use async_trait::async_trait;
use getset::{CopyGetters, Getters};

use crate::domain::common::url::Url;
use crate::domain::substituter::model::{Priority, Substituter, SubstituterMeta};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters, CopyGetters)]
pub struct SubstituterCandidate {
    #[getset(get = "pub")]
    meta: SubstituterMeta,
    #[getset(get_copy = "pub")]
    is_maybe_ready: bool,
}

impl SubstituterCandidate {
    pub fn new(meta: SubstituterMeta, is_maybe_ready: bool) -> Self {
        Self {
            meta,
            is_maybe_ready,
        }
    }

    pub fn url(&self) -> &Url {
        self.meta.url()
    }

    pub fn priority(&self) -> Priority {
        self.meta.priority()
    }

    pub fn grace(&self, tolerance: i64) -> i64 {
        -(tolerance * self.priority().value() as i64)
    }
}

impl From<&Substituter> for SubstituterCandidate {
    fn from(value: &Substituter) -> Self {
        Self::new(value.target().clone(), value.is_maybe_ready())
    }
}

#[async_trait]
pub trait SubstituterRepository: Send + Sync {
    async fn get(&self, url: &Url) -> Option<Substituter>;

    async fn query_all_available(&self) -> Arc<Vec<SubstituterCandidate>>;

    async fn save(&self, substituter: Substituter);
}
