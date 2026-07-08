# Contributing

Thanks for your interest. **exuberance** is a grounded trade-*discovery* engine in
Rust: it helps discretionary traders *find* trades across **any market and any
strategy** — plug in any data feed, AI model, or broker behind a trait — and it
**finds and cites; it never recommends and never acts.**

> Read [**`.rules`**](./.rules) first — the operating manual and the invariants that
> must never be traded away (`CLAUDE.md`, `AGENTS.md`, and `GEMINI.md` all point
> there). The staged plan is in [**`ROADMAP.md`**](./ROADMAP.md).

## Prerequisites

- **Rust, stable** ([install `rustup`](https://www.rust-lang.org/tools/install)).
  No nightly, no `sudo`, no codegen step.
- For **real data** (later milestones): an API key for your chosen feed, set via
  **environment variables** — never committed. Data-feed keys are the *only* keys
  the engine ever reads: it contains no LLM code (agents bring their own model
  over MCP) and no execution path (no broker credentials, ever). For **no keys at
  all**: the built-in **mock** feed is the keyless default, so every command
  builds, runs, tests, and demos offline.

## Quick start

```console
git clone <repo> && cd exuberance
cargo build

# The mock feed + mock model are the keyless default — no API keys, no network:
cargo run -p cli -- scan        # the demo screen (cited evidence, not advice) over synthetic data
cargo run -p cli -- providers   # the plug-in catalog: data feeds, AI models, coding agents, brokers
```

Config is layered **flags > env (`EXUB_*`) > file (TOML) > defaults**; `mock` is the
keyless default. Pick a feed with `--data-provider` (or the `EXUB_*` vars / a
`--config` TOML). Secrets come from **provider-native env vars only**
(`MASSIVE_API_KEY`, `ALPHA_VANTAGE_API_KEY`, …), never the config file — and only
data-feed keys exist; the engine reads no model or broker credentials.

## Before you push — the local gate

Run the same checks CI runs, in one shot:

```console
cargo install cargo-deny cargo-hack   # one-time: the gate shells out to both
cargo xtask ci                        # fmt + clippy -D warnings + build + test + docs + feature powerset + deny
```

…or the steps individually:

```console
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked                              # offline: mock adapters + fixtures
cargo doc --no-deps --workspace --all-features --locked         # RUSTDOCFLAGS="-D warnings"
cargo hack --feature-powerset --no-dev-deps check --workspace   # no --locked: --no-dev-deps rewrites manifests
cargo deny check
```

CI mirrors this on `ubuntu-latest` with stable Rust and **no API keys** — the mock
adapters keep the whole pipeline offline and deterministic.

## The testing approach

Almost everything runs **offline, with no API keys**, via the mock adapters:

1. **Unit / pure:** vol math, screen logic, config precedence, adapter mappings,
   format helpers — table-driven, no network.
2. **Contract tests (recorded fixtures):** each real adapter (as they land) replays
   a captured provider response, so its raw→canonical mapping is deterministic
   and **API drift fails CI**, not a live scan.
3. **Known-answer / grounding evals:** verifiable inputs → asserted outputs, plus a
   check that a surfaced `Finding` is backed by the data it cites. The honesty
   backstop.

Every new screen/strategy/metric ships with unit tests against known inputs.

## The invariants (never trade these away)

- **Agnostic by trait, not `if vendor ==`.** A new feed, model, broker, or strategy
  is a **new adapter behind a trait** — never a special case in the core. If a
  change makes the engine name a vendor, the design is wrong.
- **Finds, never recommends.** No phase adds a "buy/sell" verdict or an autonomous
  action. The engine surfaces cited evidence; the human decides and trades.
- **The engine authors the number, not the LLM.** Figures (IV rank, realized/implied,
  …) are computed and cited by the engine, so they can't be hallucinated.
- **No-panic discipline.** `unwrap`/`expect`/`panic!` are denied outside tests
  (workspace clippy lints; `clippy.toml` re-allows them in tests). A failed
  feed/model/broker call is a **value** (`Err`) that degrades to a clear message.
- **Secrets out of the repo.** Keys come from the environment only; never commit,
  log, or embed them, and never put a real key or fetched data in a fixture.
- **Offline-testable core.** `vol` / `exub-core` / `signals` build and test with no
  network; live adapters hide behind features and test against fixtures.

## Phases & decisions

Work is organized into sequentially-gated phases in [`ROADMAP.md`](./ROADMAP.md) —
the **single source of truth for progress**. Its checkboxes are the state: work the
first unchecked box in ID order, verify-before-building, and check the box **in the
same commit** as the work (referencing the ID, e.g. `P8.2: …`). A phase isn't left
until its **Exit gate** line passes; the next isn't started before that. Items tagged
`(decision)` record the significant, hard-to-reverse choice in `ARCHITECTURE.md`
(consolidated in P25.3) so the *why* outlives the diff.

## Commit & PR conventions

- One logical change per commit; **imperative** subject ("Add the Massive adapter",
  not "added the Massive adapter").
- **Never add an AI co-author or attribution trailer** — no `Co-Authored-By: Claude …`
  or similar. Never commit secrets or fetched/generated data.
- A new provider or strategy is a new **adapter behind a trait**, never a special
  case in the core.
- Every PR must pass the full gate (`cargo xtask ci`).

## License

By contributing you agree your contributions are licensed under **Apache-2.0**, the
project's license (see [`LICENSE`](./LICENSE)).
