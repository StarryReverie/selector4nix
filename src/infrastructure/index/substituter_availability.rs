use std::sync::Arc;

use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, Context, EmptyInternal};
use tokio::sync::watch::{self, Receiver, Sender as WatchSender};

use crate::domain::substituter::index::{
    SubstituterAvailabilityEvent, SubstituterAvailabilityIndex, SubstituterCandidate,
};

#[derive(Clone)]
pub struct SubstituterAvailabilityIndexView {
    snapshot: Receiver<Arc<Vec<SubstituterCandidate>>>,
}

impl SubstituterAvailabilityIndexView {
    pub fn new(snapshot: Receiver<Arc<Vec<SubstituterCandidate>>>) -> Self {
        Self { snapshot }
    }
}

impl SubstituterAvailabilityIndex for SubstituterAvailabilityIndexView {
    fn query_all(&self) -> Arc<Vec<SubstituterCandidate>> {
        Arc::clone(&self.snapshot.borrow())
    }
}

pub struct SubstituterAvailabilityIndexActor {
    init: Option<Vec<SubstituterCandidate>>,
    context: Context<SubstituterAvailabilityEvent, EmptyInternal>,
    snapshot_tx: WatchSender<Arc<Vec<SubstituterCandidate>>>,
}

impl SubstituterAvailabilityIndexActor {
    pub fn new(
        init: Vec<SubstituterCandidate>,
    ) -> (ActorPre<Self>, SubstituterAvailabilityIndexView) {
        let (snapshot_tx, snapshot_rx) = watch::channel(Arc::new(init.clone()));
        let view = SubstituterAvailabilityIndexView::new(snapshot_rx);
        let pre = ActorPreBuilder::inject(|context| Self {
            init: Some(init),
            context,
            snapshot_tx,
        });
        (pre, view)
    }

    fn apply_event(state: &mut Vec<SubstituterCandidate>, event: SubstituterAvailabilityEvent) {
        match event {
            SubstituterAvailabilityEvent::BecameAvailable(substituter) => {
                if !state.iter().any(|s| s.url() == substituter.url()) {
                    state.push(substituter);
                }
            }
            SubstituterAvailabilityEvent::BecameUnavailable(url) => {
                state.retain(|m| m.url() != &url);
            }
        }
    }
}

impl Actor for SubstituterAvailabilityIndexActor {
    type Request = SubstituterAvailabilityEvent;
    type Internal = EmptyInternal;
    type State = Vec<SubstituterCandidate>;

    fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
        &mut self.context
    }

    async fn on_start(&mut self) -> Option<Self::State> {
        self.init.take()
    }

    async fn on_request(
        &mut self,
        mut state: Self::State,
        event: Self::Request,
    ) -> Option<Self::State> {
        Self::apply_event(&mut state, event);
        let _ = self.snapshot_tx.send(Arc::new(state.clone()));
        Some(state)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::substituter::model::{Priority, SubstituterMeta, Url};

    use super::*;

    fn make_substituter_candidate(url: &str, priority: u32) -> SubstituterCandidate {
        SubstituterCandidate::new(
            SubstituterMeta::new(Url::new(url).unwrap(), Priority::new(priority).unwrap()),
            false,
        )
    }

    #[test]
    fn apply_event_adds_entry_given_became_available() {
        let sub = make_substituter_candidate("https://cache.nixos.org", 40);
        let mut state = vec![];
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameAvailable(sub.clone()),
        );
        assert_eq!(state, vec![sub]);
    }

    #[test]
    fn apply_event_is_idempotent_given_duplicate_became_available() {
        let sub = make_substituter_candidate("https://cache.nixos.org", 40);
        let mut state = vec![];
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameAvailable(sub.clone()),
        );
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameAvailable(sub.clone()),
        );
        assert_eq!(state, vec![sub]);
    }

    #[test]
    fn apply_event_removes_entry_given_became_unavailable() {
        let sub_a = make_substituter_candidate("https://cache-a.example.com", 40);
        let sub_b = make_substituter_candidate("https://cache-b.example.com", 50);
        let mut state = vec![sub_a.clone(), sub_b.clone()];
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameUnavailable(sub_a.url().clone()),
        );
        assert_eq!(state, vec![sub_b]);
    }

    #[test]
    fn apply_event_is_noop_given_unknown_became_unavailable() {
        let sub = make_substituter_candidate("https://cache.nixos.org", 40);
        let mut state = vec![sub.clone()];
        let other_url = Url::new("https://other.example.com").unwrap();
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameUnavailable(other_url),
        );
        assert_eq!(state, vec![sub]);
    }
}
