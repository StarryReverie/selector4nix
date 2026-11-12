use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::common::url::Url;
use selector4nix_core::domain::nar_info::model::test_support::{
    make_nar_file_name, make_store_path_hash,
};
use selector4nix_core::domain::nar_info::model::{NarUrlRewriteOption, StorePathHash};
use selector4nix_core::domain::nar_info::policy::PriorityPolicy;
use selector4nix_core::domain::nar_info::port::NarInfoQueryData;
use selector4nix_core::domain::nar_info::{NarInfoService, ResolveNarInfoEvent};
use selector4nix_core::domain::substituter::SubstituterRepository;
use selector4nix_core::domain::substituter::model::Substituter;
use selector4nix_core::domain::substituter::model::test_support::{
    make_substituter_meta_with_url_pri, make_substituter_normal_with_url_pri,
};
use selector4nix_core::infrastructure::repository::InMemorySubstituterRepository;

use crate::fixture::nar_file::make_source_url;
use crate::fixture::nar_info::{make_nar_info_query_data, make_nar_info_url};
use crate::fixture::substituter::make_substituter_normal_with_nar_info_timeout;
use crate::mock::nar_info_provider::MockNarInfoProvider;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TestCaseEnvironment {
    substituters: Vec<Substituter>,
    nar_info_entries: Vec<(Url, Result<NarInfoQueryData, String>)>,
    ignore_query_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TestCaseInput {
    hash: StorePathHash,
}

#[derive(Debug)]
struct TestCaseExpectation {
    source_url: Result<Option<Url>, ()>,
    events: Vec<ResolveNarInfoEvent>,
    queried: Vec<Url>,
}

fn make_nar_file_located_event(substituter_url: &Url, priority: u32) -> ResolveNarInfoEvent {
    ResolveNarInfoEvent::NarFileLocated {
        nar_file: make_nar_file_name(),
        substituter: make_substituter_meta_with_url_pri(substituter_url, priority),
        source_url: make_source_url(substituter_url, priority),
        store_path_hash: make_store_path_hash(),
    }
}

async fn run_test(
    env: TestCaseEnvironment,
    input: TestCaseInput,
    expectation: TestCaseExpectation,
) {
    let _time_advancer = tokio::spawn(async {
        loop {
            tokio::time::advance(Duration::from_millis(1)).await;
            tokio::task::yield_now().await;
        }
    });

    let repo = Arc::new(InMemorySubstituterRepository::new());
    for sub in env.substituters.iter() {
        repo.save(sub.clone()).await;
    }

    let nar_info_provider = Arc::new(MockNarInfoProvider::new(env.nar_info_entries));

    let nar_resolution_service = NarInfoService::new(
        Arc::new(PriorityPolicy::new(
            nar_info_provider.clone(),
            env.ignore_query_error,
        )),
        repo,
        NarUrlRewriteOption::ToSelf,
    );

    let (res, events) = nar_resolution_service
        .resolve(&input.hash, PassthroughHeaders::empty())
        .await;

    assert_eq!(
        res.map(|resolution| resolution.source_url().cloned())
            .map_err(|_| ()),
        expectation.source_url,
    );

    assert_eq!(
        events.into_iter().collect::<HashSet<_>>(),
        expectation.events.into_iter().collect::<HashSet<_>>(),
    );

    let queried = nar_info_provider
        .queried_urls()
        .await
        .into_iter()
        .collect::<HashSet<_>>();
    let expected_queried = expectation
        .queried
        .iter()
        .map(|url| make_nar_info_url(url, &input.hash))
        .collect::<HashSet<_>>();
    assert_eq!(queried, expected_queried);
}

#[tokio::test(start_paused = true)]
async fn higher_priority_success_overrides_faster_lower_priority() {
    let low_url = Url::new("https://mirror.example.com").unwrap();
    let high_url = Url::new("https://cache.nixos.org").unwrap();
    let low = make_substituter_normal_with_url_pri(&low_url, 40);
    let high = make_substituter_normal_with_url_pri(&high_url, 10);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![low, high],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&low_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
                (
                    make_nar_info_url(&high_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(100))),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&high_url, 10))),
            events: vec![make_nar_file_located_event(&high_url, 10)],
            queried: vec![low_url, high_url],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn higher_priority_success_cancels_lower_priority_in_flight() {
    let high_url = Url::new("https://cache.nixos.org").unwrap();
    let low_url = Url::new("https://mirror.example.com").unwrap();
    let high = make_substituter_normal_with_url_pri(&high_url, 10);
    let low =
        make_substituter_normal_with_nar_info_timeout(&low_url, 40, Duration::from_millis(100));
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![low, high],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&low_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(1600))),
                ),
                (
                    make_nar_info_url(&high_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(50))),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&high_url, 10))),
            events: vec![make_nar_file_located_event(&high_url, 10)],
            queried: vec![low_url, high_url],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn non_successful_higher_priority_queries_do_not_override_lower_priority_success() {
    let not_found_url = Url::new("https://not-found.example.com").unwrap();
    let error_url = Url::new("https://error.example.com").unwrap();
    let offline_url = Url::new("https://offline.example.com").unwrap();
    let success_url = Url::new("https://success.example.com").unwrap();
    let not_found = make_substituter_normal_with_url_pri(&not_found_url, 10);
    let error = make_substituter_normal_with_url_pri(&error_url, 20);
    let offline =
        make_substituter_normal_with_nar_info_timeout(&offline_url, 30, Duration::from_millis(100));
    let success = make_substituter_normal_with_url_pri(&success_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![success, not_found, error, offline],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&error_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    make_nar_info_url(&offline_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(1600))),
                ),
                (
                    make_nar_info_url(&success_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&success_url, 40))),
            events: vec![
                ResolveNarInfoEvent::SubstituterError(error_url.clone()),
                ResolveNarInfoEvent::SubstituterOffline(offline_url.clone()),
                make_nar_file_located_event(&success_url, 40),
            ],
            queried: vec![not_found_url, error_url, offline_url, success_url],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn same_priority_fastest_response_wins() {
    let fast_url = Url::new("https://fast.example.com").unwrap();
    let slow_url = Url::new("https://slow.example.com").unwrap();
    let fast = make_substituter_normal_with_url_pri(&fast_url, 10);
    let slow = make_substituter_normal_with_url_pri(&slow_url, 10);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![slow, fast],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&slow_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(1600))),
                ),
                (
                    make_nar_info_url(&fast_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(50))),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&fast_url, 10))),
            events: vec![make_nar_file_located_event(&fast_url, 10)],
            queried: vec![slow_url, fast_url],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_not_found_yields_not_found() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = make_substituter_normal_with_url_pri(&sub_a_url, 10);
    let sub_b = make_substituter_normal_with_url_pri(&sub_b_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(None),
            events: vec![],
            queried: vec![sub_a_url, sub_b_url],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_errors_yield_infrastructure_error() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = make_substituter_normal_with_url_pri(&sub_a_url, 10);
    let sub_b = make_substituter_normal_with_url_pri(&sub_b_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&sub_a_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    make_nar_info_url(&sub_b_url, &hash),
                    Err("stub error".into()),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Err(()),
            events: vec![
                ResolveNarInfoEvent::SubstituterError(sub_a_url.clone()),
                ResolveNarInfoEvent::SubstituterError(sub_b_url.clone()),
            ],
            queried: vec![sub_a_url, sub_b_url],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn errors_ignored_yield_not_found() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = make_substituter_normal_with_url_pri(&sub_a_url, 10);
    let sub_b = make_substituter_normal_with_url_pri(&sub_b_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&sub_a_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    make_nar_info_url(&sub_b_url, &hash),
                    Err("stub error".into()),
                ),
            ],
            ignore_query_error: true,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(None),
            events: vec![
                ResolveNarInfoEvent::SubstituterError(sub_a_url.clone()),
                ResolveNarInfoEvent::SubstituterError(sub_b_url.clone()),
            ],
            queried: vec![sub_a_url, sub_b_url],
        },
    )
    .await;
}
