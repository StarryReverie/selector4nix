use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::common::url::Url;
use selector4nix_core::domain::nar_file::NarFileService;
use selector4nix_core::domain::nar_file::model::NarFile;
use selector4nix_core::domain::substituter::SubstituterRepository;
use selector4nix_core::domain::substituter::model::Substituter;
use selector4nix_core::domain::substituter::model::test_support::{
    make_substituter_normal_with_url_pri, make_substituter_offline_with_url_pri,
};
use selector4nix_core::infrastructure::repository::InMemorySubstituterRepository;

use crate::fixture::nar_file::{
    make_nar_file_location, make_nar_file_location_with_substituter_meta,
    make_nar_file_with_location, make_source_url, make_source_url_with_substituter_meta,
};
use crate::fixture::substituter::make_substituter_meta_with_storage_url;
use crate::mock::nar_stream_provider::MockNarStreamProvider;

#[derive(Debug)]
struct TestCaseEnvironment {
    substituters: Vec<Substituter>,
    success_urls: HashSet<Url>,
}

#[derive(Debug)]
struct TestCaseInput {
    nar_file: NarFile,
}

#[derive(Debug)]
struct TestCaseExpectation {
    result_source_url: Result<Url, ()>,
    used_substituter_url: Option<Url>,
    not_contacted_source_urls: Vec<Url>,
}

async fn run_test(
    env: TestCaseEnvironment,
    input: TestCaseInput,
    expectation: TestCaseExpectation,
) {
    let repo = Arc::new(InMemorySubstituterRepository::new());
    for sub in env.substituters {
        repo.save(sub).await;
    }

    let provider = Arc::new(MockNarStreamProvider::new(env.success_urls));
    let service = NarFileService::new(provider.clone(), repo, Duration::from_secs(14400));

    let (nar_file, result, _events) = service
        .stream(
            input.nar_file,
            None,
            PassthroughHeaders::empty(),
            SystemTime::now(),
        )
        .await;

    assert_eq!(
        result.map(|data| data.source_url).map_err(|_| ()),
        expectation.result_source_url,
    );

    assert_eq!(
        nar_file
            .location()
            .map(|location| location.substituter().url().clone()),
        expectation.used_substituter_url,
    );

    for forbidden in &expectation.not_contacted_source_urls {
        assert!(!provider.has_contacted_url(forbidden));
    }
}

#[tokio::test]
async fn cached_substituter_unavailable_falls_back_early() {
    let a_url = Url::new("https://cache-a.example.com").unwrap();
    let b_url = Url::new("https://cache-b.example.com").unwrap();
    let a_src = make_source_url(&a_url, 40);
    let b_src = make_source_url(&b_url, 10);

    let nar_file = make_nar_file_with_location(make_nar_file_location(&a_url, 40));

    run_test(
        TestCaseEnvironment {
            substituters: vec![
                make_substituter_offline_with_url_pri(&a_url, 40),
                make_substituter_normal_with_url_pri(&b_url, 10),
            ],
            success_urls: HashSet::from([b_src.clone()]),
        },
        TestCaseInput { nar_file },
        TestCaseExpectation {
            result_source_url: Ok(b_src),
            used_substituter_url: Some(b_url),
            not_contacted_source_urls: vec![a_src],
        },
    )
    .await;
}

#[tokio::test]
async fn cached_substituter_available_serves_from_cache() {
    let a_url = Url::new("https://cache-a.example.com").unwrap();
    let a_src = make_source_url(&a_url, 40);

    let nar_file = make_nar_file_with_location(make_nar_file_location(&a_url, 40));

    run_test(
        TestCaseEnvironment {
            substituters: vec![make_substituter_normal_with_url_pri(&a_url, 40)],
            success_urls: HashSet::from([a_src.clone()]),
        },
        TestCaseInput { nar_file },
        TestCaseExpectation {
            result_source_url: Ok(a_src),
            used_substituter_url: Some(a_url),
            not_contacted_source_urls: vec![],
        },
    )
    .await;
}

#[tokio::test]
async fn offline_substituter_with_separate_storage_still_served_from_cache() {
    let a_url = Url::new("https://cache-a.example.com").unwrap();
    let a_storage = Url::new("https://storage-a.example.com/nar").unwrap();
    let meta = make_substituter_meta_with_storage_url(&a_url, a_storage, 40);
    let a_src = make_source_url_with_substituter_meta(&meta);

    let nar_file = make_nar_file_with_location(make_nar_file_location_with_substituter_meta(&meta));

    run_test(
        TestCaseEnvironment {
            substituters: vec![make_substituter_offline_with_url_pri(&a_url, 40)],
            success_urls: HashSet::from([a_src.clone()]),
        },
        TestCaseInput { nar_file },
        TestCaseExpectation {
            result_source_url: Ok(a_src),
            used_substituter_url: Some(a_url),
            not_contacted_source_urls: vec![],
        },
    )
    .await;
}

#[tokio::test]
async fn cached_attempt_fails_falls_back() {
    let a_url = Url::new("https://cache-a.example.com").unwrap();
    let b_url = Url::new("https://cache-b.example.com").unwrap();
    let b_src = make_source_url(&b_url, 10);

    let nar_file = make_nar_file_with_location(make_nar_file_location(&a_url, 40));

    run_test(
        TestCaseEnvironment {
            substituters: vec![
                make_substituter_normal_with_url_pri(&a_url, 40),
                make_substituter_normal_with_url_pri(&b_url, 10),
            ],
            success_urls: HashSet::from([b_src.clone()]),
        },
        TestCaseInput { nar_file },
        TestCaseExpectation {
            result_source_url: Ok(b_src),
            used_substituter_url: Some(b_url),
            not_contacted_source_urls: vec![],
        },
    )
    .await;
}
