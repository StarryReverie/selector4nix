use std::{borrow::Borrow, sync::Arc};

use getset::{CopyGetters, Getters};

use crate::domain::substituter::model::{Priority, Substituter, SubstituterMeta, Url};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubstituterAvailabilityEvent {
    BecameAvailable(SubstituterCandidate),
    BecameUnavailable(Url),
}

pub trait SubstituterAvailabilityIndex: Send + Sync {
    fn query_all(&self) -> Arc<Vec<SubstituterCandidate>>;
}

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
}

impl<S> From<S> for SubstituterCandidate
where
    S: Borrow<Substituter>,
{
    fn from(value: S) -> Self {
        let value = value.borrow();
        Self::new(value.target().clone(), value.is_maybe_ready())
    }
}
