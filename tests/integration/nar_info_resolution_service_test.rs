use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use selector4nix::domain::nar_info::model::{NarUrlRewriteOption, StorePathHash};
use selector4nix::domain::nar_info::port::NarInfoQueryData;
use selector4nix::domain::nar_info::service::{
    NarInfoResolutionEvent, NarInfoResolutionService, ResolveNarInfoError,
};
use selector4nix::domain::substituter::model::{Substituter, Url};
use selector4nix::infrastructure::index::SubstituterAvailabilityIndexActor;

use crate::fixture::{nar, substituter};
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
    source_url: Result<Option<Url>, ResolveNarInfoError>,
    events: Vec<NarInfoResolutionEvent>,
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

    let (avail_index_actor, avail_index) = SubstituterAvailabilityIndexActor::new(env.substituters);
    avail_index_actor.run();

    let nar_info_provider = MockNarInfoProvider::new(env.nar_info_entries.into_iter());

    let nar_resolution_service = NarInfoResolutionService::new(
        Arc::new(nar_info_provider),
        Arc::new(avail_index),
        NarUrlRewriteOption::ToSelf,
        env.tolerance,
        env.ignore_query_error,
    );

    let (res, events) = nar_resolution_service.resolve(&input.hash).await;

    assert_eq!(
        res.map(|resolution| resolution.source_url().cloned()),
        expectation.source_url,
    );

    assert_eq!(
        events.into_iter().collect::<HashSet<_>>(),
        expectation.events.into_iter().collect::<HashSet<_>>(),
    );
}

#[tokio::test(start_paused = true)]
async fn single_normal_sub_resolves() {
    let sub_url = Url::new("https://cache.nixos.org").unwrap();
    let sub = substituter::make_substituter_normal(&sub_url, 40);
    let hash = nar::make_store_path_hash();
    let nar_info_url = nar::make_nar_info_url(&sub_url, &hash);

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub],
            nar_info_entries: vec![(
                nar_info_url,
                Ok(nar::make_nar_info_query_data(Duration::from_millis(0))),
            )],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash: hash.clone() },
        TestCaseExpectation {
            source_url: Ok(Some(nar::make_source_url(&sub_url, 40))),
            events: vec![],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_subs_fail() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = substituter::make_substituter_normal(&sub_a_url, 40);
    let sub_b = substituter::make_substituter_normal(&sub_b_url, 10);
    let hash = nar::make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    nar::make_nar_info_url(&sub_a_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    nar::make_nar_info_url(&sub_b_url, &hash),
                    Err("stub error".into()),
                ),
            ],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Err(ResolveNarInfoError::Fetch),
            events: vec![
                NarInfoResolutionEvent::SubstituterError(sub_a_url),
                NarInfoResolutionEvent::SubstituterError(sub_b_url),
            ],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn non_normal_sub_emits_succeeded_event() {
    let sub_url = Url::new("https://cache-a.example.com").unwrap();
    let sub = substituter::make_substituter_maybe_ready(&sub_url, 40);
    let hash = nar::make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub],
            nar_info_entries: vec![(
                nar::make_nar_info_url(&sub_url, &hash),
                Ok(nar::make_nar_info_query_data(Duration::from_millis(0))),
            )],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(nar::make_source_url(&sub_url, 40))),
            events: vec![NarInfoResolutionEvent::SubstituterSucceeded(sub_url)],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn lower_priority_value_preferred_at_equal_latency() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = substituter::make_substituter_normal(&sub_a_url, 40);
    let sub_b = substituter::make_substituter_normal(&sub_b_url, 10);
    let hash = nar::make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    nar::make_nar_info_url(&sub_a_url, &hash),
                    Ok(nar::make_nar_info_query_data(Duration::from_millis(0))),
                ),
                (
                    nar::make_nar_info_url(&sub_b_url, &hash),
                    Ok(nar::make_nar_info_query_data(Duration::from_millis(0))),
                ),
            ],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(nar::make_source_url(&sub_b_url, 10))),
            events: vec![],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn faster_high_priority_value_beats_slow_low() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = substituter::make_substituter_normal(&sub_a_url, 40);
    let sub_b = substituter::make_substituter_normal(&sub_b_url, 10);
    let hash = nar::make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    nar::make_nar_info_url(&sub_a_url, &hash),
                    Ok(nar::make_nar_info_query_data(Duration::from_millis(0))),
                ),
                (
                    nar::make_nar_info_url(&sub_b_url, &hash),
                    Ok(nar::make_nar_info_query_data(Duration::from_millis(1600))),
                ),
            ],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(nar::make_source_url(&sub_a_url, 40))),
            events: vec![],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn partial_error_with_success() {
    let error_sub_url = Url::new("https://cache-a.example.com").unwrap();
    let success_sub_url = Url::new("https://cache-b.example.com").unwrap();
    let error_sub = substituter::make_substituter_normal(&error_sub_url, 40);
    let success_sub = substituter::make_substituter_normal(&success_sub_url, 10);
    let hash = nar::make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![error_sub, success_sub],
            nar_info_entries: vec![
                (
                    nar::make_nar_info_url(&error_sub_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    nar::make_nar_info_url(&success_sub_url, &hash),
                    Ok(nar::make_nar_info_query_data(Duration::from_millis(0))),
                ),
            ],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(nar::make_source_url(&success_sub_url, 10))),
            events: vec![NarInfoResolutionEvent::SubstituterError(error_sub_url)],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_subs_service_fail_with_ignore_error() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = substituter::make_substituter_normal(&sub_a_url, 40);
    let sub_b = substituter::make_substituter_normal(&sub_b_url, 10);
    let hash = nar::make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    nar::make_nar_info_url(&sub_a_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    nar::make_nar_info_url(&sub_b_url, &hash),
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
                NarInfoResolutionEvent::SubstituterError(sub_a_url),
                NarInfoResolutionEvent::SubstituterError(sub_b_url),
            ],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_subs_offline_treated_as_not_found() {
    let sub_a_url = Url::new("https://cache-a.example.com").unwrap();
    let sub_b_url = Url::new("https://cache-b.example.com").unwrap();
    let sub_a = substituter::make_substituter_normal_with_nar_info_timeout(
        &sub_a_url,
        40,
        Duration::from_millis(10),
    );
    let sub_b = substituter::make_substituter_normal_with_nar_info_timeout(
        &sub_b_url,
        10,
        Duration::from_millis(20),
    );
    let hash = nar::make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![sub_a, sub_b],
            nar_info_entries: vec![
                (
                    nar::make_nar_info_url(&sub_a_url, &hash),
                    Ok(nar::make_nar_info_query_data(Duration::from_millis(100))),
                ),
                (
                    nar::make_nar_info_url(&sub_b_url, &hash),
                    Ok(nar::make_nar_info_query_data(Duration::from_millis(200))),
                ),
            ],
            tolerance: 50,
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(None),
            events: vec![
                NarInfoResolutionEvent::SubstituterOffline(sub_a_url),
                NarInfoResolutionEvent::SubstituterOffline(sub_b_url),
            ],
        },
    )
    .await;
}
