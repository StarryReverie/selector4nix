use std::sync::Arc;

use selector4nix_actor::actor::{Actor, ActorPre, ActorPreBuilder, Context, EmptyInternal};
use tokio::sync::watch::{self, Sender as WatchSender};

use crate::domain::substituter::index::SubstituterAvailabilityEvent;
use crate::domain::substituter::model::SubstituterMeta;
use crate::infrastructure::index::substituter_availability::SubstituterAvailabilityIndexView;

pub struct SubstituterAvailabilityIndexActor {
    context: Context<SubstituterAvailabilityEvent, EmptyInternal>,
    snapshot_tx: WatchSender<Arc<Vec<SubstituterMeta>>>,
}

impl SubstituterAvailabilityIndexActor {
    pub fn new(
        initial: Vec<SubstituterMeta>,
    ) -> (ActorPre<Self>, SubstituterAvailabilityIndexView) {
        let (snapshot_tx, snapshot_rx) = watch::channel(Arc::new(initial));
        let view = SubstituterAvailabilityIndexView::new(snapshot_rx);
        let pre = ActorPreBuilder::inject(|context| Self {
            context,
            snapshot_tx,
        });
        (pre, view)
    }

    fn apply_event(state: &mut Vec<SubstituterMeta>, event: SubstituterAvailabilityEvent) {
        match event {
            SubstituterAvailabilityEvent::BecameAvailable(meta) => {
                if !state.iter().any(|m| m.url() == meta.url()) {
                    state.push(meta);
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
    type State = Vec<SubstituterMeta>;

    fn context(&mut self) -> &mut Context<Self::Request, Self::Internal> {
        &mut self.context
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
    use crate::domain::substituter::model::{Priority, Url};

    use super::*;

    fn make_meta(url: &str, priority: u32) -> SubstituterMeta {
        SubstituterMeta::new(Url::new(url).unwrap(), Priority::new(priority).unwrap())
    }

    #[test]
    fn apply_event_adds_entry_given_became_available() {
        let meta = make_meta("https://cache.nixos.org", 40);
        let mut state = vec![];
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameAvailable(meta.clone()),
        );
        assert_eq!(state, vec![meta]);
    }

    #[test]
    fn apply_event_is_idempotent_given_duplicate_became_available() {
        let meta = make_meta("https://cache.nixos.org", 40);
        let mut state = vec![];
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameAvailable(meta.clone()),
        );
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameAvailable(meta.clone()),
        );
        assert_eq!(state, vec![meta]);
    }

    #[test]
    fn apply_event_removes_entry_given_became_unavailable() {
        let meta_a = make_meta("https://cache-a.example.com", 40);
        let meta_b = make_meta("https://cache-b.example.com", 50);
        let mut state = vec![meta_a.clone(), meta_b.clone()];
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameUnavailable(meta_a.url().clone()),
        );
        assert_eq!(state, vec![meta_b]);
    }

    #[test]
    fn apply_event_is_noop_given_unknown_became_unavailable() {
        let meta = make_meta("https://cache.nixos.org", 40);
        let mut state = vec![meta.clone()];
        let other_url = Url::new("https://other.example.com").unwrap();
        SubstituterAvailabilityIndexActor::apply_event(
            &mut state,
            SubstituterAvailabilityEvent::BecameUnavailable(other_url),
        );
        assert_eq!(state, vec![meta]);
    }
}
