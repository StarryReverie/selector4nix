use std::sync::Arc;

use selector4nix_actor::actor::{Address, AnyAddress};
use tokio::sync::oneshot;

use crate::domain::nar::actor::{NarActor, NarActorEffect, NarRequest, ResolveNarInfoError};
use crate::domain::nar::index::NarFileEvent;
use crate::domain::nar::model::{Nar, NarInfoData, StorePathHash};
use crate::domain::nar::port::NarInfoProvider;
use crate::domain::substituter::actor::SubstituterRequest;
use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::infrastructure::registry::NarActorRegistry;
use crate::infrastructure::registry::SubstituterActorRegistry;

pub struct NarUseCase {
    nar_registry: Arc<NarActorRegistry>,
    substituter_registry: Arc<SubstituterActorRegistry>,
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
    nar_info_provider: Arc<dyn NarInfoProvider>,
    nar_file_index_pub: AnyAddress<NarFileEvent>,
}

impl NarUseCase {
    pub fn new(
        nar_registry: Arc<NarActorRegistry>,
        substituter_registry: Arc<SubstituterActorRegistry>,
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
        nar_info_provider: Arc<dyn NarInfoProvider>,
        nar_file_index_pub: AnyAddress<NarFileEvent>,
    ) -> Self {
        Self {
            nar_registry,
            substituter_registry,
            substituter_availability_index,
            nar_info_provider,
            nar_file_index_pub,
        }
    }

    pub async fn get_nar_info(
        &self,
        hash: StorePathHash,
    ) -> Result<NarInfoData, ResolveNarInfoError> {
        tracing::info!(hash = hash.value(), "resolving narinfo");

        let address = self.get_nar_actor_sender(hash.clone()).await;

        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = address.tell(NarRequest::ResolveNarInfo(reply_tx)).await;
        let response = reply_rx.await.expect("nar actor shouldn't be dropped");

        match &response.result {
            Ok(data) => tracing::info!(
                hash = hash.value(),
                nar_file = data.nar_file(),
                "narinfo resolved"
            ),
            Err(ResolveNarInfoError::NotFound) => {
                tracing::info!(hash = hash.value(), "narinfo not found")
            }
            Err(ResolveNarInfoError::Fetch) => {
                tracing::warn!(hash = hash.value(), "narinfo fetch failed")
            }
        }

        self.exec_effects(response.effects).await;
        response.result
    }

    async fn exec_effects(&self, effects: Vec<NarActorEffect>) {
        for effect in effects {
            self.exec_effect(effect).await;
        }
    }

    async fn exec_effect(&self, effect: NarActorEffect) {
        match effect {
            NarActorEffect::ReportSubstituterSuccess(url) => {
                if let Some(sender) = self.substituter_registry.get(&url) {
                    let _ = sender.tell(SubstituterRequest::ServiceSuccessful).await;
                }
            }
            NarActorEffect::ReportSubstituterFailure(url) => {
                if let Some(sender) = self.substituter_registry.get(&url) {
                    let _ = sender.tell(SubstituterRequest::ServiceFailed).await;
                }
            }
        }
    }

    async fn get_nar_actor_sender(&self, hash: StorePathHash) -> Address<NarActor> {
        self.nar_registry
            .get_or_create(hash, |hash| {
                NarActor::new(
                    Nar::new(hash.clone()),
                    Arc::clone(&self.substituter_availability_index),
                    Arc::clone(&self.nar_info_provider),
                    self.nar_file_index_pub.clone(),
                )
                .run()
            })
            .await
    }
}
