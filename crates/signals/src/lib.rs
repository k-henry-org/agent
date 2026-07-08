//! Screeners for `exuberance`.
//!
//! A screen turns raw market data into a pass/fail verdict plus the numbers that
//! justify it. The flagship is the cheap-vol screen ([`scan`]): find names where implied vol
//! is cheap **for that name** and below what the stock has actually been doing,
//! filtered to underlyings with a proven history of large moves.

use exub_core::{closes, MarketDataProvider, ProviderError};

/// Entry criteria for the cheap-vol / proven-mover setup.
///
/// Defaults encode the strategy discussed: IV in the bottom fifth of its own
/// range, realized vol at least matching implied, and at least a couple of
/// ≥10% single-day moves over the lookback window.
#[derive(Debug, Clone, Copy)]
pub struct CheapVolCriteria {
    /// Max IV rank to qualify (0.0–1.0). Lower = cheaper vs. the name's own history.
    pub max_iv_rank: f64,
    /// Minimum realized/implied ratio. `1.0` means realized must at least match implied.
    pub min_realized_over_implied: f64,
    /// A move counts as "big" if its absolute daily return meets this (decimal).
    pub big_move_threshold: f64,
    /// Require at least this many big moves in the lookback window.
    pub min_big_moves: usize,
    /// How many daily bars to pull for realized-vol and move detection.
    pub lookback_days: usize,
}

impl Default for CheapVolCriteria {
    fn default() -> Self {
        Self {
            max_iv_rank: 0.20,
            min_realized_over_implied: 1.0,
            big_move_threshold: 0.10,
            min_big_moves: 2,
            lookback_days: 756, // ~3 years of trading days
        }
    }
}

/// The computed evidence for one symbol against [`CheapVolCriteria`]. A cited evidence record, not a
/// verdict. `#[non_exhaustive]` so fields can be added (new metrics) without breaking downstream readers.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CheapVolResult {
    pub symbol: String,
    pub iv: f64,
    pub iv_rank: Option<f64>,
    /// Annualized realized vol. `None` when the price history is too short to compute it —
    /// a value we can't compute is never reported as a fake zero.
    pub realized_vol: Option<f64>,
    /// realized / implied. `> 1.0` means the stock moved more than options imply.
    pub realized_over_implied: Option<f64>,
    /// implied − realized. Negative == options underpricing movement. `None` when realized
    /// vol is not computable.
    pub implied_realized_spread: Option<f64>,
    pub big_moves: usize,
    pub max_move: f64,
    pub passed: bool,
    /// Human-readable reasons a symbol failed (empty when it passed).
    pub fail_reasons: Vec<String>,
}

/// Evaluate a single symbol. Returns the full evidence, whether it passed or not.
///
/// Generic over `?Sized` so it accepts both a concrete provider and a
/// `&dyn MarketDataProvider` handed out by the runtime registry.
pub async fn evaluate<S: MarketDataProvider + ?Sized>(
    source: &S,
    symbol: &str,
    c: &CheapVolCriteria,
) -> Result<CheapVolResult, ProviderError> {
    let bars = source.daily_bars(symbol, c.lookback_days).await?;
    let snap = source.iv_snapshot(symbol).await?;
    let prices = closes(&bars);

    // `None` when the history is too short — the evidence stays honest rather than
    // reporting a fabricated 0.0 realized vol (grounded invariant: the engine authors
    // the number, and a number it can't compute is `None`, never a fake).
    let realized = vol::realized_vol_daily(&prices);
    let rank = vol::iv_rank(snap.iv, &snap.iv_history);
    let roi = realized.and_then(|rv| vol::realized_over_implied(rv, snap.iv));
    let spread = realized.map(|rv| vol::implied_realized_spread(snap.iv, rv));
    let big_moves = vol::count_moves_over(&prices, c.big_move_threshold);
    let max_move = vol::max_abs_move(&prices).unwrap_or(0.0);

    let mut fail_reasons = Vec::new();
    match rank {
        Some(r) if r <= c.max_iv_rank => {}
        Some(r) => fail_reasons.push(format!("IV rank {r:.2} > max {:.2}", c.max_iv_rank)),
        None => fail_reasons.push("no IV history to rank".into()),
    }
    match (realized, roi) {
        // The honest reason: without realized vol there is no ratio to judge, so don't
        // misattribute the failure to "realized/implied too low".
        (None, _) => fail_reasons.push(format!(
            "insufficient price history to compute realized vol ({} bars, need ≥ 3)",
            bars.len()
        )),
        (Some(_), Some(r)) if r >= c.min_realized_over_implied => {}
        (Some(_), Some(r)) => fail_reasons.push(format!(
            "realized/implied {r:.2} < min {:.2}",
            c.min_realized_over_implied
        )),
        (Some(_), None) => fail_reasons.push("implied vol non-positive".into()),
    }
    if big_moves < c.min_big_moves {
        fail_reasons.push(format!(
            "{big_moves} big moves (≥{:.0}%) < min {}",
            c.big_move_threshold * 100.0,
            c.min_big_moves
        ));
    }

    Ok(CheapVolResult {
        symbol: symbol.to_string(),
        iv: snap.iv,
        iv_rank: rank,
        realized_vol: realized,
        realized_over_implied: roi,
        implied_realized_spread: spread,
        big_moves,
        max_move,
        passed: fail_reasons.is_empty(),
        fail_reasons,
    })
}

/// Run the screen over a universe, returning **only the names that passed**,
/// sorted most-underpriced first (most negative implied−realized spread).
///
/// Symbols the data source can't serve are skipped (not fatal to the scan).
pub async fn scan<S: MarketDataProvider + ?Sized>(
    source: &S,
    universe: &[&str],
    c: &CheapVolCriteria,
) -> Vec<CheapVolResult> {
    let mut hits: Vec<CheapVolResult> = Vec::new();
    for sym in universe {
        // Symbols the source can't serve are skipped, not fatal to the scan.
        if let Ok(r) = evaluate(source, sym, c).await {
            if r.passed {
                hits.push(r);
            }
        }
    }
    // Passers always carry a spread (no realized vol → the roi criterion fails), but sort
    // defensively: a `None` spread sinks to the end rather than panicking.
    hits.sort_by(|a, b| {
        let sa = a.implied_realized_spread.unwrap_or(f64::INFINITY);
        let sb = b.implied_realized_spread.unwrap_or(f64::INFINITY);
        sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use market_data::MockSource;

    /// Build a price series with `n` gentle steps then a few explicit big moves,
    /// so realized vol is high and the move filter trips.
    fn mover_series() -> Vec<f64> {
        let mut p = vec![100.0];
        // three ±12% moves — proven mover, drives realized vol up
        for &m in &[1.12, 0.88, 1.12, 0.90, 1.11] {
            let last = *p.last().unwrap();
            p.push(last * m);
        }
        p
    }

    #[tokio::test]
    async fn cheap_vol_name_passes() {
        let mut src = MockSource::new();
        // IV of 20%, sitting near the bottom of a 15–60% history → low rank.
        src.insert_from_closes("MOVER", &mover_series(), 0.20, vec![0.15, 0.30, 0.45, 0.60]);
        let c = CheapVolCriteria {
            lookback_days: 100,
            ..Default::default()
        };
        let r = evaluate(&src, "MOVER", &c).await.unwrap();
        assert!(r.passed, "expected pass, reasons: {:?}", r.fail_reasons);
        assert!(r.iv_rank.unwrap() <= 0.20);
        assert!(r.realized_over_implied.unwrap() > 1.0);
        assert!(r.big_moves >= 2);
    }

    #[tokio::test]
    async fn expensive_iv_fails_on_rank() {
        let mut src = MockSource::new();
        // Same mover, but IV is pinned at the TOP of its history → high rank.
        src.insert_from_closes("MOVER", &mover_series(), 0.60, vec![0.15, 0.30, 0.60]);
        let c = CheapVolCriteria {
            lookback_days: 100,
            ..Default::default()
        };
        let r = evaluate(&src, "MOVER", &c).await.unwrap();
        assert!(!r.passed);
        assert!(r.fail_reasons.iter().any(|s| s.contains("IV rank")));
    }

    #[tokio::test]
    async fn sleepy_stock_fails_on_moves() {
        let mut src = MockSource::new();
        // Cheap IV rank, but the stock barely moves → fails the proven-mover gate.
        src.insert_from_closes(
            "SLEEPY",
            &[100.0, 100.5, 100.2, 100.7, 100.3],
            0.10,
            vec![0.10, 0.40, 0.80],
        );
        let c = CheapVolCriteria {
            lookback_days: 100,
            ..Default::default()
        };
        let r = evaluate(&src, "SLEEPY", &c).await.unwrap();
        assert!(!r.passed);
        assert!(r.fail_reasons.iter().any(|s| s.contains("big moves")));
    }

    /// The exit-gate property for Phase 3: the screen runs against the feed *purely
    /// through the trait*. Production hands `scan` a `Box<dyn MarketDataProvider>` from
    /// the registry (dynamic dispatch); this pins that path with a test, asserting the
    /// `&dyn` screen agrees with the concrete (monomorphized) one.
    #[tokio::test]
    async fn screen_runs_through_dyn_provider() {
        let mut src = MockSource::new();
        src.insert_from_closes("MOVER", &mover_series(), 0.20, vec![0.15, 0.60]);
        let c = CheapVolCriteria {
            lookback_days: 100,
            ..Default::default()
        };

        let concrete = evaluate(&src, "MOVER", &c).await.unwrap();

        let dyn_src: &dyn MarketDataProvider = &src;
        let via_dyn = evaluate(dyn_src, "MOVER", &c).await.unwrap();
        assert_eq!(via_dyn.passed, concrete.passed);
        assert_eq!(via_dyn.iv_rank, concrete.iv_rank);
        assert_eq!(via_dyn.realized_vol, concrete.realized_vol);

        let hits = scan(dyn_src, &["MOVER"], &c).await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].symbol, "MOVER");
    }

    #[tokio::test]
    async fn insufficient_history_fails_honestly() {
        let mut src = MockSource::new();
        // Two closes → one return → no sample std-dev → realized vol is not computable.
        src.insert_from_closes("THIN", &[100.0, 105.0], 0.20, vec![0.15, 0.30, 0.60]);
        let c = CheapVolCriteria {
            lookback_days: 100,
            ..Default::default()
        };
        let r = evaluate(&src, "THIN", &c).await.unwrap();
        assert!(!r.passed);
        // The un-computable values are honest `None`s, never fabricated zeros.
        assert_eq!(r.realized_vol, None);
        assert_eq!(r.implied_realized_spread, None);
        assert_eq!(r.realized_over_implied, None);
        // And the fail reason names the real cause, not a misattributed ratio failure.
        assert!(r
            .fail_reasons
            .iter()
            .any(|s| s.contains("insufficient price history")));
        assert!(!r
            .fail_reasons
            .iter()
            .any(|s| s.contains("realized/implied")));
    }

    #[tokio::test]
    async fn scan_sorts_most_underpriced_first() {
        let mut src = MockSource::new();
        // Same mover series → same realized vol; DEEP's lower IV makes its
        // implied−realized spread more negative, so it must sort first.
        src.insert_from_closes("DEEP", &mover_series(), 0.18, vec![0.15, 0.60]);
        src.insert_from_closes("MILD", &mover_series(), 0.22, vec![0.20, 0.80]);
        let c = CheapVolCriteria {
            lookback_days: 100,
            ..Default::default()
        };
        let hits = scan(&src, &["MILD", "DEEP"], &c).await;
        assert_eq!(hits.len(), 2, "both movers should pass");
        assert_eq!(hits[0].symbol, "DEEP");
        assert!(
            hits[0].implied_realized_spread.unwrap() < hits[1].implied_realized_spread.unwrap()
        );
    }

    #[tokio::test]
    async fn scan_returns_only_passers_sorted() {
        let mut src = MockSource::new();
        src.insert_from_closes("MOVER", &mover_series(), 0.20, vec![0.15, 0.60]);
        src.insert_from_closes("SLEEPY", &[100.0, 100.5, 100.2], 0.10, vec![0.10, 0.80]);
        let c = CheapVolCriteria {
            lookback_days: 100,
            ..Default::default()
        };
        let hits = scan(&src, &["MOVER", "SLEEPY", "MISSING"], &c).await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].symbol, "MOVER");
    }
}
