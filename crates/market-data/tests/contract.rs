//! Contract tests for the live Massive adapter (ROADMAP P7.4).
//!
//! Every case runs **offline** against a local `wiremock` server replaying synthetic
//! fixtures (never fetched data — the repo guardrail). They exercise the real
//! `MarketDataProvider` methods end-to-end through the HTTP path, so a change in the shape
//! we expect from Massive fails here in CI rather than during a live scan. The live network
//! path is never touched — only a human running with a real key and the real base URL hits
//! api.massive.com.
#![cfg(feature = "massive-live")]
// This is a test-only crate. clippy's `allow-{unwrap,expect}-in-tests` recognizes `#[test]`
// fns but not the shared `fn source()` helper below, so allow the no-panic lints file-wide —
// the production discipline is unaffected (nothing here ships).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use market_data::{MarketDataProvider, MassiveSource, ProviderError};
use wiremock::matchers::{header, method, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// An adapter pointed at the mock server, with near-zero backoff so retries don't wait.
fn source(uri: &str) -> MassiveSource {
    MassiveSource::with_key("test-key")
        .expect("client builds")
        .with_base_url(uri)
        .with_backoff_base(Duration::from_millis(1))
}

#[tokio::test]
async fn daily_bars_maps_adjusted_aggregates() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/v2/aggs/ticker/AAPL/range/1/day/.*"))
        // The correctness invariant on the wire: we always request split-adjusted prices.
        .and(query_param("adjusted", "true"))
        // The key rides the Authorization header, never the URL.
        .and(header("authorization", "Bearer test-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("fixtures/aggs_ok.json")),
        )
        .mount(&server)
        .await;

    let bars = source(&server.uri())
        .daily_bars("AAPL", 100)
        .await
        .expect("fixture maps cleanly");

    // Three bars, millis→seconds, sorted ascending (the fixture is deliberately out of order).
    assert_eq!(bars.len(), 3);
    assert_eq!(bars[0].t, 1);
    assert_eq!(bars[1].t, 2);
    assert_eq!(bars[2].t, 3);
    assert!((bars[2].close - 102.5).abs() < 1e-9);
}

#[tokio::test]
async fn rate_limited_surfaces_retry_after_hint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "2"))
        .mount(&server)
        .await;

    let err = source(&server.uri())
        .daily_bars("AAPL", 100)
        .await
        .expect_err("429 must be an error");
    // The hint is parsed and surfaced for the caller to honor — not blocked on mid-call.
    assert_eq!(
        err,
        ProviderError::RateLimited {
            retry_after: Some(Duration::from_secs(2))
        }
    );
}

#[tokio::test]
async fn unauthorized_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    assert!(matches!(
        source(&server.uri()).daily_bars("AAPL", 100).await,
        Err(ProviderError::Auth(_))
    ));
}

#[tokio::test]
async fn iv_snapshot_reads_atm_iv_with_empty_history() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/v3/snapshot/options/AAPL"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("fixtures/chain_ok.json")),
        )
        .mount(&server)
        .await;

    let snap = source(&server.uri())
        .iv_snapshot("AAPL")
        .await
        .expect("chain maps cleanly");

    assert_eq!(snap.symbol, "AAPL");
    // Underlying 101 → ATM strike 100; call 0.30 + put 0.40 averaged = 0.35.
    assert!((snap.iv - 0.35).abs() < 1e-9, "got {}", snap.iv);
    // History is empty here — accumulating the trailing IV series is Phase 8's store.
    assert!(snap.iv_history.is_empty());
}
