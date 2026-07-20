use std::sync::Arc;

use arc_swap::ArcSwap;
use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::Semaphore;

use crate::domain::common::url::Url;
use crate::domain::substituter::model::Substituter;
use crate::domain::substituter::{SubstituterCandidate, SubstituterRepository};

pub struct InMemorySubstituterRepository {
    substituters: DashMap<Url, Substituter>,
    available_substituters: ArcSwap<Vec<SubstituterCandidate>>,
    write_permit: Semaphore,
}

impl InMemorySubstituterRepository {
    pub fn new() -> Self {
        Self {
            substituters: DashMap::new(),
            available_substituters: ArcSwap::new(Arc::new(Vec::new())),
            write_permit: Semaphore::new(1),
        }
    }
}

#[async_trait]
impl SubstituterRepository for InMemorySubstituterRepository {
    async fn get(&self, url: &Url) -> Option<Substituter> {
        self.substituters.get(url).map(|s| s.clone())
    }

    async fn query_all(&self) -> Vec<Substituter> {
        self.substituters
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    async fn query_all_available(&self) -> Arc<Vec<SubstituterCandidate>> {
        self.available_substituters.load_full()
    }

    async fn exists_available(&self, url: &Url) -> bool {
        self.substituters
            .get(url)
            .is_some_and(|s| !s.is_unavailable())
    }

    async fn save(&self, substituter: Substituter) {
        let _permit = self.write_permit.acquire().await;

        let substituter = self
            .substituters
            .entry(substituter.url().clone())
            .insert(substituter)
            .downgrade();

        if substituter.is_unavailable() {
            let avail = self.available_substituters.load_full();
            if let Some(index) = avail.iter().position(|s| s.url() == substituter.url()) {
                let mut avail = (*avail).clone();
                avail.swap_remove(index);
                self.available_substituters.store(Arc::new(avail));
            }
        } else {
            let avail = self.available_substituters.load_full();
            let candidate = SubstituterCandidate::from(&*substituter);
            if let Some(index) = avail.iter().position(|s| s.url() == substituter.url()) {
                let mut avail = (*avail).clone();
                avail[index] = candidate;
                self.available_substituters.store(Arc::new(avail));
            } else {
                let mut avail = (*avail).clone();
                avail.push(candidate);
                self.available_substituters.store(Arc::new(avail));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::common::url::Url;
    use crate::domain::substituter::SubstituterRepository;
    use crate::domain::substituter::model::test_support::{
        make_substituter_maybe_ready_with_url, make_substituter_normal_with_url,
        make_substituter_offline_with_url,
    };

    use super::*;

    #[tokio::test]
    async fn query_all_returns_saved_substituters() {
        let repo = InMemorySubstituterRepository::new();
        repo.save(make_substituter_normal_with_url(
            &Url::new("https://a.example.com").unwrap(),
        ))
        .await;
        repo.save(make_substituter_offline_with_url(
            &Url::new("https://b.example.com").unwrap(),
        ))
        .await;

        let all = repo.query_all().await;
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn query_all_available_returns_empty_initially() {
        let repo = InMemorySubstituterRepository::new();
        let result = repo.query_all_available().await;
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn exists_available_returns_true() {
        let repo = InMemorySubstituterRepository::new();
        repo.save(make_substituter_normal_with_url(
            &Url::new("https://a.example.com").unwrap(),
        ))
        .await;

        let res = repo
            .exists_available(&Url::new("https://a.example.com").unwrap())
            .await;
        assert!(res);
    }

    #[tokio::test]
    async fn exists_available_returns_false_if_unavailable() {
        let repo = InMemorySubstituterRepository::new();
        repo.save(make_substituter_offline_with_url(
            &Url::new("https://a.example.com").unwrap(),
        ))
        .await;

        let res = repo
            .exists_available(&Url::new("https://a.example.com").unwrap())
            .await;
        assert!(!res);
    }

    #[tokio::test]
    async fn exists_available_returns_false_if_does_not_exist() {
        let repo = InMemorySubstituterRepository::new();

        let res = repo
            .exists_available(&Url::new("https://a.example.com").unwrap())
            .await;
        assert!(!res);
    }

    #[tokio::test]
    async fn save_available_inserts_into_snapshot() {
        let repo = InMemorySubstituterRepository::new();
        repo.save(make_substituter_normal_with_url(
            &Url::new("https://a.example.com").unwrap(),
        ))
        .await;

        let avail = repo.query_all_available().await;
        assert_eq!(avail.len(), 1);
        assert_eq!(avail[0].url(), &Url::new("https://a.example.com").unwrap());
        assert!(!avail[0].is_maybe_ready());
    }

    #[tokio::test]
    async fn save_unavailable_removes_from_snapshot() {
        let repo = InMemorySubstituterRepository::new();
        let url = Url::new("https://a.example.com").unwrap();
        repo.save(make_substituter_normal_with_url(&url)).await;
        repo.save(make_substituter_offline_with_url(&url)).await;

        let avail = repo.query_all_available().await;
        assert!(avail.is_empty());
        assert!(repo.get(&url).await.is_some());
    }

    #[tokio::test]
    async fn save_available_updates_is_maybe_ready() {
        let repo = InMemorySubstituterRepository::new();
        let url = Url::new("https://a.example.com").unwrap();
        repo.save(make_substituter_maybe_ready_with_url(&url)).await;
        repo.save(make_substituter_normal_with_url(&url)).await;

        let avail = repo.query_all_available().await;
        assert_eq!(avail.len(), 1);
        assert!(!avail[0].is_maybe_ready());
    }

    #[tokio::test]
    async fn save_multiple_available() {
        let repo = InMemorySubstituterRepository::new();
        repo.save(make_substituter_normal_with_url(
            &Url::new("https://a.example.com").unwrap(),
        ))
        .await;
        repo.save(make_substituter_maybe_ready_with_url(
            &Url::new("https://b.example.com").unwrap(),
        ))
        .await;

        let avail = repo.query_all_available().await;
        assert_eq!(avail.len(), 2);
    }
}
