use std::sync::Arc;

use tokio::sync::mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender};
use tokio::sync::watch::{self, Sender as WatchSender};

use crate::domain::substituter::index::SubstituterAvailabilityEvent;
use crate::domain::substituter::model::SubstituterMeta;
use crate::infrastructure::index::substituter_availability::SubstituterAvailabilityIndexView;

pub struct SubstituterAvailabilityIndexActor {
    events_tx: MpscSender<SubstituterAvailabilityEvent>,
    events: MpscReceiver<SubstituterAvailabilityEvent>,
    snapshot_tx: WatchSender<Arc<Vec<SubstituterMeta>>>,
}

impl SubstituterAvailabilityIndexActor {
    pub fn new() -> Self {
        let (events_tx, events) = mpsc::channel(32);
        let (snapshot_tx, _) = watch::channel(Arc::new(Vec::new()));
        Self {
            events_tx,
            events,
            snapshot_tx,
        }
    }

    pub fn publisher(&self) -> MpscSender<SubstituterAvailabilityEvent> {
        self.events_tx.clone()
    }

    pub fn view(&self) -> SubstituterAvailabilityIndexView {
        SubstituterAvailabilityIndexView::new(self.snapshot_tx.subscribe())
    }

    pub fn run(mut self, initial: Vec<SubstituterMeta>) {
        let mut state = initial;
        let _ = self.snapshot_tx.send(Arc::new(state.clone()));

        tokio::spawn(async move {
            while let Some(event) = self.events.recv().await {
                Self::apply_event(&mut state, event);
                let _ = self.snapshot_tx.send(Arc::new(state.clone()));
            }
        });
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
