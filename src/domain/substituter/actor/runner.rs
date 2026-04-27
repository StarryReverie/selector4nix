use std::time::Duration;

use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, Context};
use tokio::sync::mpsc::Sender;
use tokio::time::Instant;

use crate::domain::substituter::actor::{SubstituterActorEffect, SubstituterActorState};
use crate::domain::substituter::index::SubstituterAvailabilityEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubstituterRequest {
    ServiceSuccessful,
    ServiceFailed,
}

pub enum SubstituterInternal {
    NextRetryReady,
}

pub struct SubstituterActor {
    context: Context<SubstituterRequest, SubstituterInternal>,
    availability_index_pub: Sender<SubstituterAvailabilityEvent>,
}

impl SubstituterActor {
    pub fn new(availability_index_pub: Sender<SubstituterAvailabilityEvent>) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            context,
            availability_index_pub,
        })
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
                self.dispatch_internal(Duration::ZERO, async move {
                    tokio::time::sleep_until(instant).await;
                    SubstituterInternal::NextRetryReady
                });
            }
            SubstituterActorEffect::NotifyUnavailable => {
                let url = state.inner().url().clone();
                let prev_failures = state.inner().prev_failures();
                tracing::warn!(%url, %prev_failures, "substituter became unavailable");
                let event = SubstituterAvailabilityEvent::BecameUnavailable(url);
                let _ = self.availability_index_pub.send(event).await;
            }
            SubstituterActorEffect::NotifyAvailable => {
                let meta = state.inner().target().clone();
                let prev_failures = state.inner().prev_failures();
                tracing::info!(url = %meta.url(), %prev_failures, "assume substituter became available after backoff expired");
                let event = SubstituterAvailabilityEvent::BecameAvailable(meta);
                let _ = self.availability_index_pub.send(event).await;
            }
        }
    }
}

impl Actor for SubstituterActor {
    type Request = SubstituterRequest;
    type Internal = SubstituterInternal;
    type State = SubstituterActorState;

    fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
        &mut self.context
    }

    async fn on_request(
        &mut self,
        state: Self::State,
        request: Self::Request,
    ) -> Option<Self::State> {
        match request {
            SubstituterRequest::ServiceSuccessful => {
                Some(SubstituterActorState::on_service_successful(state))
            }
            SubstituterRequest::ServiceFailed => {
                let now = Instant::now();
                let (effects, state) = SubstituterActorState::on_service_failed(state, now);
                self.exec_all_effects(&state, effects).await;
                Some(state)
            }
        }
    }

    async fn on_internal(
        &mut self,
        state: Self::State,
        internal: Self::Internal,
    ) -> Option<Self::State> {
        match internal {
            SubstituterInternal::NextRetryReady => {
                let (effects, state) = SubstituterActorState::on_next_retry_ready(state);
                self.exec_all_effects(&state, effects).await;
                Some(state)
            }
        }
    }
}
