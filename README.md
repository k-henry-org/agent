# exuberance

A grounded trade-*discovery* engine + AI cockpit for discretionary traders —
**any market, any strategy. It finds and cites; it never recommends and never
acts.** A Rust engine whose **data feeds and strategies plug in behind traits** —
surfaced through a CLI now and an MCP server on the roadmap, so any AI agent can
call it. The engine contains **no LLM code and no execution path**: agents bring
their own model over MCP, and you trade in your own brokerage.

The engine is **strategy- and asset-class-agnostic** (equities, options, futures,
crypto, FX). Its **flagship reference strategy** — the first that ships and proves
the seam — is *cheap volatility on proven movers*: find options where implied vol is
underpricing future movement (low IV rank, implied below realized) on underlyings
with a demonstrated history of big moves. Other strategies plug in the same way. See
[`.rules`](.rules) for the full operating manual.

## Why this engine exists (vs. Massive's MCP)

Massive (and other vendors) already ship an **MCP server** that hands an AI agent
raw market data — bars, quotes, chains, a live IV snapshot. So why a Rust engine
instead of just letting Claude Code call that MCP? Because **the edge is a
computation, not a data lookup — and the data you need doesn't come from one call.**

(Shown with the flagship vol strategy; every point holds for any strategy.)

- **LLMs can't be trusted to compute the signal.** IV rank (where today's IV sits
  in its *own* 1–3yr range), realized-vs-implied, proven-mover move-counting — exact
  numbers a real-money decision rests on. A chat model eyeballing them from a data
  blob approximates or hallucinates. The engine computes them deterministically and
  **cites the exact inputs** (bars used, history window, provider). The engine
  authors the number; the model never does.
- **A strategy needs accumulated state no snapshot returns.** To rank today's IV
  against its history you need the 1–3yr *series* of daily ATM IV — which a snapshot
  MCP call doesn't give you. The engine **accumulates and persists** that state; every
  strategy has state like it, and without the engine you can't compute the signal at
  all. *This is the concrete reason to exist.*
- **Scale, schedule, reproducibility, vendor-independence.** Premarket-scan a
  universe, rank, diff a watchlist, alert; backtest the screen to know the edge is
  real; do it identically every day across whichever feed is available — Massive
  today, Alpha Vantage tomorrow — behind one schema, drift caught in CI. A chat
  session is none of these.
- **Then it becomes the MCP.** exuberance exposes a *higher-order* tool — "cheap-vol
  candidates, with cited evidence" — that Claude Code / Gemini CLI / Codex call.
  Their LLM reasons about *which* trade; the engine supplies the trustworthy,
  computed signal. Massive's MCP cites nothing; ours cites the number and its
  provenance.

**In one line:** Massive's MCP gives an agent raw data to (mis)crunch; exuberance
computes the edge (cheap-vol today, any strategy next) — correctly, statefully,
reproducibly, vendor-agnostically — and exposes *that* as a grounded tool.

## Quick start

```bash
cargo test --workspace          # 30 tests, runs offline
cargo run -p cli -- scan        # demo screen over synthetic data (evidence, not advice)
cargo run -p cli -- providers   # the plug-in catalog: data feeds, AI models, coding agents, brokers
cargo xtask ci                  # the full local gate (fmt, clippy, build, test, docs, powerset, deny)
```

`exub providers` shows the multi-vendor **plug-in matrix** — every data feed
(mock, Massive, Alpha Vantage) with `wired`/`planned` status; the AI-model and
broker entries document *dormant* seams (agents connect over MCP instead, and the
engine places no orders). Selecting a feed is config (`--data-provider`,
`EXUB_DATA_PROVIDER`); adding one is a new adapter + one registry arm. The seams
are `async` so real feeds slot straight in.

Config is layered **flags > env (`EXUB_*`) > file (TOML) > defaults** — e.g.
`EXUB_DATA_PROVIDER=massive`, `EXUB_TRADING_MODE=paper`, `--config exub.toml`.
Secrets never live in config: copy `.env.example` → `.env` and add your
`MASSIVE_API_KEY` (env only) when you wire live data.

## The engine (Rust)

The engine is **agnostic across four seams**: it talks to market-data feeds,
brokers, AI models, and strategies only through traits in `exub-core`, so swapping a
feed, broker, model, or strategy is adding a crate, not editing the engine. The
agnosticism is a means, not the end: the goal is the most **efficient** way to find
trades — whatever they are — so the engine is never tied to one data or LLM vendor,
and can swap or benchmark them head-to-head as better or cheaper ones appear. See
[`ROADMAP.md`](ROADMAP.md).

| Crate | Role |
|-------|------|
| `exub-core` | The contract layer: `Provider`/`Capability`, the unified `ProviderError`, and the `MarketDataProvider` / `BrokerProvider` / `AiProvider` seams + mock/paper/echo reference impls. Only dep: `async-trait`. |
| `vol` | Vol math for the flagship strategy: realized vol, IV rank/percentile, implied−realized spread, move detection. Fully tested, no deps. |
| `market-data` | Market-data **providers** implementing `exub-core`'s trait. `MockSource` for tests, `MassiveSource` stub for live. |
| `signals` | Pluggable **strategies/screens** over any `MarketDataProvider`. Cheap-vol is the flagship reference strategy; others plug in behind the same seam. |
| `cli` | The `exub` binary. `exub scan` runs the screen; `exub providers` lists the wired providers. |
| `xtask` | Dev orchestration — `cargo xtask ci` runs the full local gate. Never shipped. |

## The AI layer (MCP — agents bring their own model)

The engine contains **no LLM code**; intelligence connects from the outside:

- **Agents drive the engine over MCP.** Claude Code, Gemini CLI, or Codex connect
  as MCP clients with their own model + key — the engine never reads a model key.
  The agent *plans* what to look at; the **engine** fetches, computes, and cites
  the number — so a figure can't be hallucinated, *by construction*.
- **The MCP surface is the AI layer.** `exub serve` (roadmap Phase 17) publishes
  the grounded discovery capabilities — scan, evaluate, backtest, stored IV
  history — as tools, so any agentic assistant calls *cited signals* instead of
  crunching raw data itself.
- **The desk process is agent-side.** scan → research → thesis → adversarial
  review is a documented reference workflow the agent orchestrates over those
  tools; every number in it comes from an engine call. See
  [`ROADMAP.md`](ROADMAP.md) Phases 17–18.

## Guardrails

The engine contains **no execution path and no LLM calls** — it cannot act and
cannot fabricate a number, because the code isn't there. You trade in your own
brokerage. Secrets live in `.env` (gitignored); the only keys the engine reads are
data-feed keys. This is decision *support* — the human owns the trade and the
risk. Details in [`.rules`](.rules).

## Roadmap

The arc: vol math + screen → provider-agnostic contracts → config/CI → async seams +
registry → a real EOD data feed → the IV-history store (the "reason to exist") → the
`Strategy` seam + backtests → the MCP surface. Deliberately cut along the way:
in-engine LLM adapters (agents bring their own over MCP) and all broker/execution
phases (the engine never places orders). The full plan — tombstones included — is in
[`ROADMAP.md`](ROADMAP.md); its checkboxes are the **single source of truth** for
progress.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) — prerequisites, the local gate
(`cargo xtask ci`), the testing approach, and the invariants (agnostic-by-trait,
finds-not-recommends, no-panic, secrets-out-of-repo). The operating manual is
[`.rules`](.rules).

## License

Apache-2.0 — see [`LICENSE`](LICENSE).
