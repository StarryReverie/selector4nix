use std::sync::Arc;

use tokio::sync::mpsc::{self, Sender};
use tokio::sync::oneshot;

use crate::domain::nar::actor::{NarActor, NarActorEffect, NarMessage, ResolveNarInfoError};
use crate::domain::nar::model::{Nar, NarInfoData, StorePathHash};
use crate::domain::nar::port::DynNarInfoProvider;
use crate::domain::substituter::actor::SubstituterMessage;
use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::infrastructure::registry::NarActorRegistry;
use crate::infrastructure::registry::SubstituterActorRegistry;

pub struct NarUseCase {
    nar_registry: Arc<NarActorRegistry>,
    substituter_registry: Arc<SubstituterActorRegistry>,
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
    nar_info_provider: Arc<DynNarInfoProvider<'static>>,
}

impl NarUseCase {
    pub fn new(
        nar_registry: Arc<NarActorRegistry>,
        substituter_registry: Arc<SubstituterActorRegistry>,
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
        nar_info_provider: Arc<DynNarInfoProvider<'static>>,
    ) -> Self {
        Self {
            nar_registry,
            substituter_registry,
            substituter_availability_index,
            nar_info_provider,
        }
    }

    pub async fn get_nar_info(
        &self,
        hash: StorePathHash,
    ) -> Result<NarInfoData, ResolveNarInfoError> {
        let sender = self.get_nar_actor_sender(hash).await;

        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = sender.send(NarMessage::ResolveNarInfo(reply_tx)).await;
        let response = reply_rx.await.expect("nar actor shouldn't be dropped");

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
                    let _ = sender.send(SubstituterMessage::ServiceSuccessful).await;
                }
            }
            NarActorEffect::ReportSubstituterFailure(url) => {
                if let Some(sender) = self.substituter_registry.get(&url) {
                    let _ = sender.send(SubstituterMessage::ServiceFailed).await;
                }
            }
        }
    }

    async fn get_nar_actor_sender(&self, hash: StorePathHash) -> Sender<NarMessage> {
        self.nar_registry
            .get_or_create(hash, |hash| {
                let (messages_tx, messages) = mpsc::channel(32);
                let actor = NarActor::new(
                    messages,
                    Arc::clone(&self.substituter_availability_index),
                    Arc::clone(&self.nar_info_provider),
                );
                actor.run(Nar::new(hash.clone()));
                messages_tx
            })
            .await
    }
}
