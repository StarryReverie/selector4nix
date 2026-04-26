use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time::Instant;

use crate::domain::substituter::actor::{SubstituterActorEffect, SubstituterActorState};
use crate::domain::substituter::index::SubstituterAvailabilityEvent;
use crate::domain::substituter::model::Substituter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubstituterMessage {
    ServiceSuccessful,
    ServiceFailed,
}

enum SubstituterInternalMessage {
    NextRetryReady,
}

pub struct SubstituterActor {
    messages: Receiver<SubstituterMessage>,
    internal: Receiver<SubstituterInternalMessage>,
    internal_tx: Sender<SubstituterInternalMessage>,
    availability_index_pub: Sender<SubstituterAvailabilityEvent>,
}

impl SubstituterActor {
    pub fn new(
        messages: Receiver<SubstituterMessage>,
        availability_index_pub: Sender<SubstituterAvailabilityEvent>,
    ) -> Self {
        let (internal_tx, internal) = mpsc::channel(32);
        Self {
            messages,
            internal,
            internal_tx,
            availability_index_pub,
        }
    }

    pub async fn run(mut self, state: Substituter) {
        tokio::spawn(async move {
            let mut state = SubstituterActorState::new(state);
            loop {
                tokio::select! {
                    Some(message) = self.messages.recv() => state = self.handle_message(state, message).await,
                    Some(message) = self.internal.recv() => state = self.handle_internal(state, message).await,
                }
            }
        });
    }

    async fn handle_message(
        &mut self,
        state: SubstituterActorState,
        message: SubstituterMessage,
    ) -> SubstituterActorState {
        match message {
            SubstituterMessage::ServiceSuccessful => {
                SubstituterActorState::on_service_successful(state)
            }
            SubstituterMessage::ServiceFailed => {
                let now = Instant::now();
                let (effects, state) = SubstituterActorState::on_service_failed(state, now);
                self.exec_all_effects(&state, effects).await;
                state
            }
        }
    }

    async fn handle_internal(
        &mut self,
        state: SubstituterActorState,
        message: SubstituterInternalMessage,
    ) -> SubstituterActorState {
        match message {
            SubstituterInternalMessage::NextRetryReady => {
                let (effects, state) = SubstituterActorState::on_next_retry_ready(state);
                self.exec_all_effects(&state, effects).await;
                state
            }
        }
    }

    async fn exec_all_effects(
        &mut self,
        state: &SubstituterActorState,
        effects: Vec<SubstituterActorEffect>,
    ) {
        for effect in effects {
            self.exec_effect(state, effect).await;
        }
    }

    async fn exec_effect(&mut self, state: &SubstituterActorState, effect: SubstituterActorEffect) {
        match effect {
            SubstituterActorEffect::ScheduleRetryReady(instant) => {
                let internal_tx = self.internal_tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep_until(instant).await;
                    let _ = internal_tx
                        .send(SubstituterInternalMessage::NextRetryReady)
                        .await;
                });
            }
            SubstituterActorEffect::NotifyUnavailable => {
                let url = state.inner().url().clone();
                let event = SubstituterAvailabilityEvent::BecameUnavailable(url);
                let _ = self.availability_index_pub.send(event).await;
            }
            SubstituterActorEffect::NotifyAvailable => {
                let meta = state.inner().target().clone();
                let event = SubstituterAvailabilityEvent::BecameAvailable(meta);
                let _ = self.availability_index_pub.send(event).await;
            }
        }
    }
}
