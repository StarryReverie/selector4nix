use std::collections::HashMap;

use tokio::sync::mpsc::Sender as MpscSender;

use crate::domain::substituter::actor::SubstituterMessage;
use crate::domain::substituter::model::Url;

pub struct SubstituterActorRegistry {
    senders: HashMap<Url, MpscSender<SubstituterMessage>>,
}

impl SubstituterActorRegistry {
    pub fn new(senders: HashMap<Url, MpscSender<SubstituterMessage>>) -> Self {
        Self { senders }
    }

    pub fn get(&self, url: &Url) -> Option<&MpscSender<SubstituterMessage>> {
        self.senders.get(url)
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    fn make_url(url: &str) -> Url {
        Url::new(url).unwrap()
    }

    #[test]
    fn get_returns_sender_given_known_url() {
        let url = make_url("https://cache.nixos.org");
        let (tx, _) = mpsc::channel(32);
        let registry = SubstituterActorRegistry::new(HashMap::from([(url.clone(), tx)]));
        assert!(registry.get(&url).is_some());
    }

    #[test]
    fn get_returns_none_given_unknown_url() {
        let url_a = make_url("https://cache-a.example.com");
        let url_b = make_url("https://cache-b.example.com");
        let (tx, _) = mpsc::channel(32);
        let registry = SubstituterActorRegistry::new(HashMap::from([(url_a, tx)]));
        assert!(registry.get(&url_b).is_none());
    }
}
