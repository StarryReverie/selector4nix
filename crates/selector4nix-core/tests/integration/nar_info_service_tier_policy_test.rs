use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use selector4nix_core::domain::common::passthrough_headers::PassthroughHeaders;
use selector4nix_core::domain::common::url::Url;
use selector4nix_core::domain::nar_info::model::test_support::{
    make_nar_file_name, make_store_path_hash,
};
use selector4nix_core::domain::nar_info::model::{NarUrlRewriteOption, StorePathHash};
use selector4nix_core::domain::nar_info::policy::TierPolicy;
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
    not_queried: Vec<Url>,
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
        Arc::new(TierPolicy::new(
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

    let queried = nar_info_provider.queried_urls().await;
    for url in &expectation.not_queried {
        let nar_info_url = make_nar_info_url(url, &input.hash);
        assert!(
            !queried.contains(&nar_info_url),
            "expected {url} to not be queried, but it was"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn lower_tier_queried_when_higher_tier_lacks_nar_info() {
    let mirror_url = Url::new("https://mirror.example.com").unwrap();
    let official_url = Url::new("https://cache.nixos.org").unwrap();
    let mirror = make_substituter_normal_with_url_pri(&mirror_url, 10);
    let official = make_substituter_normal_with_url_pri(&official_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![mirror, official],
            // The mirror tier lacks the NAR info (absent from entries = not
            // found), so the official tier is queried and wins.
            nar_info_entries: vec![(
                make_nar_info_url(&official_url, &hash),
                Ok(make_nar_info_query_data(Duration::from_millis(0))),
            )],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&official_url, 40))),
            events: vec![ResolveNarInfoEvent::NarFileLocated {
                nar_file: make_nar_file_name(),
                substituter: make_substituter_meta_with_url_pri(&official_url, 40),
                source_url: make_source_url(&official_url, 40),
                store_path_hash: make_store_path_hash(),
            }],
            not_queried: vec![],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn slow_higher_tier_beats_fast_lower_tier() {
    let mirror_url = Url::new("https://mirror.example.com").unwrap();
    let official_url = Url::new("https://cache.nixos.org").unwrap();
    let mirror = make_substituter_normal_with_url_pri(&mirror_url, 10);
    let official = make_substituter_normal_with_url_pri(&official_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![mirror, official],
            // The mirror is slow (1600ms) but holds the NAR info, while the
            // official cache would answer instantly: under strict tiering the
            // official tier is never queried, so the mirror wins.
            nar_info_entries: vec![
                (
                    make_nar_info_url(&mirror_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(1600))),
                ),
                (
                    make_nar_info_url(&official_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&mirror_url, 10))),
            events: vec![ResolveNarInfoEvent::NarFileLocated {
                nar_file: make_nar_file_name(),
                substituter: make_substituter_meta_with_url_pri(&mirror_url, 10),
                source_url: make_source_url(&mirror_url, 10),
                store_path_hash: make_store_path_hash(),
            }],
            not_queried: vec![official_url],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn lower_tier_queried_when_higher_tier_errors() {
    let mirror_url = Url::new("https://mirror.example.com").unwrap();
    let official_url = Url::new("https://cache.nixos.org").unwrap();
    let mirror = make_substituter_normal_with_url_pri(&mirror_url, 10);
    let official = make_substituter_normal_with_url_pri(&official_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![mirror, official],
            // The mirror tier fails with a service error rather than
            // not-found; strict tiering still queries the official tier.
            nar_info_entries: vec![
                (
                    make_nar_info_url(&mirror_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    make_nar_info_url(&official_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&official_url, 40))),
            events: vec![
                ResolveNarInfoEvent::SubstituterError(mirror_url),
                ResolveNarInfoEvent::NarFileLocated {
                    nar_file: make_nar_file_name(),
                    substituter: make_substituter_meta_with_url_pri(&official_url, 40),
                    source_url: make_source_url(&official_url, 40),
                    store_path_hash: make_store_path_hash(),
                },
            ],
            not_queried: vec![],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn races_within_same_tier() {
    let mirror_a_url = Url::new("https://mirror-a.example.com").unwrap();
    let mirror_b_url = Url::new("https://mirror-b.example.com").unwrap();
    let official_url = Url::new("https://cache.nixos.org").unwrap();
    let mirror_a = make_substituter_normal_with_url_pri(&mirror_a_url, 10);
    let mirror_b = make_substituter_normal_with_url_pri(&mirror_b_url, 10);
    let official = make_substituter_normal_with_url_pri(&official_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![mirror_a, mirror_b, official],
            // Within the tier of priority 10 the faster mirror_a wins over
            // the slower mirror_b; the official tier (40) answers instantly
            // but is never queried because the higher tier found the NAR info.
            nar_info_entries: vec![
                (
                    make_nar_info_url(&mirror_a_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
                (
                    make_nar_info_url(&mirror_b_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(1600))),
                ),
                (
                    make_nar_info_url(&official_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(0))),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(Some(make_source_url(&mirror_a_url, 10))),
            events: vec![ResolveNarInfoEvent::NarFileLocated {
                nar_file: make_nar_file_name(),
                substituter: make_substituter_meta_with_url_pri(&mirror_a_url, 10),
                source_url: make_source_url(&mirror_a_url, 10),
                store_path_hash: make_store_path_hash(),
            }],
            not_queried: vec![official_url],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_tiers_error_yields_infrastructure_error() {
    let mirror_url = Url::new("https://mirror.example.com").unwrap();
    let official_url = Url::new("https://cache.nixos.org").unwrap();
    let mirror = make_substituter_normal_with_url_pri(&mirror_url, 10);
    let official = make_substituter_normal_with_url_pri(&official_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![mirror, official],
            // Every tier errors and no tier is rescued: the sticky error
            // makes the overall lookup fail, mirroring racing semantics.
            nar_info_entries: vec![
                (
                    make_nar_info_url(&mirror_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    make_nar_info_url(&official_url, &hash),
                    Err("stub error".into()),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Err(()),
            events: vec![
                ResolveNarInfoEvent::SubstituterError(mirror_url),
                ResolveNarInfoEvent::SubstituterError(official_url),
            ],
            not_queried: vec![],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn errors_treated_as_not_found_with_ignore_error() {
    let mirror_url = Url::new("https://mirror.example.com").unwrap();
    let official_url = Url::new("https://cache.nixos.org").unwrap();
    let mirror = make_substituter_normal_with_url_pri(&mirror_url, 10);
    let official = make_substituter_normal_with_url_pri(&official_url, 40);
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![mirror, official],
            // Every tier errors, but query errors are ignored: the sticky
            // error is downgraded to not-found instead of failing the lookup.
            nar_info_entries: vec![
                (
                    make_nar_info_url(&mirror_url, &hash),
                    Err("stub error".into()),
                ),
                (
                    make_nar_info_url(&official_url, &hash),
                    Err("stub error".into()),
                ),
            ],
            ignore_query_error: true,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(None),
            events: vec![
                ResolveNarInfoEvent::SubstituterError(mirror_url),
                ResolveNarInfoEvent::SubstituterError(official_url),
            ],
            not_queried: vec![],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn all_offline_treated_as_not_found() {
    let mirror_url = Url::new("https://mirror.example.com").unwrap();
    let official_url = Url::new("https://cache.nixos.org").unwrap();
    let mirror =
        make_substituter_normal_with_nar_info_timeout(&mirror_url, 10, Duration::from_millis(10));
    let official =
        make_substituter_normal_with_nar_info_timeout(&official_url, 40, Duration::from_millis(20));
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![mirror, official],
            // Both tiers time out (offline): offline is not an error, so the
            // overall lookup resolves to not-found, mirroring racing semantics.
            nar_info_entries: vec![
                (
                    make_nar_info_url(&mirror_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(100))),
                ),
                (
                    make_nar_info_url(&official_url, &hash),
                    Ok(make_nar_info_query_data(Duration::from_millis(200))),
                ),
            ],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(None),
            events: vec![
                ResolveNarInfoEvent::SubstituterOffline(mirror_url),
                ResolveNarInfoEvent::SubstituterOffline(official_url),
            ],
            not_queried: vec![],
        },
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn no_substituter_yields_not_found() {
    let hash = make_store_path_hash();

    run_test(
        TestCaseEnvironment {
            substituters: vec![],
            nar_info_entries: vec![],
            ignore_query_error: false,
        },
        TestCaseInput { hash },
        TestCaseExpectation {
            source_url: Ok(None),
            events: vec![],
            not_queried: vec![],
        },
    )
    .await;
}
