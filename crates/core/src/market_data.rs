//! Market-data domain types and the [`MarketDataProvider`] contract.
//!
//! These are the value types every price feed speaks, plus the trait screeners
//! depend on. The concrete feeds (mock, Massive, and later others) live in the
//! `market-data` crate and implement this trait — so `signals` never names a
//! vendor, only the contract.

use async_trait::async_trait;

use crate::error::ProviderResult;
use crate::provider::{Capability, Provider};

/// A daily OHLCV bar. Timestamps are Unix epoch **seconds** (UTC, market close).
///
/// Prices are `f64`: this is *statistical* vol input (log returns, stddev, ranks), where floating point
/// is the correct and standard representation. Exact decimal money lives at the order/broker edge, not
/// here — see the money-vs-stats note in the Phase 6 roadmap. `#[non_exhaustive]` for additive evolution;
/// construct with [`Bar::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Bar {
    pub t: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Bar {
    /// Build a bar (needed because `#[non_exhaustive]` forbids struct-literal construction downstream).
    #[must_use]
    pub fn new(t: i64, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self {
            t,
            open,
            high,
            low,
            close,
            volume,
        }
    }
}

/// An implied-volatility snapshot for a symbol, with enough history to rank it.
///
/// `iv` and each entry in `iv_history` are decimals (0.30 == 30% annualized),
/// typically the ATM / 30-day implied vol. `#[non_exhaustive]`; construct with [`IvSnapshot::new`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct IvSnapshot {
    pub symbol: String,
    pub iv: f64,
    /// Trailing IV observations (e.g. daily ATM IV over the last 1–3 years),
    /// used to compute IV rank / percentile.
    pub iv_history: Vec<f64>,
}

impl IvSnapshot {
    /// Build an IV snapshot (needed because `#[non_exhaustive]` forbids downstream struct literals).
    #[must_use]
    pub fn new(symbol: impl Into<String>, iv: f64, iv_history: Vec<f64>) -> Self {
        Self {
            symbol: symbol.into(),
            iv,
            iv_history,
        }
    }
}

/// Anything that can supply prices and IV. Screeners depend on this, not on a
/// concrete vendor, so tests use a mock and prod uses Massive, Alpha
/// Vantage, or any other feed.
///
/// The methods are `async` because a real feed is a network round-trip;
/// `#[async_trait]` keeps the trait object-safe so the engine can hold a
/// `Box<dyn MarketDataProvider>` selected at runtime from config.
#[async_trait]
pub trait MarketDataProvider: Provider {
    /// Daily bars, oldest-first, covering roughly `lookback_days` sessions.
    async fn daily_bars(&self, symbol: &str, lookback_days: usize) -> ProviderResult<Vec<Bar>>;

    /// Current IV plus trailing history for the symbol.
    async fn iv_snapshot(&self, symbol: &str) -> ProviderResult<IvSnapshot>;
}

/// Convenience: pull the closing prices out of a bar series (oldest-first).
pub fn closes(bars: &[Bar]) -> Vec<f64> {
    bars.iter().map(|b| b.close).collect()
}

/// How the engine obtains the trailing IV distribution that `iv_rank` needs, given what a provider can
/// serve. This is the crux of staying vendor-agnostic on IV history: the acquisition strategy is a function
/// of the provider's [`Capability`], not a hard-coded assumption about one feed. (A live IV snapshot — what
/// a vendor's MCP typically exposes — can't give you the 1–3yr *series*, which is exactly why the engine
/// owns this; see the "why this exists" note in the README.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IvHistoryStrategy {
    /// The provider serves historical option chains with IV (e.g. Alpha Vantage's historical options) —
    /// **backfill** the distribution in a bounded batch of calls.
    Backfill,
    /// The provider serves only a live IV snapshot (e.g. a snapshot-only feed) — **accumulate** the
    /// distribution forward over time and persist it to the store.
    Accumulate,
}

/// Choose the IV-history strategy from a provider's capabilities: [`Backfill`](IvHistoryStrategy::Backfill)
/// when it advertises [`Capability::OptionsHistory`], otherwise
/// [`Accumulate`](IvHistoryStrategy::Accumulate). Generic over `?Sized` so it accepts a boxed `&dyn`
/// provider from the registry.
#[must_use]
pub fn iv_history_strategy<P: Provider + ?Sized>(provider: &P) -> IvHistoryStrategy {
    if provider.supports(Capability::OptionsHistory) {
        IvHistoryStrategy::Backfill
    } else {
        IvHistoryStrategy::Accumulate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ProviderInfo, ProviderKind};

    /// A stand-in feed advertising a chosen capability set, to test the strategy selector.
    struct Feed {
        caps: Vec<Capability>,
    }
    impl Provider for Feed {
        fn info(&self) -> ProviderInfo {
            ProviderInfo {
                id: "feed".into(),
                kind: ProviderKind::MarketData,
                capabilities: self.caps.clone(),
            }
        }
    }

    #[test]
    fn iv_history_strategy_follows_capabilities() {
        // A feed that serves historical option chains (Alpha-Vantage-like) → backfill.
        let historical = Feed {
            caps: vec![Capability::ImpliedVol, Capability::OptionsHistory],
        };
        // A snapshot-only feed → accumulate forward.
        let snapshot = Feed {
            caps: vec![Capability::ImpliedVol],
        };
        assert_eq!(
            iv_history_strategy(&historical),
            IvHistoryStrategy::Backfill
        );
        assert_eq!(
            iv_history_strategy(&snapshot),
            IvHistoryStrategy::Accumulate
        );
    }

    #[test]
    fn closes_extracts_close_series() {
        // Via the constructor even in-crate (where `#[non_exhaustive]` wouldn't force it):
        // the P6.1 exit-gate proof showed struct literals here are the only call sites a
        // new field would break — route them through `::new` so additive evolution is free.
        let bars = vec![
            Bar::new(0, 1.0, 1.0, 1.0, 10.0, 0.0),
            Bar::new(1, 1.0, 1.0, 1.0, 11.0, 0.0),
        ];
        assert_eq!(closes(&bars), vec![10.0, 11.0]);
    }
}
