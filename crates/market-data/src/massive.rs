//! The live Massive market-data adapter (ROADMAP P7.1 + P7.2), **end-of-day only**.
//!
//! Two REST endpoints, mapped through the anti-corruption boundary into the canonical
//! [`Bar`] / [`IvSnapshot`] types so `signals` never sees a raw payload:
//! - `GET /v2/aggs/ticker/{sym}/range/1/day/{from}/{to}` → daily bars, always **split-adjusted**
//!   (realized vol on unadjusted prices is wrong — the #1 data-correctness bug).
//! - `GET /v3/snapshot/options/{sym}` → the ATM ~30-DTE implied vol.
//!
//! The raw→canonical mapping and the calendar math are **pure functions**, tested against
//! recorded fixtures (`tests/contract.rs`); only `daily_bars`/`iv_snapshot` touch the network.

use async_trait::async_trait;
use exub_core::{
    Bar, Capability, IvSnapshot, MarketDataProvider, Provider, ProviderError, ProviderInfo,
    ProviderKind, ProviderResult,
};
use secrecy::SecretString;
use serde::Deserialize;
use std::time::Duration;

use crate::http::HttpClient;

const MASSIVE_BASE_URL: &str = "https://api.massive.com";
const SECS_PER_DAY: i64 = 86_400;
const MS_PER_DAY: i64 = 86_400_000;
/// Target days-to-expiry for the ATM implied-vol read (the market's ~1-month forecast).
const TARGET_DTE: i64 = 30;

// --- Wire DTOs (tolerant: unknown fields are ignored, so a growing payload never breaks us) ---

#[derive(Debug, Deserialize)]
struct AggsResponse {
    #[serde(default)]
    results: Vec<AggBar>,
}

#[derive(Debug, Deserialize)]
struct AggBar {
    /// Unix **millisecond** timestamp for the start of the window.
    t: i64,
    o: f64,
    h: f64,
    l: f64,
    c: f64,
    #[serde(default)]
    v: f64,
}

#[derive(Debug, Deserialize)]
struct ChainResponse {
    #[serde(default)]
    results: Vec<OptionContract>,
}

#[derive(Debug, Deserialize)]
struct OptionContract {
    #[serde(default)]
    implied_volatility: Option<f64>,
    details: ContractDetails,
    #[serde(default)]
    underlying_asset: Option<UnderlyingAsset>,
}

#[derive(Debug, Deserialize)]
struct ContractDetails {
    strike_price: f64,
    /// `YYYY-MM-DD`.
    expiration_date: String,
}

#[derive(Debug, Deserialize)]
struct UnderlyingAsset {
    #[serde(default)]
    price: Option<f64>,
}

// --- The adapter ---

/// Live Massive EOD data source. Holds the API key in a [`SecretString`] so it can't leak to
/// a log or an error; `base_url`/backoff are overridable for the offline contract tests.
#[derive(Debug)]
pub struct MassiveSource {
    api_key: SecretString,
    base_url: String,
    http: HttpClient,
}

impl MassiveSource {
    /// Construct from the `MASSIVE_API_KEY` environment variable.
    ///
    /// # Errors
    /// [`ProviderError::Auth`] if the key is unset; [`ProviderError::Transport`] if the HTTP
    /// client can't be built.
    pub fn from_env() -> ProviderResult<Self> {
        let key = std::env::var("MASSIVE_API_KEY")
            .map_err(|_| ProviderError::Auth("MASSIVE_API_KEY not set".into()))?;
        Self::with_key(key)
    }

    /// Construct with an explicit key (used by contract tests; `from_env` is the real path).
    ///
    /// # Errors
    /// [`ProviderError::Transport`] if the HTTP client can't be built.
    pub fn with_key(key: impl Into<String>) -> ProviderResult<Self> {
        Ok(Self {
            api_key: SecretString::from(key.into()),
            base_url: MASSIVE_BASE_URL.to_string(),
            http: HttpClient::new()?,
        })
    }

    /// Point the adapter at a different base URL (a wiremock server in tests).
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Shrink the retry backoff so tests exercise the retry path without real waiting.
    #[must_use]
    pub fn with_backoff_base(mut self, base: Duration) -> Self {
        self.http = self.http.with_backoff_base(base);
        self
    }
}

impl Provider for MassiveSource {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "massive".into(),
            kind: ProviderKind::MarketData,
            // EOD-only (the re-scope): no IntradayBars / Quotes. No OptionsHistory → the
            // engine accumulates the IV distribution forward (Phase 8), not backfills.
            capabilities: vec![
                Capability::DailyBars,
                Capability::ImpliedVol,
                Capability::OptionsChain,
            ],
        }
    }
}

#[async_trait]
impl MarketDataProvider for MassiveSource {
    /// Fetch split-**adjusted** daily bars covering roughly `lookback_days` sessions.
    ///
    /// `adjusted=true` is not optional: realized vol computed on unadjusted prices treats a
    /// split as a real move and is simply wrong (P7.2). The window is padded past
    /// `lookback_days` to cover weekends/holidays, then trimmed to the most recent bars.
    async fn daily_bars(&self, symbol: &str, lookback_days: usize) -> ProviderResult<Vec<Bar>> {
        let to_ms = now_millis();
        // ~1.6 calendar days per trading day, plus a few days of slack for long weekends.
        let span_days = ((lookback_days as f64) * 1.6).ceil() as i64 + 5;
        let from_ms = to_ms - span_days * MS_PER_DAY;
        let url = format!(
            "{}/v2/aggs/ticker/{symbol}/range/1/day/{from_ms}/{to_ms}?adjusted=true&sort=asc&limit=50000",
            self.base_url
        );
        let resp: AggsResponse = self.http.get_json(&url, &self.api_key).await?;
        map_aggs(resp, lookback_days, symbol)
    }

    /// Fetch the ATM ~30-DTE implied vol as a snapshot. History is empty here — accumulating
    /// the trailing IV series is the store's job (Phase 8), which is what makes IV *rank*
    /// computable at all.
    async fn iv_snapshot(&self, symbol: &str) -> ProviderResult<IvSnapshot> {
        let now_s = now_secs();
        let today = now_s / SECS_PER_DAY;
        let (gy, gm, gd) = civil_from_days(today + 21);
        let (ly, lm, ld) = civil_from_days(today + 45);
        let url = format!(
            "{}/v3/snapshot/options/{symbol}\
             ?expiration_date.gte={gy:04}-{gm:02}-{gd:02}\
             &expiration_date.lte={ly:04}-{lm:02}-{ld:02}&limit=250",
            self.base_url
        );
        let resp: ChainResponse = self.http.get_json(&url, &self.api_key).await?;
        let iv = select_atm_iv(&resp, now_s, symbol)?;
        Ok(IvSnapshot::new(symbol, iv, Vec::new()))
    }
}

// --- Pure mapping / validation (fixture-tested, no network) ---

/// Map an aggregates response to canonical bars: **millis → epoch-seconds**, bad ticks
/// dropped, sorted ascending, duplicate timestamps removed, trimmed to the most recent
/// `lookback_days`. `NotFound` if nothing valid survives.
fn map_aggs(resp: AggsResponse, lookback_days: usize, symbol: &str) -> ProviderResult<Vec<Bar>> {
    let mut bars = Vec::with_capacity(resp.results.len());
    let mut dropped = 0usize;
    for b in resp.results {
        if is_valid_tick(&b) {
            // Massive timestamps are epoch millis; the canonical `Bar.t` is epoch seconds.
            bars.push(Bar::new(b.t / 1000, b.o, b.h, b.l, b.c, b.v));
        } else {
            dropped += 1;
        }
    }
    if dropped > 0 {
        tracing::warn!(symbol, dropped, "dropped invalid daily bars from Massive");
    }
    bars.sort_by_key(|b| b.t);
    bars.dedup_by_key(|b| b.t);
    if bars.is_empty() {
        return Err(ProviderError::NotFound(format!(
            "no valid daily bars for {symbol}"
        )));
    }
    let start = bars.len().saturating_sub(lookback_days);
    Ok(bars[start..].to_vec())
}

/// A bar is usable only if every price is finite and positive, the high isn't below the low,
/// and volume is non-negative — anything else is a bad tick we refuse to feed a screen.
fn is_valid_tick(b: &AggBar) -> bool {
    let prices_ok = [b.o, b.h, b.l, b.c]
        .iter()
        .all(|p| p.is_finite() && *p > 0.0);
    prices_ok && b.h >= b.l && b.v.is_finite() && b.v >= 0.0
}

/// Select the ATM implied vol nearest [`TARGET_DTE`] from an options-chain snapshot:
/// pick the expiration closest to 30 DTE, the strike nearest the underlying, and average the
/// call+put IV at that strike (skipping absent/non-positive values).
fn select_atm_iv(chain: &ChainResponse, now_secs: i64, symbol: &str) -> ProviderResult<f64> {
    if chain.results.is_empty() {
        return Err(ProviderError::NotFound(format!(
            "empty options chain for {symbol}"
        )));
    }
    let underlying = chain
        .results
        .iter()
        .find_map(|c| c.underlying_asset.as_ref().and_then(|u| u.price))
        .ok_or_else(|| {
            ProviderError::Transport(format!("no underlying price in chain for {symbol}"))
        })?;

    let today = now_secs / SECS_PER_DAY;
    // The expiration closest to 30 days out among those returned.
    let target_exp = chain
        .results
        .iter()
        .filter_map(|c| {
            dte(&c.details.expiration_date, today).map(|d| (&c.details.expiration_date, d))
        })
        .min_by_key(|(_, d)| (d - TARGET_DTE).abs())
        .map(|(exp, _)| exp.clone())
        .ok_or_else(|| {
            ProviderError::Transport(format!("no parseable expirations for {symbol}"))
        })?;

    // Contracts at that expiration with a usable IV.
    let candidates: Vec<&OptionContract> = chain
        .results
        .iter()
        .filter(|c| c.details.expiration_date == target_exp)
        .filter(|c| {
            c.implied_volatility
                .is_some_and(|iv| iv.is_finite() && iv > 0.0)
        })
        .collect();
    if candidates.is_empty() {
        return Err(ProviderError::NotFound(format!(
            "no usable IV near 30 DTE for {symbol}"
        )));
    }

    // The strike nearest the money…
    let nearest_strike = candidates
        .iter()
        .min_by(|a, b| {
            (a.details.strike_price - underlying)
                .abs()
                .partial_cmp(&(b.details.strike_price - underlying).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or(underlying, |c| c.details.strike_price);

    // …averaging whatever IV is present there (the call+put pair).
    let ivs: Vec<f64> = candidates
        .iter()
        .filter(|c| (c.details.strike_price - nearest_strike).abs() < 1e-9)
        .filter_map(|c| c.implied_volatility)
        .collect();
    if ivs.is_empty() {
        return Err(ProviderError::NotFound(format!(
            "no IV at ATM strike for {symbol}"
        )));
    }
    Ok(ivs.iter().sum::<f64>() / ivs.len() as f64)
}

// --- Calendar helpers (pure; decision 001 chose a tiny civil-date routine over a time crate) ---

/// Days-to-expiry for a `YYYY-MM-DD` expiration relative to `today_days` (epoch days).
fn dte(expiration: &str, today_days: i64) -> Option<i64> {
    let (y, m, d) = parse_ymd(expiration)?;
    Some(days_from_civil(y, m, d) - today_days)
}

/// Parse `YYYY-MM-DD` into `(year, month, day)`; `None` on any malformed field.
fn parse_ymd(s: &str) -> Option<(i64, i64, i64)> {
    let mut it = s.split('-');
    let y = it.next()?.parse().ok()?;
    let m = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    Some((y, m, d))
}

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`]: epoch-day number → `(year, month, day)`.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Current Unix time in seconds (0 before the epoch — unreachable in practice).
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Current Unix time in milliseconds.
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use exub_core::closes;

    #[test]
    fn civil_date_roundtrips_and_matches_known_epochs() {
        // Anchor days: 1970-01-01 = 0, 2000-01-01 = 10957, 2021-01-01 = 18628.
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 1, 1), 10_957);
        assert_eq!(days_from_civil(2021, 1, 1), 18_628);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(18_628), (2021, 1, 1));
        // Round-trip a spread of dates.
        for z in [-1, 1, 12_000, 20_000, 25_000] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
    }

    #[test]
    fn dte_measures_calendar_distance() {
        let today = days_from_civil(2024, 3, 1);
        assert_eq!(dte("2024-03-01", today), Some(0));
        assert_eq!(dte("2024-03-31", today), Some(30));
        assert_eq!(parse_ymd("2024-13"), None); // malformed
        assert_eq!(dte("not-a-date", today), None);
    }

    fn agg(t: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> AggBar {
        AggBar { t, o, h, l, c, v }
    }

    #[test]
    fn map_aggs_converts_millis_and_orders() {
        // Deliberately out of order with a duplicate timestamp.
        let resp = AggsResponse {
            results: vec![
                agg(2_000, 11.0, 12.0, 10.0, 11.5, 100.0),
                agg(1_000, 10.0, 11.0, 9.0, 10.5, 50.0),
                agg(2_000, 11.0, 12.0, 10.0, 11.9, 100.0), // dup t → dropped after sort
            ],
        };
        let bars = map_aggs(resp, 100, "AAA").unwrap();
        assert_eq!(bars.len(), 2);
        // millis → seconds, ascending.
        assert_eq!(bars[0].t, 1);
        assert_eq!(bars[1].t, 2);
        assert_eq!(closes(&bars), vec![10.5, 11.5]);
    }

    #[test]
    fn map_aggs_drops_bad_ticks_and_errors_when_empty() {
        let resp = AggsResponse {
            results: vec![
                agg(1_000, 0.0, 1.0, 1.0, 1.0, 10.0),    // non-positive open
                agg(2_000, 10.0, 9.0, 11.0, 10.0, 10.0), // high < low
                agg(3_000, 10.0, 11.0, 9.0, 10.0, -1.0), // negative volume
            ],
        };
        assert!(matches!(
            map_aggs(resp, 100, "BAD"),
            Err(ProviderError::NotFound(_))
        ));
    }

    #[test]
    fn map_aggs_trims_to_lookback() {
        let resp = AggsResponse {
            results: (1..=10)
                .map(|i| agg(i * 1_000, 10.0, 11.0, 9.0, 10.0 + i as f64, 1.0))
                .collect(),
        };
        let bars = map_aggs(resp, 3, "AAA").unwrap();
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0].t, 8); // most recent three
    }

    fn contract(
        exp: &str,
        strike: f64,
        iv: Option<f64>,
        underlying: Option<f64>,
    ) -> OptionContract {
        OptionContract {
            implied_volatility: iv,
            details: ContractDetails {
                strike_price: strike,
                expiration_date: exp.to_string(),
            },
            underlying_asset: underlying.map(|price| UnderlyingAsset { price: Some(price) }),
        }
    }

    #[test]
    fn select_atm_iv_picks_nearest_dte_strike_and_averages() {
        let now = days_from_civil(2024, 3, 1) * SECS_PER_DAY;
        let chain = ChainResponse {
            results: vec![
                // A far expiration (should lose to the ~30 DTE one).
                contract("2024-06-01", 100.0, Some(0.99), Some(101.0)),
                // ~30 DTE band; underlying ~101, so strike 100 is ATM.
                contract("2024-03-31", 100.0, Some(0.30), Some(101.0)), // call
                contract("2024-03-31", 100.0, Some(0.40), Some(101.0)), // put → avg 0.35
                contract("2024-03-31", 120.0, Some(0.80), Some(101.0)), // far OTM, ignored
            ],
        };
        let iv = select_atm_iv(&chain, now, "AAA").unwrap();
        assert!((iv - 0.35).abs() < 1e-9, "got {iv}");
    }

    #[test]
    fn select_atm_iv_typed_errors() {
        let now = 0;
        assert!(matches!(
            select_atm_iv(&ChainResponse { results: vec![] }, now, "X"),
            Err(ProviderError::NotFound(_))
        ));
        // No underlying price → Transport.
        let no_px = ChainResponse {
            results: vec![contract("2024-03-31", 100.0, Some(0.3), None)],
        };
        assert!(matches!(
            select_atm_iv(&no_px, now, "X"),
            Err(ProviderError::Transport(_))
        ));
    }
}
