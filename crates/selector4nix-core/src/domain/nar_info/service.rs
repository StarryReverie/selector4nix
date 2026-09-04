use std::sync::Arc;

use async_trait::async_trait;

use crate::AppError;
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::common::url::Url;
use crate::domain::nar_info::model::{
    NarFileName, NarInfoResolution, NarUrlRewriteOption, StorePathHash, UpstreamNarInfoData,
};
use crate::domain::nar_info::port::{NarInfoProvider, NarInfoQueryData, QueryNarInfoError};
use crate::domain::substituter::SubstituterCandidate;
use crate::domain::substituter::SubstituterRepository;
use crate::domain::substituter::model::SubstituterMeta;

pub struct NarInfoService {
    resolution_policy: Arc<dyn NarInfoResolutionPolicy>,
    substituter_repository: Arc<dyn SubstituterRepository>,
    rewrite_nar_url: NarUrlRewriteOption,
}

impl NarInfoService {
    pub fn new(
        resolution_policy: Arc<dyn NarInfoResolutionPolicy>,
        substituter_repository: Arc<dyn SubstituterRepository>,
        rewrite_nar_url: NarUrlRewriteOption,
    ) -> Self {
        Self {
            resolution_policy,
            substituter_repository,
            rewrite_nar_url,
        }
    }

    pub async fn resolve(
        &self,
        hash: &StorePathHash,
        headers: PassthroughHeaders,
    ) -> (
        Result<NarInfoResolution, AppError>,
        Vec<ResolveNarInfoEvent>,
    ) {
        let (res, mut events) = self.resolve_unknown(hash, headers).await;
        match res {
            Ok(outcome) => {
                let resolution =
                    NarInfoResolution::from_completed_query(outcome, self.rewrite_nar_url);

                if let NarInfoResolution::Resolved {
                    nar_info,
                    substituter,
                    source_url,
                } = &resolution
                {
                    events.push(ResolveNarInfoEvent::NarFileLocated {
                        nar_file: nar_info.nar_file().clone(),
                        substituter: substituter.clone(),
                        source_url: source_url.clone(),
                        store_path_hash: hash.clone(),
                    });
                }

                if let Some(source_url) = resolution.source_url() {
                    tracing::debug!(hash = %hash.value(), %source_url, "selected source url from substituter");
                }

                (Ok(resolution), events)
            }
            Err(err) => (Err(err), events),
        }
    }

    async fn resolve_unknown(
        &self,
        hash: &StorePathHash,
        headers: PassthroughHeaders,
    ) -> (
        Result<Option<(UpstreamNarInfoData, SubstituterMeta)>, AppError>,
        Vec<ResolveNarInfoEvent>,
    ) {
        let substituters = self.substituter_repository.query_all_available().await;

        self.resolution_policy
            .resolve(hash, &headers, substituters)
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResolveNarInfoEvent {
    SubstituterSucceeded(Url),
    SubstituterOffline(Url),
    SubstituterError(Url),
    NarFileLocated {
        nar_file: NarFileName,
        substituter: SubstituterMeta,
        source_url: Url,
        store_path_hash: StorePathHash,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolutionPolicyOption {
    Preference,
    Tier,
}

#[async_trait]
pub trait NarInfoResolutionPolicy: Send + Sync {
    async fn resolve(
        &self,
        hash: &StorePathHash,
        headers: &PassthroughHeaders,
        substituters: Arc<Vec<SubstituterCandidate>>,
    ) -> (
        Result<Option<(UpstreamNarInfoData, SubstituterMeta)>, AppError>,
        Vec<ResolveNarInfoEvent>,
    );
}

#[derive(Debug)]
pub(crate) enum QueryOutcome {
    Responded(Option<NarInfoQueryData>),
    Offline,
    Error,
}

// The implementations and this helper must stay panic-free: unlike the racing
// policy, which isolates task panics via `JoinSet`, the tiered policy polls
// its query futures inline within the caller's task, so a panic here would
// tear down the whole NAR info actor.
pub(crate) async fn query_substituter(
    provider: &dyn NarInfoProvider,
    hash: &StorePathHash,
    headers: &PassthroughHeaders,
    substituter: &SubstituterCandidate,
) -> (QueryOutcome, Option<ResolveNarInfoEvent>) {
    let url = hash.on_substituter(substituter.meta());
    let timeout = substituter.meta().nar_info_timeout();
    match provider.query_nar_info(&url, headers, timeout).await {
        Ok(data) => {
            let event = if substituter.is_maybe_ready() {
                Some(ResolveNarInfoEvent::SubstituterSucceeded(
                    substituter.url().clone(),
                ))
            } else {
                None
            };
            (QueryOutcome::Responded(data), event)
        }
        Err(QueryNarInfoError::Offline { .. }) => (
            QueryOutcome::Offline,
            Some(ResolveNarInfoEvent::SubstituterOffline(
                substituter.url().clone(),
            )),
        ),
        Err(QueryNarInfoError::Service { .. }) => (
            QueryOutcome::Error,
            Some(ResolveNarInfoEvent::SubstituterError(
                substituter.url().clone(),
            )),
        ),
    }
}

pub(crate) fn indeterminate_existence_error() -> AppError {
    AppError::infrastructure(
        "could not get results from all substituters to determine whether the nar info exists",
    )
}
