//! Market-data **providers** for `exuberance`.
//!
//! The types and the [`MarketDataProvider`] contract live in `exub-core`; this
//! crate supplies concrete feeds that implement it — [`MockSource`] for tests and
//! demos, [`MassiveSource`] for live data (stub until the HTTP path lands). The
//! core types are re-exported here so downstream crates can keep a single import.
//!
//! Live market data also reaches the AI agents through the `massive` and Massive
//! **MCP tools**; this crate is the *Rust* path used by the compiled engine.

use async_trait::async_trait;

pub use exub_core::{
    closes, Bar, Capability, IvSnapshot, MarketDataProvider, Provider, ProviderError, ProviderInfo,
    ProviderKind, ProviderResult,
};

/// In-memory data source for tests, demos, and backtest fixtures.
#[derive(Debug, Default, Clone)]
pub struct MockSource {
    entries: std::collections::HashMap<String, (Vec<Bar>, IvSnapshot)>,
}

impl MockSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// A `MockSource` pre-seeded with a small demo universe — a proven mover at cheap IV (surfaces), a
    /// sleepy name (fails the move gate), and a big mover at rich IV (fails on rank). Used by `exub scan`'s
    /// demo path so the pipeline is visible before a live feed is wired, and by tests as a fixture.
    pub fn demo() -> Self {
        let mut src = Self::new();
        src.insert_from_closes(
            "MOVER",
            &[100.0, 112.0, 98.5, 110.3, 99.3, 110.2],
            0.22,
            vec![0.18, 0.35, 0.52, 0.60],
        );
        src.insert_from_closes(
            "SLEEPY",
            &[100.0, 100.4, 100.1, 100.5, 100.2],
            0.11,
            vec![0.10, 0.45, 0.80],
        );
        src.insert_from_closes(
            "PRICEY",
            &[100.0, 112.0, 98.0, 111.0, 99.0],
            0.58,
            vec![0.15, 0.30, 0.60],
        );
        src
    }

    /// The tickers [`MockSource::demo`] seeds, in scan order.
    pub const DEMO_UNIVERSE: [&'static str; 3] = ["MOVER", "SLEEPY", "PRICEY"];

    /// Register a symbol from raw closing prices and an IV snapshot. Bars are
    /// synthesized with `close == open == high == low` and sequential timestamps.
    pub fn insert_from_closes(
        &mut self,
        symbol: &str,
        closes: &[f64],
        iv: f64,
        iv_history: Vec<f64>,
    ) {
        let bars = closes
            .iter()
            .enumerate()
            .map(|(i, &c)| Bar::new(i as i64, c, c, c, c, 0.0))
            .collect();
        let snap = IvSnapshot::new(symbol, iv, iv_history);
        self.entries.insert(symbol.to_string(), (bars, snap));
    }
}

impl Provider for MockSource {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "mock".into(),
            kind: ProviderKind::MarketData,
            capabilities: vec![Capability::DailyBars, Capability::ImpliedVol],
        }
    }
}

#[async_trait]
impl MarketDataProvider for MockSource {
    async fn daily_bars(&self, symbol: &str, lookback_days: usize) -> ProviderResult<Vec<Bar>> {
        let (bars, _) = self
            .entries
            .get(symbol)
            .ok_or_else(|| ProviderError::NotFound(symbol.to_string()))?;
        let start = bars.len().saturating_sub(lookback_days);
        Ok(bars[start..].to_vec())
    }

    async fn iv_snapshot(&self, symbol: &str) -> ProviderResult<IvSnapshot> {
        self.entries
            .get(symbol)
            .map(|(_, snap)| snap.clone())
            .ok_or_else(|| ProviderError::NotFound(symbol.to_string()))
    }
}

/// Live Massive data source. Skeleton only — HTTP wiring is the next
/// milestone (see CLAUDE.md "Build order"). Reads `MASSIVE_API_KEY` from env.
#[derive(Debug, Clone)]
pub struct MassiveSource {
    #[allow(dead_code)]
    api_key: String,
}

impl MassiveSource {
    /// Construct from the `MASSIVE_API_KEY` environment variable.
    pub fn from_env() -> ProviderResult<Self> {
        let api_key = std::env::var("MASSIVE_API_KEY")
            .map_err(|_| ProviderError::Auth("MASSIVE_API_KEY not set".into()))?;
        Ok(Self { api_key })
    }
}

impl Provider for MassiveSource {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "massive".into(),
            kind: ProviderKind::MarketData,
            capabilities: vec![
                Capability::DailyBars,
                Capability::IntradayBars,
                Capability::Quotes,
                Capability::OptionsChain,
                Capability::ImpliedVol,
            ],
        }
    }
}

#[async_trait]
impl MarketDataProvider for MassiveSource {
    async fn daily_bars(&self, _symbol: &str, _lookback_days: usize) -> ProviderResult<Vec<Bar>> {
        // TODO: GET /v2/aggs/ticker/{symbol}/range/1/day/{from}/{to}
        Err(ProviderError::NotImplemented("MassiveSource::daily_bars"))
    }

    async fn iv_snapshot(&self, _symbol: &str) -> ProviderResult<IvSnapshot> {
        // TODO: pull ATM IV from the options snapshot endpoint + build history.
        Err(ProviderError::NotImplemented("MassiveSource::iv_snapshot"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_roundtrips_closes_and_iv() {
        let mut src = MockSource::new();
        src.insert_from_closes("AAA", &[10.0, 11.0, 12.0], 0.25, vec![0.2, 0.3, 0.4]);

        let bars = src.daily_bars("AAA", 100).await.unwrap();
        assert_eq!(closes(&bars), vec![10.0, 11.0, 12.0]);

        let snap = src.iv_snapshot("AAA").await.unwrap();
        assert_eq!(snap.iv, 0.25);
        assert_eq!(snap.iv_history.len(), 3);

        assert_eq!(
            src.daily_bars("ZZZ", 10).await,
            Err(ProviderError::NotFound("ZZZ".into()))
        );
    }

    #[tokio::test]
    async fn lookback_truncates_to_most_recent() {
        let mut src = MockSource::new();
        src.insert_from_closes("BBB", &[1.0, 2.0, 3.0, 4.0, 5.0], 0.2, vec![]);
        let bars = src.daily_bars("BBB", 2).await.unwrap();
        assert_eq!(closes(&bars), vec![4.0, 5.0]);
    }

    #[tokio::test]
    async fn demo_universe_is_seeded_and_scannable() {
        let src = MockSource::demo();
        for sym in MockSource::DEMO_UNIVERSE {
            assert!(
                src.daily_bars(sym, 100).await.is_ok(),
                "{sym} should be seeded"
            );
        }
    }

    #[test]
    fn mock_advertises_capabilities() {
        let src = MockSource::new();
        assert!(src.supports(Capability::DailyBars));
        assert!(src.supports(Capability::ImpliedVol));
        assert!(!src.supports(Capability::OptionsChain));
        assert_eq!(src.info().kind, ProviderKind::MarketData);
    }

    /// One test fn for every `MassiveSource` path that exists today, because they all
    /// touch the process-global `MASSIVE_API_KEY` env var — a single sequential fn
    /// avoids races with cargo's parallel test threads (no other test reads it).
    #[tokio::test]
    async fn massive_stub_auth_and_not_implemented() {
        // No key → a typed Auth error, not a panic or a silent default.
        std::env::remove_var("MASSIVE_API_KEY");
        assert_eq!(
            MassiveSource::from_env().err(),
            Some(ProviderError::Auth("MASSIVE_API_KEY not set".into()))
        );

        // With a key the stub constructs, advertises its card, and returns honest
        // NotImplemented from both trait methods (never fabricated data).
        std::env::set_var("MASSIVE_API_KEY", "test-key-not-real");
        let src = MassiveSource::from_env().expect("key is set");
        assert_eq!(src.info().id, "massive");
        assert_eq!(
            src.daily_bars("SPY", 10).await,
            Err(ProviderError::NotImplemented("MassiveSource::daily_bars"))
        );
        assert_eq!(
            src.iv_snapshot("SPY").await,
            Err(ProviderError::NotImplemented("MassiveSource::iv_snapshot"))
        );
        // Snapshot-only card (no OptionsHistory) → the engine must ACCUMULATE its IV
        // history forward (Phase 8) — the capability-driven strategy, pinned on a real
        // vendor's card rather than a synthetic test double.
        assert!(!src.supports(Capability::OptionsHistory));
        assert_eq!(
            exub_core::iv_history_strategy(&src),
            exub_core::IvHistoryStrategy::Accumulate
        );

        std::env::remove_var("MASSIVE_API_KEY");
    }
}
