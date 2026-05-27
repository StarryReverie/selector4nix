use std::sync::Arc;
use std::time::Duration;

use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, AnyAddress, Context};
use tokio::time::Instant;

use crate::domain::substituter::SubstituterService;
use crate::domain::substituter::index::{SubstituterAvailabilityEvent, SubstituterCandidate};
use crate::domain::substituter::model::{Substituter, UpdateSubstituterEvent};
use crate::domain::substituter::port::{ProbeSubstituterError, SubstituterProbingProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubstituterRequest {
    ServiceSuccessful,
    ServiceOffline,
    ServiceError,
}

pub enum SubstituterInternal {
    NextRetryReady,
    ProbingFinished(Result<(), ProbeSubstituterError>),
}

pub struct SubstituterActor {
    init: Option<Substituter>,
    context: Context<SubstituterRequest, SubstituterInternal>,
    substituter_service: Arc<SubstituterService>,
    substituter_probing_provider: Arc<dyn SubstituterProbingProvider>,
    availability_index_pub: AnyAddress<SubstituterAvailabilityEvent>,
}

impl SubstituterActor {
    pub fn new(
        init: Option<Substituter>,
        substituter_service: Arc<SubstituterService>,
        substituter_probing_provider: Arc<dyn SubstituterProbingProvider>,
        availability_index_pub: AnyAddress<SubstituterAvailabilityEvent>,
    ) -> ActorPre<Self> {
        ActorPreBuilder::inject(|context| Self {
            init,
            context,
            substituter_service,
            substituter_probing_provider,
            availability_index_pub,
        })
    }

    async fn exec_all_events(
        &mut self,
        substituter: &Substituter,
        events: Vec<UpdateSubstituterEvent>,
    ) {
        for event in events {
            self.exec_event(substituter, event).await;
        }
    }

    async fn exec_event(&mut self, substituter: &Substituter, event: UpdateSubstituterEvent) {
        match event {
            UpdateSubstituterEvent::ScheduleRetryReady(instant) => {
                self.dispatch_internal(Duration::ZERO, async move {
                    tokio::time::sleep_until(instant).await;
                    SubstituterInternal::NextRetryReady
                });
            }
            UpdateSubstituterEvent::ScheduleProbing(instant) => {
                let substituter = substituter.target().clone();
                let provider = Arc::clone(&self.substituter_probing_provider);
                self.dispatch_internal(Duration::ZERO, async move {
                    if let Some(instant) = instant {
                        tokio::time::sleep_until(instant).await;
                    }
                    let res = provider.probe_substituter(&substituter).await;
                    SubstituterInternal::ProbingFinished(res)
                });
            }
            UpdateSubstituterEvent::NotifyUnavailable => {
                let url = substituter.url().clone();
                let prev_failures = substituter.prev_failures();
                tracing::warn!(%url, %prev_failures, "substituter became unavailable");
                let event = SubstituterAvailabilityEvent::BecameUnavailable(url);
                let _ = self.availability_index_pub.tell(event).await;
            }
            UpdateSubstituterEvent::NotifyAvailable => {
                let substituter = SubstituterCandidate::from(substituter);
                tracing::debug!(url = %substituter.url(), "substituter became or stayed available after probing");
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
        match self.init.take() {
            Some(init) => {
                let now = Instant::now();
                let events = self.substituter_service.on_initial(now);
                self.exec_all_events(&init, events).await;
                Some(init)
            }
            None => None,
        }
    }

    async fn on_request(
        &mut self,
        substituter: Self::State,
        request: Self::Request,
    ) -> Option<Self::State> {
        match request {
            SubstituterRequest::ServiceSuccessful => {
                let (substituter, events) = substituter.update_on_service_successful();
                self.exec_all_events(&substituter, events).await;
                Some(substituter)
            }
            SubstituterRequest::ServiceOffline => {
                let now = Instant::now();
                let (substituter, events) = substituter.update_on_service_offline(now);
                self.exec_all_events(&substituter, events).await;
                Some(substituter)
            }
            SubstituterRequest::ServiceError => {
                let now = Instant::now();
                let (substituter, events) = substituter.update_on_service_error(now);
                self.exec_all_events(&substituter, events).await;
                Some(substituter)
            }
        }
    }

    async fn on_internal(
        &mut self,
        substituter: Self::State,
        internal: Self::Internal,
    ) -> Option<Self::State> {
        match internal {
            SubstituterInternal::NextRetryReady => {
                let (substituter, events) = substituter.update_on_next_retry_ready();
                self.exec_all_events(&substituter, events).await;
                Some(substituter)
            }
            SubstituterInternal::ProbingFinished(res) => {
                let now = Instant::now();
                let (substituter, events) =
                    self.substituter_service
                        .update_on_probing_finished(substituter, res, now);
                self.exec_all_events(&substituter, events).await;
                Some(substituter)
            }
        }
    }
}
