use std::sync::Arc;
use std::time::Duration;

use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, AnyAddress, Context};
use tokio::time::Instant;

use crate::domain::substituter::index::SubstituterAvailabilityEvent;
use crate::domain::substituter::model::Substituter;
use crate::domain::substituter::service::{SubstituterLifecycleEvent, SubstituterLifecycleService};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubstituterRequest {
    ServiceSuccessful,
    ServiceFailed,
}

pub enum SubstituterInternal {
    NextRetryReady,
}

pub struct SubstituterActor {
    init: Option<Substituter>,
    context: Context<SubstituterRequest, SubstituterInternal>,
    lifecycle_service: Arc<SubstituterLifecycleService>,
    availability_index_pub: AnyAddress<SubstituterAvailabilityEvent>,
}

impl SubstituterActor {
    pub fn new(
        init: Option<Substituter>,
        lifecycle_service: Arc<SubstituterLifecycleService>,
        availability_index_pub: AnyAddress<SubstituterAvailabilityEvent>,
    ) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init,
            context,
            lifecycle_service,
            availability_index_pub,
        })
    }

    async fn exec_all_events(
        &mut self,
        substituter: &Substituter,
        events: Vec<SubstituterLifecycleEvent>,
    ) {
        for event in events {
            self.exec_event(substituter, event).await;
        }
    }

    async fn exec_event(&mut self, substituter: &Substituter, event: SubstituterLifecycleEvent) {
        match event {
            SubstituterLifecycleEvent::ScheduleRetryReady(instant) => {
                self.dispatch_internal(Duration::ZERO, async move {
                    tokio::time::sleep_until(instant).await;
                    SubstituterInternal::NextRetryReady
                });
            }
            SubstituterLifecycleEvent::NotifyUnavailable => {
                let url = substituter.url().clone();
                let prev_failures = substituter.prev_failures();
                tracing::warn!(%url, %prev_failures, "substituter became unavailable");
                let event = SubstituterAvailabilityEvent::BecameUnavailable(url);
                let _ = self.availability_index_pub.tell(event).await;
            }
            SubstituterLifecycleEvent::NotifyAvailable => {
                let substituter = substituter.clone();
                let prev_failures = substituter.prev_failures();
                tracing::debug!(url = %substituter.target().url(), %prev_failures, "assume substituter became available after backoff expired");
                let event = SubstituterAvailabilityEvent::BecameAvailable(substituter);
                let _ = self.availability_index_pub.tell(event).await;
            }
        }
    }
}

impl Actor for SubstituterActor {
    type Request = SubstituterRequest;
    type Internal = SubstituterInternal;
    type State = Substituter;

    fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
        &mut self.context
    }

    async fn on_start(&mut self) -> Option<Self::State> {
        self.init.take()
    }

    async fn on_request(
        &mut self,
        state: Self::State,
        request: Self::Request,
    ) -> Option<Self::State> {
        match request {
            SubstituterRequest::ServiceSuccessful => {
                let (substituter, _events) =
                    self.lifecycle_service.update_on_service_successful(state);
                Some(substituter)
            }
            SubstituterRequest::ServiceFailed => {
                let now = Instant::now();
                let (substituter, events) =
                    self.lifecycle_service.update_on_service_failed(state, now);
                self.exec_all_events(&substituter, events).await;
                Some(substituter)
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
                let (substituter, events) =
                    self.lifecycle_service.update_on_next_retry_ready(state);
                self.exec_all_events(&substituter, events).await;
                Some(substituter)
            }
        }
    }
}
