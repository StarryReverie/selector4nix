use std::future::Future;

use anyhow::Result as AnyhowResult;
use dynosaur::dynosaur;

use crate::domain::nar::model::NarInfoQueryOutcome;
use crate::domain::substituter::model::Url;

#[dynosaur(pub DynNarInfoProvider = dyn(box) NarInfoProvider)]
pub trait NarInfoProvider: Send + Sync {
    fn provide_nar_info(
        &self,
        url: &Url,
    ) -> impl Future<Output = AnyhowResult<NarInfoQueryOutcome>> + Send;
}
