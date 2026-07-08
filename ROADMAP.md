# Roadmap

## §0 The spine

**`exuberance` is a grounded trade-*discovery* engine (Rust).** It helps a discretionary
trader *find* trades — across **any market and any strategy**. You ask (say) where implied
volatility is cheap on a name with a proven ability to move — or wherever your strategy sees
an edge — and the engine surfaces candidates plus the evidence that justifies them, grounded
in real data with its provenance, so *you* decide. Decision **support**, never advice, never
autonomous execution.

The shape is **ports & adapters** (hexagonal): a headless **engine** drives one external
I/O port — `MarketDataProvider` — and runs a pluggable **`Strategy`** (the second agnostic
seam; internal, not I/O) over canonical data. The `AiProvider` and `BrokerProvider` traits
exist in `exub-core` as **designed-but-dormant contracts**: intelligence connects from the
*outside* over MCP — agents bring their own LLM, the engine contains **no LLM code** — and
execution is **cut by design** (the Phases 19–22 tombstone: the engine places no orders,
ever). Every **surface** (the `exub` CLI now; the MCP server later) only *renders* what the
engine found — no surface calls a feed or a strategy directly.
The cheap-vol / proven-mover screen is the **flagship reference strategy**, not the product:
momentum, mean-reversion, breakout — over equities, futures, crypto, FX — plug into the same
seam. The operating manual is [`.rules`](./.rules); hard-to-reverse decisions land in
`ARCHITECTURE.md` as they're made.

Three keystones hold it up:

1. **Agnostic by trait, never by vendor — because lock-in caps the search.** The goal is
   the most *efficient* way to find trades, whatever they are — and that search must stay
   free to swap, combine, and benchmark data feeds and models head-to-head as better,
   cheaper, or faster ones appear. So: a new feed, model, broker, or strategy is a **new
   adapter behind a trait and nothing else**; the core depends on none of them. Adapters
   are feature-gated so a lean build compiles without every vendor SDK. The registry is the
   one place a vendor is named: config resolves a name → a boxed adapter; capabilities are
   declared (`Capability`), and the engine only plans a query a provider can answer. Because
   every vendor answers the same seam, the same question can be run over two feeds and the
   results *compared* — the backtest (Phase 13) and evals (Phase 14) turn "which provider
   finds trades best" into a measurement, not an allegiance. And because intelligence
   connects over MCP instead of living in the engine, the LLM is the most swappable piece
   of all: any agent, any model, zero engine changes and zero model keys. If a change makes
   the core name a vendor, the design is wrong.
2. **Grounded discovery, never advice.** The engine answers from data it actually fetched,
   **authors every figure itself** (IV rank, realized/implied, move counts — never the LLM),
   and reports provenance: metric, value, the exact bars/IV history used, the provider. It
   surfaces *candidates + evidence*, never a verdict, and no layer ever emits "buy this."
   The guardrails are structural, not convention: the engine contains **no execution path**
   (no order code to any real venue exists — the `PaperBroker` is an inert reference mock)
   and **no LLM code** — it cannot act, cannot recommend on a model's behalf, and cannot
   hallucinate a number, *because the code isn't there*. Secrets never enter the repo.
3. **The differentiator is what the engine computes and keeps — never raw access.** Massive
   (and others) already expose raw market data to agents over MCP, so raw access is not a
   reason to build. The engine earns its existence three ways: **(a)** deterministic, cited
   computation an LLM can't be trusted to do; **(b)** accumulated state no snapshot returns —
   the flagship needs a 1–3yr IV *series* to rank against, which no snapshot call provides,
   so the engine persists it (Phase 8) and the signal is computable *only because the engine
   exists*; **(c)** becoming a **higher-order MCP** (Phase 17) that serves grounded, cited
   signals to Claude Code / Gemini CLI / Codex — the mirror of a vendor's raw-data MCP. A
   live feed (Phase 7) is *plumbing* whose value is realized by Phases 8 + 17; if a change
   turns the engine into a thin passthrough of a vendor's data, it has lost its reason to
   exist.

The discipline test for every step: *"does this make discovery more grounded — more computed
by the engine, better cited, more strategy- and vendor-agnostic — without recommending,
acting, or naming a vendor in the core?"* If no, it sinks to a later phase — or out entirely:
this roadmap was **deliberately cut down** (2026-07) to the smallest set of phases that
reaches both reasons to exist; the tombstones (2, 15–16, 19–22) record what was cut and why.
**The mock feed keeps every phase keyless and offline-testable.** We do not start a phase
until the one before it is green on the gate.

---

## §0.5 How to work this roadmap (the agent loop)

This file is the **single source of truth for progress**. The checkboxes are the state; no
other tracker exists. **Every box below is intentionally unchecked: the repo predates this
rewrite, and earlier iterations built much of Phases 1–6. The reset is deliberate — re-verify
prior work instead of trusting it. Early iterations are audits, not rebuilds.** Work it as a
loop:

1. **Locate.** The current item is the first unchecked box in the lowest-numbered phase with
   unchecked boxes. Work strictly in ID order (`P3.2` before `P3.3`) unless a box says
   otherwise.
2. **Verify before building.** An item may already be partially or fully satisfied by
   existing code. Audit the codebase against the item first; if it's genuinely done, run the
   gate and check the box (that *is* the iteration). Never rebuild what exists; close the
   gap instead.
3. **Implement exactly the item.** One item ≈ one iteration ≈ one reviewable change. Don't
   reach ahead into later items "while you're in there" — that's how phases bleed together.
4. **Gate.** `cargo xtask ci` must be green before an item is done (fmt · clippy
   `-D warnings` · build · test · docs · feature powerset · `cargo-deny` — all keyless and
   offline). An item whose box mentions a test or doc isn't done until that test/doc exists.
5. **Check the box in the same commit as the work**, and reference the ID in the commit
   message (e.g. `P8.2: persist ATM IV behind the Store trait`). A checked box with no
   commit behind it is a lie; a landed change with an unchecked box is invisible.
6. **Advance.** A phase is done only when its **Exit gate** line passes end-to-end. Never
   start phase N+1 before phase N's exit gate is green.

**Epics.** An item tagged `(epic)` is too big for one iteration: before implementing, expand
it in-place into lettered sub-boxes (`P8.2a`, `P8.2b`, …) sized to one iteration each — that
expansion is itself one iteration — then work the sub-boxes.

**Decision items** are tagged `(decision)`: they produce a dated entry in `ARCHITECTURE.md`
(the decision, the alternatives, the why) and get checked when it's merged.

**When the map is wrong.** If an item turns out to be obsolete, mis-scoped, or blocked by a
decision above your pay grade: don't silently skip it and don't silently do something else.
Edit this file (reword / split / move the item) in its own commit with a one-line rationale,
or stop and ask. The roadmap must always describe reality. (Phase 2 below is exactly such an
edit, kept as a tombstone.)

---

## Phase 1 — Vol math + screen framework + CLI demo
Goal: the flagship strategy's math and screen, provable offline — pure vol primitives, the
cheap-vol / proven-mover screen, and a CLI demo so the whole pipeline is visible end-to-end
before any live data exists.

- [x] **P1.1** Pure vol math in `vol`: log/simple returns, sample std-dev, annualized
      realized vol, IV rank + IV percentile, implied−realized spread, realized/implied
      ratio, max-move + big-move counting. Deterministic, dependency-free, offline-testable;
      a known-answer test for every function.
- [x] **P1.2** The cheap-vol / proven-mover screen in `signals`, built over the market-data
      trait *(formalized in Phase 3 — the two phases landed together)*: `evaluate` returns
      the full evidence record with human-readable `fail_reasons` (failure is a value, never
      a panic); `scan` returns only passers, sorted most-underpriced first, skipping symbols
      the source can't serve.
- [x] **P1.3** `exub scan` demo over a synthetic universe — cited evidence rendered
      end-to-end, keyless and offline, framed as evidence-not-advice.

**Exit gate:** all P1 boxes checked · `cargo xtask ci` green · `exub scan` renders the demo
screen offline with no API keys.

## Phase 2 — AI layer (subagents + skills) — dropped
A tombstone, not work. The `.claude/` subagent squad + slash-command skills were **removed**:
harness-specific convenience, not part of the defensible engine. Their value (the research →
thesis → risk → adversarial-review *process*) lives **agent-side**, orchestrated over the
engine's MCP tools (Phase 17); the deterministic Finding→structure step is Phase 18. The
`#2` slot is kept only so later phase numbers — and the cross-references in code, docs, and
commit history that cite them — don't shift. Do not recreate them as `.claude/` files; put
the value in the engine.

**Exit gate:** none — nothing to verify. Proceed to Phase 3.

## Phase 3 — Provider-agnostic contracts
Goal: the contracts every adapter implements — the base provider/capability model, one error
vocabulary, and the three I/O seams — with reference impls proving them. Nothing
vendor-specific anywhere.

- [x] **P3.1** `exub-core`: the object-safe `Provider` base trait + `ProviderInfo` identity
      card + `ProviderKind` (MarketData / Broker / Ai / Agent) + the `Capability` vocabulary
      with `supports()` probing — screeners and the orchestrator branch on capability, never
      on a vendor name.
- [x] **P3.2** A unified `ProviderError` (NotFound / Auth / RateLimited / Transport /
      Unsupported / Refused / NotImplemented): every failed provider call is a typed value
      that degrades to a clear message.
- [x] **P3.3** The three I/O seams — `MarketDataProvider` (daily bars + IV snapshot; the
      one *active* port), plus `BrokerProvider` and `AiProvider` verified as
      **designed-but-dormant contracts**: no phase wires real vendors to them (see the
      Phase 15–16 and 19–22 tombstones). The other agnostic seam, `Strategy`, is internal
      and lands in Phase 11.
- [x] **P3.4** Reference impls proving the seams: `PaperBroker` + `EchoAi` in `exub-core`,
      the `MockSource` feed in `market-data` — and the dependency direction enforced:
      `market-data` + `signals` depend on the traits, never a concrete vendor.

**Exit gate:** all P3 boxes checked · `cargo xtask ci` green · the screen runs against
`MockSource` purely through the trait; a capability probe answers without a vendor `if`.

## Phase 4 — Config, CI & xtask scaffolding
Goal: the 12-factor + rigor substrate, before any real I/O — so every later phase inherits
the gate instead of relitigating discipline by hand.

- [x] **P4.1** Layered `Config` (**flags > env (`EXUB_*`) > file (TOML) > defaults**) with a
      pure `resolve()` fold and precedence pinned by unit tests; adapter selection + trading
      mode are config, not code; **secrets never enter the config** — provider-native env
      vars only (`MASSIVE_API_KEY`, `ALPHA_VANTAGE_API_KEY`, …), read at the adapter edge.
- [x] **P4.2** `tracing` logs to **stderr**, filtered by config; stdout reserved for data,
      so `exub scan 2>/dev/null` stays pipe-clean.
- [x] **P4.3** `cargo xtask ci` — the local gate: fmt · clippy `-D warnings` · build · test ·
      docs (`RUSTDOCFLAGS=-D warnings`) · feature powerset (`cargo-hack`) · `cargo-deny`,
      with `RUSTFLAGS=-D warnings` on every step, stopping at the first failure. Keyless.
- [x] **P4.4** A GitHub Actions workflow mirroring the gate step-for-step, plus one
      aggregate required status check so branch protection needs a single rule.

**Exit gate:** all P4 boxes checked · `cargo xtask ci` green locally **and** in CI with no
API keys · the precedence tests pin flags > env > file > defaults.

## Phase 5 — Async seams + adapter registry
Goal: the irreversible shape decision — async seams and the runtime registry — made while
mock-only (cheap now, expensive after a live adapter exists).

- [x] **P5.1** All three seams `async` (`async-trait`, driven by `tokio`); the engine holds
      `Box<dyn MarketDataProvider>` / `Box<dyn AiProvider>` selected at runtime; `signals`
      is generic over `S: MarketDataProvider + ?Sized` so it screens over a `&dyn` from the
      registry.
- [x] **P5.2** The **registry** — the one place a vendor is named: config name → boxed
      adapter (`build_data_provider` / `build_ai_provider`); selecting a planned-but-unwired
      vendor is a clear, actionable error, never a silent fallback.
- [x] **P5.3** The plug-in **catalog** (`exub providers`): every intended data feed with
      wired/planned status. The AI-model / coding-agent / broker entries document the
      **dormant seams** — permanently `planned` unless explicitly re-scoped (see the
      Phase 15–16 and 19–22 tombstones); agents connect over MCP instead.
- [x] **P5.4** Stream scan results to the surface as they're found (useful once real feeds
      make a universe scan slow); `--json` stays atomic. Streams `Finding`s, never model
      tokens.

**Exit gate:** all P5 boxes checked · `cargo xtask ci` green · `exub providers` renders the
catalog; a planned vendor errors actionably; the screen runs through a `Box<dyn …>` chosen
from config.

## Phase 6 — Canonical schema & error hardening
Goal: the canonical types hardened for additive evolution, and the numeric/time/error story
decided once — before real adapters multiply the cost of changing them.

- [x] **P6.1** Canonical types (`Bar`, `IvSnapshot`, `CheapVolResult`, `ProviderError`) are
      `#[non_exhaustive]` with constructors, so the schema evolves additively without
      breaking downstream matches or struct literals.
- [x] **P6.2** (decision) **f64 for stats, decimal at the money edge:** prices/vols stay
      `f64` — the correct representation for statistical vol math (log returns, stddev,
      ranks); exact decimal money would belong at an order/broker edge, which is **cut by
      design** (if execution is ever re-scoped, decimal money comes with it). Timestamps
      stay epoch-seconds **UTC** (market close); a typed time crate isn't worth the
      dependency yet.
- [x] **P6.3** `ProviderError` grown to the structured set incl.
      `RateLimited { retry_after }`; the **no-panic lint** (`unwrap_used` / `expect_used`
      denied outside tests via workspace lints + `clippy.toml`) — production code is
      panic-free by construction.

**Exit gate:** all P6 boxes checked · `cargo xtask ci` green · adding a field to a canonical
type compiles downstream crates without edits (the constructors + `#[non_exhaustive]` prove
it).

## Phase 7 — First real data adapter (EOD)
Goal: the first real feed, behind the same trait — **end-of-day only**. A discovery engine
needs a once-a-day fetch of daily bars + a daily IV observation; it never needs streaming,
websockets, or intraday data (real-time is an *execution* concern, and execution is cut).
**Plumbing, per §0 keystone 3** — on its own this is raw access (what a vendor's MCP already
offers); its payoff is Phases 8 + 17 built on top. Do not treat wiring it as the product.

- [x] **P7.1** `MassiveSource` behind a `massive-live` feature (default on; a lean
      `--no-default-features` build keeps a `NotImplemented` stub): **EOD** REST aggregates →
      `daily_bars`; end-of-day options snapshot → `iv_snapshot` (ATM / 30-day IV, history
      empty until Phase 8). Key from `MASSIVE_API_KEY` (env only, held in
      `secrecy::SecretString` at the adapter edge). `exub scan --symbols AAPL,…` is the
      minimal real-universe input; index/watchlist universes are Phase 10.
- [x] **P7.2** **Data correctness (the #1 bug class):** `adjusted=true` always requested
      (realized vol on unadjusted prices is *wrong*); bad-tick validation (non-positive/
      non-finite prices, high<low, negative volume dropped), ascending sort + duplicate-
      timestamp dedupe, `NotFound` when nothing valid survives. Calendar math (30-DTE
      selection, expiration-band query) via a pure civil-date routine — no time crate
      (decision 001); trading-calendar/session refinements land as real usage demands.
- [x] **P7.3** A shared HTTP module (`http.rs`): client with connect/total timeouts, bounded
      retry+backoff on 5xx/transport, and `Retry-After` parsed and surfaced as
      `ProviderError::RateLimited { retry_after }` (surfaced for the caller to honor, not
      blocked on mid-call). Key on the `Authorization` header, never the URL.
- [x] **P7.4** **Contract tests over synthetic fixtures** — a local `wiremock` server replays
      recorded shapes, so provider API drift fails CI, not a premarket scan: adjusted-query
      + header assertion, millis→secs mapping, 429→`RateLimited{retry_after}`, 401→`Auth`,
      ATM-IV selection. All offline; the real-network path only runs when a human sets a key.

**Exit gate:** all P7 boxes checked · `cargo xtask ci` green offline · with a real key set
locally, `exub scan --data-provider massive --symbols AAPL,TSLA,NVDA` fetches real EOD bars +
ATM IV and renders **cited evidence** (candidates need Phase 8's IV *history* to rank — until
then every name honestly fails "no IV history to rank").

## Phase 8 — IV history pipeline + storage seam
Goal: **the reason to exist** (§0 keystone 3b). IV rank needs 1–3 years of trailing IV that
no snapshot call returns; the engine persists it, making the flagship signal computable only
because the engine exists. Prioritize this directly behind the first feed.

- [ ] **P8.1** Acquisition is **capability-driven** via `Capability::OptionsHistory` + the
      `iv_history_strategy` selector: a feed serving historical options-with-IV (e.g. Alpha
      Vantage, ORATS) **backfills** the distribution in a bounded batch; a snapshot-only
      feed (e.g. Massive) **accumulates forward**. The screen is identical either way — the
      anti-corruption layer hides which feed filled the distribution.
- [ ] **P8.2** (epic) A `Store` trait (SQLite default, in-memory for tests): persist daily
      ATM IV per symbol; cache bars to respect rate limits; the same store later feeds the
      journal (Phase 23).
- [ ] **P8.3** Grounded output cites the exact history window used (span, observation
      count, source), and `iv_rank` ranks against the real persisted distribution — not a
      mock's.

**Exit gate:** all P8 boxes checked · `cargo xtask ci` green · two consecutive scans
accumulate IV into the store and the second cites a longer window; a backfill-capable feed
fills a 1yr+ distribution in one bounded run.

## Phase 9 — Second feed + caching / fan-out
Goal: prove the agnosticism claim with real second feeds — each chosen for the role it
fills — plus the caching/fan-out layer that brutal free-tier rate limits make mandatory.

- [ ] **P9.1** **Yahoo Finance (unofficial)** — the **keyless** real feed for onboarding
      (equities/ETFs, some FX/crypto); the best first-run after the mock since it needs no
      signup. Unofficial + ToS-gray → **synthetic fixtures only**, live path opt-in, never
      redistribute its data.
- [ ] **P9.2** An options/vol specialist — **ORATS (or Intrinio)** — advertises
      `Capability::OptionsHistory` and serves historical IV / vol surfaces, so it
      **backfills** the Phase-8 distribution directly instead of accumulating forward. The
      highest-leverage add for the flagship strategy.
- [ ] **P9.3** A caching / fan-out provider: try primary, fall back on error, cache reads to
      respect rate limits — essential, not optional, for a feed like **Alpha Vantage** (free
      tier on the order of ~25 requests/day; a universe scan is impossible without it).
      Respect each source's redistribution terms.
- [ ] **P9.4** An **events / catalysts capability** (earnings dates, corporate events) as a
      distinct `Capability` behind the provider seam — served by **Financial Modeling Prep
      (or Finnhub)**. Feeds the proven-mover context *and* the strategy-level
      earnings-proximity filter (P12.3).

*(Further candidates as coverage demands: Databento and Twelve Data for broad multi-asset
incl. futures/crypto/FX; Tradier/Alpaca/IBKR dual-use with their broker credentials. Skip
IEX Cloud — shut down 2024.)*

**Exit gate:** all P9 boxes checked · `cargo xtask ci` green · the same screen runs unchanged
over two real feeds; a rate-limited feed completes a universe scan through the cache.

## Phase 10 — Universe & symbology
Goal: scan real universes, not a hardcoded demo list — **equities + listed options first**;
other asset classes only when a strategy actually demands them.

- [ ] **P10.1** Real universe input for `exub scan`: index constituents, sector ETFs, and
      custom lists; liquidity pre-filters so illiquid names don't waste the request budget.
- [ ] **P10.2** A symbology layer normalizing equity tickers and option (OCC) symbols
      across providers.

**Deferred — not part of this phase's exit gate; pick up only when a concrete strategy
demands the asset class:**
- [ ] **P10.D1** Futures / crypto / FX symbology, and the canonical-schema extensions those
      assets need (futures roll / continuous contracts, crypto 24/7 sessions, FX without
      volume) — additively, via the Phase-6 `#[non_exhaustive]` types.

**Exit gate:** P10.1–P10.2 checked (P10.D* excluded) · `cargo xtask ci` green · a scan over
a named universe (e.g. an index's constituents) runs end-to-end with liquidity
pre-filtering.

## Phase 11 — Engine orchestrator + Strategy seam + cited `Finding`
Goal: where "all traders" becomes real — the headless `Engine` §0 promises, and the
`Strategy` trait that lets any strategy plug in the way a provider does. Cheap-vol becomes
the flagship *reference implementation*, not the hard-coded point.

- [ ] **P11.1** The **`Engine`** — the headless orchestrator: owns the runtime-selected data
      provider + the chosen `Strategy`, runs the screen/compute, and returns a cited
      result. The one object every surface drives — the CLI now, the MCP server (Phase 17).
      Today "the engine" is `signals::scan`; this makes it real, and it is the prerequisite
      for Phase 17.
- [ ] **P11.2** The **`Strategy` trait** (the second agnostic seam, alongside market data):
      canonical data in → cited **`Finding`s** out — symbol + strategy + the
      metrics that justify it + provenance. The cheap-vol screen becomes its first impl;
      `CheapVolResult` generalizes to `Finding`, so an equity setup, an option structure,
      or (later) a futures spread all surface the same way. **Design `Finding` as a wire
      type from day one:** it will be serialized by `--json` (P11.3), the MCP server
      (Phase 17), and any future programmatic surface — so extend the Phase-6 discipline
      (`#[non_exhaustive]` + constructors) with **serde-stable field naming**: explicit
      `#[serde(rename_all = "...")]` chosen deliberately, additive-only evolution (new
      fields optional with defaults, never renamed/removed/retyped), and a round-trip test
      pinning the JSON shape. Cheap to decide here; a breaking wire change after agents
      script against it is not.
- [ ] **P11.3** JSON + table output for `exub scan`, sortable/filterable — a
      machine-readable, cited evidence set, never a verdict.

**Exit gate:** all P11 boxes checked · `cargo xtask ci` green · two different strategies
produce `Finding`s through the same `Engine` call; `--json` output round-trips.

## Phase 12 — Strategy library (vol depth + non-vol strategies)
Goal: make the seam earn its keep — deepen the flagship and add strategies that have nothing
to do with vol, proving the engine is for all traders.

- [ ] **P12.1** Deepen the flagship's vol math beyond ATM: term structure, skew,
      front-vs-back — each metric a pure, tested function in `vol`.
- [ ] **P12.2** **One non-vol strategy** (momentum or breakout) on the same seam — the
      proof the seam is real. The library then grows *organically*: a new strategy is a new
      `Strategy` impl + tests, never a roadmap phase.
- [ ] **P12.3** Strategy-level **context filters**: earnings-date proximity (via the
      Phase-9 events capability) and liquidity — the useful residue of the cut risk engine
      (Phases 19–22), reframed as *discovery context* ("this cheap vol is an earnings
      landmine"), not execution risk.

**Exit gate:** all P12 boxes checked · `cargo xtask ci` green · a non-vol strategy surfaces
cited `Finding`s over the same engine and data the flagship uses · a `Finding` inside an
earnings window carries that context in its evidence.

## Phase 13 — Backtest harness
Goal: measure whether a strategy's findings actually realize — offline, reproducible, over
fixtures — so a strategy earns trust with numbers, not vibes.

- [ ] **P13.1** Replay historical data through any `Strategy` to measure how often its
      findings realize (the flagship: "cheap vol on a proven mover"); entry/exit simulation
      + summary stats. Offline (mock/fixtures), no live data.

**Exit gate:** all P13 boxes checked · `cargo xtask ci` green · a backtest over fixture data
produces a deterministic summary for two different strategies.

## Phase 14 — Strategy evals (grounding, in CI)
Goal: the honesty backstop, standing in CI — every metric proven against known answers, and
every surfaced finding proven to be backed by the data it cites.

- [ ] **P14.1** Known-answer evals for every strategy/metric: verifiable inputs → asserted
      outputs, run by the gate.
- [ ] **P14.2** A **grounding check** in CI: a surfaced `Finding` is backed by the exact
      data it cites — recompute from the cited inputs and compare. Grows with every later
      phase.

**Exit gate:** all P14 boxes checked · `cargo xtask ci` green · the grounding check is a
standing CI test that would fail on a fabricated or drifted figure.

## Phases 15–16 — ~~In-engine AI layer (tool-use loop + model adapters)~~ — cut
A tombstone (re-scoped 2026-07). The MCP surface (Phase 17) **inverts the plan**: instead of
the engine driving an LLM through an in-engine tool-use loop, **agents drive the engine** —
Claude Code / Gemini CLI / Codex connect as MCP clients and bring their own model. The
division of labor is identical (the model plans *what to look at*; the engine fetches,
computes, and cites), but the engine now contains **zero LLM code** and never reads a model
API key. That's a strictly better trade: every model/agent vendor stays swappable (keystone
1) with nothing to maintain, and "the engine authors the number" is enforced by *absence*
rather than discipline. The `AiProvider` trait + `EchoAi` mock stay in `exub-core` as a
designed-but-dormant seam; the registry's model/agent entries stay `planned` as
documentation. Reviving in-engine model calls is an explicit re-scope, not a drift. The
`#15–16` slots are kept so phase numbers don't shift.

**Exit gate:** none — nothing to verify. Proceed to Phase 17.

## Phase 17 — The MCP surface (the AI layer)
Goal: **the reason to exist** (§0 keystone 3c) — the engine becomes a higher-order MCP, the
mirror of a vendor's raw-data MCP: theirs hands over raw bars and cites nothing; ours returns
grounded candidates with provenance. Since Phases 15–16 were cut, this *is* the AI layer:
agents bring the intelligence, the engine brings the numbers.

- [ ] **P17.1** Expose the engine's discovery capabilities (scan, evaluate, backtest, and
      the stored IV history with provenance) as **MCP tools** — an `exub serve` command —
      that agentic assistants (Claude Code, Gemini CLI, Codex) call: *their* LLM reasons
      about which trade; *our* engine screens, computes, and **cites** the number.
- [ ] **P17.2** The MCP tool-server is another pure view over the same `Engine` (no logic of
      its own); the `massive` MCP remains a possible upstream feed, never a competitor.
- [ ] **P17.3** Document a **reference agent workflow** — scan → research → thesis →
      adversarial review, orchestrated *agent-side* over these tools (the desk process the
      dropped Phase-2 skills sketched, now living where the LLM lives). Documentation, not
      engine code; every number in it comes from an engine tool call.

**Exit gate:** all P17 boxes checked · `cargo xtask ci` green · an MCP client calls a scan
tool and receives cited `Finding`s identical to the CLI's for the same question · the
reference workflow runs end-to-end with every figure traceable to a tool call.

## Phase 18 — Finding → tradeable structure
Goal: the deterministic last mile — turn a candidate into the concrete structure a thesis
rests on. (The research/thesis/review *process* around it is agent-side, P17.3; this phase
is only the math the engine must own.) Still support, never a call: the human decides.

- [ ] **P18.1** **Finding → concrete trade structure:** for the vol flagship, the option
      structure (strike/expiry) with cost + breakeven; for other strategies, the instrument
      + entry/stop. A `Finding` names *what's mispriced*; this names *what you'd trade* —
      and still never whether to. Pure, tested, cited like every other figure; exposed to
      the CLI and as an MCP tool.

**Exit gate:** all P18 boxes checked · `cargo xtask ci` green · a flagship `Finding`
resolves offline to a concrete structure with cost + breakeven, cited to its inputs.

## Phases 19–22 — ~~Broker seam, sandbox fills, risk engine, live gate~~ — cut by design
A tombstone (re-scoped 2026-07) — and a *strengthening* one: the engine now contains **no
execution path at all**. "Finds, never recommends, never acts" is guaranteed **by absence**:
there is no order code to guard, so four phases of live-gate/risk-veto apparatus protect
nothing and are gone. You trade in your own brokerage; the engine's job ends at cited
evidence. What survives: the `BrokerProvider` trait + `PaperBroker` mock stay in `exub-core`
as an inert reference seam (P3.3/P3.4 verify them as *contracts*, nothing more), and the
earnings-landmine check became a strategy-level context filter (P12.3). Reviving execution
is an explicit re-scope with its own guardrail design — never a feature to drift into. The
`#19–22` slots are kept so phase numbers don't shift.

**Exit gate:** none — nothing to verify. Proceed to Phase 23.

## Phase 23 — Journal crate
Goal: close the loop from finding to outcome — so a strategy's *real* hit rate becomes
measurable instead of remembered.

- [ ] **P23.1** Watchlist, trade log, and thesis tracking, persisted via the Phase-8
      `Store`.
- [ ] **P23.2** Link the chain: scan hit → research → thesis → trade (logged manually —
      the engine places nothing) → outcome; the strategy's realized hit rate is queryable.

**Exit gate:** all P23 boxes checked · `cargo xtask ci` green · a full chain from finding to
recorded outcome round-trips through the store.

## Phase 24 — Scheduled premarket scan & alerts
Goal: the engine runs while you sleep — surface what's *new*, never whether to trade.

- [ ] **P24.1** A scheduled premarket routine — deliberately thin: a cron entry driving
      `exub scan --json` plus a journal-backed watchlist **diff** (`exub scan --diff`);
      near-zero new engine code by design.
- [ ] **P24.2** Alert on new candidates (the diff's output) via a simple notification hook
      (webhook / ntfy / email — pluggable, minimal); advisory only.

**Exit gate:** all P24 boxes checked · `cargo xtask ci` green · a scheduled run diffs, fires
on a new candidate, and stays silent on a repeat.

## Phase 25 — Packaging, CI, docs & signed releases
Goal: ship it honestly — reproducible builds, a written architecture record, and supply-chain
rigor sized to the actual audience.

- [ ] **P25.1** Tag-triggered release building `--locked` from the committed lockfile, with
      checksums.
- [ ] **P25.2** (decision) **Scope call:** SBOM + keyless signing (cosign/sigstore) + a
      scheduled RustSec advisory audit are right-sized only if exuberance is *distributed*;
      for a personal cockpit, trim to reproducible `--locked` builds + checksums and add the
      rest when there are external users.
- [ ] **P25.3** Consolidate `ARCHITECTURE.md` from the accumulated `(decision)` entries —
      the design + hard-to-reverse-decisions record §0 references.

**Exit gate:** all P25 boxes checked · `cargo xtask ci` green · a tagged release builds
reproducibly from the lockfile; `ARCHITECTURE.md` exists and covers every `(decision)` item.

## Phase 26 — CLI cockpit polish
Goal: the daily-driver surface, finished. The programmatic surfaces once planned here — a
TUI, an HTTP/JSON API, and four SDKs (Python/TS/Rust/Go) — are **cut** (re-scoped 2026-07):
`--json` covers scripts, MCP (Phase 17) covers agents, and a Rust consumer embeds the
crates directly. The wire-stable `Finding` (P11.2) keeps a future API cheap *if demand ever
appears*; building one now would be maintenance for users who don't exist.

- [ ] **P26.1** Finish the cockpit: `scan` (with `--diff`), `providers`, `backtest`,
      `structure` (the Phase-18 resolver), `journal`, `serve` (the MCP server), `doctor`
      (env/keys/store health). Consistent `--json` on every read command.

**Exit gate:** all P26 boxes checked · `cargo xtask ci` green · the full loop — find →
inspect → structure → journal → diff tomorrow — works from one binary, offline on mock and
live on a real feed.

---

## Architectural invariants (never traded away)
- **Agnostic by trait, not `if vendor ==`:** a new feed, model, broker, or strategy is a new
  adapter behind a trait — never a special case in the core. The registry is the only place
  a vendor is named; capabilities are declared and probed, never assumed.
- **Finds, never recommends:** no phase adds a "buy/sell" verdict or an autonomous action.
  The engine surfaces cited evidence; the human decides and trades.
- **The engine authors the number, not the LLM:** every figure is computed by the engine and
  cited to its inputs, so a number can't drift into a hallucination. The agent (over MCP)
  chooses what to look at; it never eyeballs a metric.
- **Canonical schema is the anti-corruption layer:** adapters map raw APIs → canonical types
  (`Bar`, `IvSnapshot`, `Finding`); the engine never sees raw. Provider API drift is
  contained to one adapter and **caught in CI** by contract tests over recorded fixtures.
- **Mock-first, keyless, offline-testable core:** `vol` / `exub-core` / `signals` build and
  test with no network and no keys; live adapters hide behind features and test against
  fixtures. The gate runs green on a machine with no secrets.
- **Twelve-factor config; secrets out of the repo:** flags > env (`EXUB_*`) > file (TOML) >
  defaults; adapter selection is config, not code. Secrets come from provider-native env
  vars only — never the config file, a log, a fixture, or a commit.
- **Structured errors, no panics:** one `ProviderError` vocabulary; `unwrap` / `expect` /
  `panic!` denied outside tests. A failed feed/model/broker call is a value that degrades to
  a clear message.
- **No execution, no LLM — by construction:** the engine contains no order-placement path
  to any real venue and no code that calls a model. It cannot act, and it cannot fabricate
  a number, because the code isn't there. `BrokerProvider` / `AiProvider` stay dormant
  contracts; reviving either is an explicit re-scope, never drift.
- **Decimals internally, percent at the edge:** money/vol are decimals; format only at
  display.
- **Discovery, not everything:** exuberance does *not* do real-time or streaming data (EOD
  is the product), HFT / sub-second anything, order placement of any kind, portfolio
  optimization or allocation, tax-lot accounting, or act as a data vendor (it reads sources
  you're licensed for and never redistributes their data). It finds and cites; the human
  decides and trades.
