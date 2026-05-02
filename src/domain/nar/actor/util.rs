#![expect(unused)]

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

use tokio::time::Instant;

pub struct DeadlineGroup<K> {
    deadlines: HashMap<K, Instant>,
}

impl<K> DeadlineGroup<K> {
    pub fn new() -> Self {
        Self {
            deadlines: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.deadlines.is_empty()
    }
}

impl<K> DeadlineGroup<K>
where
    K: Eq + Hash,
{
    pub fn insert_or_set_earlier(&mut self, key: K, deadline: Instant) {
        self.deadlines
            .entry(key)
            .and_modify(|current| {
                *current = deadline.min(*current);
            })
            .or_insert(deadline);
    }

    pub async fn wait_earliest(&self) -> Option<K>
    where
        K: Clone,
    {
        let entry = self
            .deadlines
            .iter()
            .min_by(|e1, e2| e1.1.cmp(e2.1))
            .map(|(key, earliest)| (key.clone(), *earliest));
        match entry {
            Some((key, earliest)) => {
                tokio::time::sleep_until(earliest).await;
                Some(key)
            }
            None => None,
        }
    }

    pub fn remove<Q>(&mut self, key: &Q)
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.deadlines.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn wait_returns_earliest_key() {
        let mut group = DeadlineGroup::new();
        let now = Instant::now();
        group.insert_or_set_earlier("a", now + Duration::from_secs(5));
        group.insert_or_set_earlier("b", now + Duration::from_secs(2));
        group.insert_or_set_earlier("c", now + Duration::from_secs(8));

        tokio::time::advance(Duration::from_secs(2)).await;
        let key = group.wait_earliest().await.unwrap();
        assert_eq!(key, "b");
    }

    #[tokio::test(start_paused = true)]
    async fn insert_or_set_earlier_updates_given_earlier() {
        let mut group = DeadlineGroup::new();
        let now = Instant::now();
        group.insert_or_set_earlier("a", now + Duration::from_secs(10));
        group.insert_or_set_earlier("a", now + Duration::from_secs(3));

        tokio::time::advance(Duration::from_secs(3)).await;
        let key = group.wait_earliest().await.unwrap();
        assert_eq!(key, "a");
    }

    #[tokio::test(start_paused = true)]
    async fn insert_or_set_earlier_ignores_given_later() {
        let mut group = DeadlineGroup::new();
        let now = Instant::now();
        group.insert_or_set_earlier("a", now + Duration::from_secs(3));
        group.insert_or_set_earlier("a", now + Duration::from_secs(10));

        tokio::time::advance(Duration::from_secs(3)).await;
        let key = group.wait_earliest().await.unwrap();
        assert_eq!(key, "a");
    }
}
