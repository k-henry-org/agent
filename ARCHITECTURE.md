# Architecture decisions

The record [`ROADMAP.md`](./ROADMAP.md) §0 references: every roadmap item tagged
`(decision)` produces a dated entry here — the decision, the alternatives considered, and
the why — so the reasoning outlives the diff. Entries are append-only and numbered;
reversing one is a new entry, not an edit. (Roadmap *re-scopes* — cut phases and why — are
recorded in the roadmap's tombstones, not duplicated here. P25.3 consolidates this file
before any release.)

---

## 001 — f64 for stats, decimal at the money edge (2026-07-08 · accepted)

**Roadmap item:** P6.2.

**Context.** The engine's numbers split into two families with different correctness
needs: *statistical* values (prices feeding log returns, standard deviations, realized
vol, IV ranks) where floating point is the standard and correct representation, and
*money* values (order prices, account balances) where exact decimal arithmetic matters
because rounding errors compound into real cents.

**Decision.**
- Prices, vols, returns, and every derived statistic stay **`f64`** end-to-end (`Bar`,
  `IvSnapshot`, the `vol` crate, `CheapVolResult`).
- **Exact decimal money belongs at an order/broker edge — which is cut by design**
  (ROADMAP Phases 19–22 tombstone: the engine places no orders). If execution is ever
  explicitly re-scoped, a decimal money type arrives with it as part of that re-scope's
  design; nothing in the discovery pipeline needs it.
- Timestamps stay **epoch-seconds UTC** (`Bar.t: i64`, market close). No typed time crate
  yet.

**Alternatives considered.**
- *`rust_decimal` (or similar) everywhere* — wrong tool for the statistical path
  (log/sqrt/stddev live in `f64`; converting at every math call adds cost and noise) and a
  pervasive dependency in crates that are deliberately dependency-free (`vol` has zero
  deps).
- *A typed time crate (`time`/`chrono`/`jiff`) now* — buys nothing while the only
  timestamps are daily-close markers from a mock. The real need appears with a live
  adapter's trading-calendar / session / timezone handling — **revisit inside Phase 7's
  data-correctness work (P7.2)**, where the cost is justified by actual session logic.

**Consequences.**
- `vol` stays pure and dependency-free; the whole discovery pipeline is `f64` with
  documented decimal semantics (0.30 == 30%; percent only at the display edge).
- `Bar.t`'s meaning (epoch seconds, UTC, market close) is documented on the type; any
  future granularity change is additive via the `#[non_exhaustive]` schema.
- Phase 7 owns the calendar/session/timezone story and is the checkpoint for the
  time-crate question.
