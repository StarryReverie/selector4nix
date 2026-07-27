use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::common::url::Url;
use selector4nix_core::domain::nar_info::model::test_support::{
    make_nar_file_name, make_store_path_hash,
};
use selector4nix_core::domain::nar_info::model::{NarUrlRewriteOption, StorePathHash};
use selector4nix_core::domain::nar_info::port::NarInfoQueryData;
use selector4nix_core::domain::nar_info::{NarInfoService, ResolveNarInfoEvent};
use selector4nix_core::domain::substituter::SubstituterRepository;
use selector4nix_core::domain::substituter::model::Substituter;
use selector4nix_core::domain::substituter::model::test_support::{
    make_substituter_maybe_ready_with_url_pri, make_substituter_meta_with_url_pri,
    make_substituter_normal_with_url_pri,
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
    tolerance: u64,
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

    let nar_info_provider = MockNarInfoProvider::new(env.nar_info_entries);

    let nar_resolution_service = NarInfoService::new(
        Arc::new(nar_info_provider),
        repo,
        NarUrlRewriteOption::ToSelf,
        env.tolerance,
        env.ignore_query_error,
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
}

#[tokio::test(start_paused = true)]
async fn single_normal_substituter_resolves() {
    let sub_url = Url::new("https://cache.nixos.org").unwrap();
    let sub = make_substituter_normal_with_url_pri(&sub_url, 40);
    let hash = make_store_path_hash();
    let nar_info_url = make_nar_info_url(&sub_url, &hash);

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub],
            nar_info_entries: vec![(
                nar_info_url,
                Ok(make_nar_info_query_data(Duration::from_millis(0))),
            )],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash: hash.clone() },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&sub_url, 40))),
            events: vec![ResolveNarInfoEvent::NarFileLocated {
                nar_file: make_nar_file_name(),
                substituter: make_substituter_meta_with_url_pri(&sub_url, 40),
                source_url: make_source_url(&sub_url, 40),
                store_path_hash: make_store_path_hash(),
            }],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_substituters_fail() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = make_substituter_normal_with_url_pri(&sub_a_url, 40);
    let sub_b = make_substituter_normal_with_url_pri(&sub_b_url, 10);
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
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Err(()),
            events: vec![
                ResolveNarInfoEvent::SubstituterError(sub_a_url),
                ResolveNarInfoEvent::SubstituterError(sub_b_url),
            ],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn non_normal_substituter_emits_succeeded_event() {
    let sub_url = Url::new("https://cache-a.example.com").unwrap();
    let sub = make_substituter_maybe_ready_with_url_pri(&sub_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub],
            nar_info_entries: vec![(
                make_nar_info_url(&sub_url, &hash),
                Ok(make_nar_info_query_data(Duration::from_millis(0))),
            )],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&sub_url, 40))),
            events: vec![
                ResolveNarInfoEvent::SubstituterSucceeded(sub_url.clone()),
                ResolveNarInfoEvent::NarFileLocated {
                    nar_file: make_nar_file_name(),
                    substituter: make_substituter_meta_with_url_pri(&sub_url, 40),
                    source_url: make_source_url(&sub_url, 40),
                    store_path_hash: make_store_path_hash(),
                },
            ],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn lower_priority_value_preferred_at_equal_latency() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = make_substituter_normal_with_url_pri(&sub_a_url, 40);
    let sub_b = make_substituter_normal_with_url_pri(&sub_b_url, 10);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&sub_a_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
                (
                    make_nar_info_url(&sub_b_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
            ],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&sub_b_url, 10))),
            events: vec![ResolveNarInfoEvent::NarFileLocated {
                nar_file: make_nar_file_name(),
                substituter: make_substituter_meta_with_url_pri(&sub_b_url, 10),
                source_url: make_source_url(&sub_b_url, 10),
                store_path_hash: make_store_path_hash(),
            }],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn faster_high_priority_value_beats_slow_low() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = make_substituter_normal_with_url_pri(&sub_a_url, 40);
    let sub_b = make_substituter_normal_with_url_pri(&sub_b_url, 10);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&sub_a_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
                (
                    make_nar_info_url(&sub_b_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(1600))),
                ),
            ],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&sub_a_url, 40))),
            events: vec![ResolveNarInfoEvent::NarFileLocated {
                nar_file: make_nar_file_name(),
                substituter: make_substituter_meta_with_url_pri(&sub_a_url, 40),
                source_url: make_source_url(&sub_a_url, 40),
                store_path_hash: make_store_path_hash(),
            }],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn partial_error_with_success() {
    let error_substituter_url = Url::new("https://cache-a.example.com").unwrap();
    let success_substituter_url = Url::new("https://cache-b.example.com").unwrap();
    let error_sub = make_substituter_normal_with_url_pri(&error_substituter_url, 40);
    let success_sub = make_substituter_normal_with_url_pri(&success_substituter_url, 10);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![error_sub, success_sub],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&error_substituter_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    make_nar_info_url(&success_substituter_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
            ],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&success_substituter_url, 10))),
            events: vec![
                ResolveNarInfoEvent::SubstituterError(error_substituter_url),
                ResolveNarInfoEvent::NarFileLocated {
                    nar_file: make_nar_file_name(),
                    substituter: make_substituter_meta_with_url_pri(&success_substituter_url, 10),
                    source_url: make_source_url(&success_substituter_url, 10),
                    store_path_hash: make_store_path_hash(),
                },
            ],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_substituters_service_fail_with_ignore_error() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = make_substituter_normal_with_url_pri(&sub_a_url, 40);
    let sub_b = make_substituter_normal_with_url_pri(&sub_b_url, 10);
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
            tolerance: 50,
            ignore_query_error: true,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(None),
            events: vec![
                ResolveNarInfoEvent::SubstituterError(sub_a_url),
                ResolveNarInfoEvent::SubstituterError(sub_b_url),
            ],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_substituters_offline_treated_as_not_found() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a =
        make_substituter_normal_with_nar_info_timeout(&sub_a_url, 40, Duration::from_millis(10));
    let sub_b =
        make_substituter_normal_with_nar_info_timeout(&sub_b_url, 10, Duration::from_millis(20));
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    make_nar_info_url(&sub_a_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(100))),
                ),
                (
                    make_nar_info_url(&sub_b_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(200))),
                ),
            ],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(None),
            events: vec![
                ResolveNarInfoEvent::SubstituterOffline(sub_a_url),
                ResolveNarInfoEvent::SubstituterOffline(sub_b_url),
            ],
        },
    )
    .await;
}
