# Contributing

Thanks for your interest. **agent** (working name) is a guardrail-detection kernel in
Rust: tiny classifiers (prompt-injection, PII, secrets, toxicity) compiled into
**portable, signed WASM artifacts** that run identically everywhere — embedded via
wasmtime, at the edge, in a browser. **It detects and cites; it never decides.**

> Read [**`.rules`**](./.rules) first — the operating manual and the invariants that
> must never be traded away (`CLAUDE.md`, `AGENTS.md`, and `GEMINI.md` all point
> there). The staged plan is in [**`ROADMAP.md`**](./ROADMAP.md).

## Prerequisites

- **Rust, stable** ([install `rustup`](https://www.rust-lang.org/tools/install)),
  plus the `wasm32-unknown-unknown` target (`rustup target add
  wasm32-unknown-unknown`) for building detector artifacts. No nightly, no `sudo`.
- **No API keys — ever.** The detection path needs none by design, and the mock
  detector keeps every command, test, and demo keyless and offline.

## Quick start

```console
git clone <repo> && cd <repo>
cargo build

# The mock detector is the keyless default — no keys, no network, no registry:
cargo run -p cli -- check --detector mock "text to scan"   # rendered Verdict
cargo run -p cli -- check --detector mock --json < file    # wire output; exit 1 = detection
```

Config is layered **flags > env (`AGENT_*`) > file (TOML) > defaults**. Exit codes
are contract: `0` clean · `1` detection fired · `2`+ operational error.

## Before you push — the local gate

```console
cargo install cargo-deny cargo-hack   # one-time: the gate shells out to both
cargo xtask ci                        # fmt + clippy -D warnings + build + test + docs
                                      # + feature powerset + deny + artifact goldens
```

The gate also **builds every `detectors/*` source to wasm and runs its golden
verdicts** — a detector change that shifts a verdict fails CI unless its goldens
are updated in the same change. CI mirrors the gate on `ubuntu-latest` with no
secrets.

## The testing approach

Everything runs **offline and keyless**:

1. **Unit / pure:** ABI encode/decode, config precedence, span math, verdict
   rendering — table-driven, no network.
2. **Golden verdicts per detector:** `detectors/*/cases/` pairs input text with the
   expected `Verdict` JSON; run against the *built artifact* by the gate.
3. **Determinism tests:** the same input × 100 runs × two targets must produce
   byte-identical verdicts; learned detectors additionally prove cross-architecture
   identity (quantized math).
4. **Eval scorecards (Phase 6):** precision/recall on public corpora, CI-generated,
   with a regression fence — quality drops fail the gate.

## The invariants (never trade these away)

- **Agnostic by ABI, not by host or detector.** A new detector, inference
  technique, or host language is a new artifact/SDK behind the frozen contract —
  never a special case in the kernel.
- **Detects, never decides.** No policy (block/redact/route) in the kernel; spans
  are lossless so the *host* can act.
- **Deterministic by absence.** No clocks, randomness, network, or filesystem
  inside the sandbox; an artifact importing anything beyond the ABI fails to load.
- **Measured, not marketed.** Scorecards are CI-generated; hand-written accuracy
  claims are forbidden.
- **The wire contract is sacred.** `Verdict` JSON + exit codes are golden-tested
  and evolve additively-only.
- **No LLM code, no model keys, no secrets.** Fixture credentials are synthetic
  only — never real, not even revoked.
- **No-panic discipline.** `unwrap`/`expect`/`panic!` denied outside tests; every
  failure is a typed value.
- **Artifacts are source.** Wasm binaries are built by the gate, signed at
  release — never hand-committed.

## Phases & decisions

Work is organized into sequentially-gated phases in [`ROADMAP.md`](./ROADMAP.md) —
the **single source of truth for progress**. Its checkboxes are the state: work the
first unchecked box in ID order, one item per iteration, and check the box **in the
same commit** as the work (referencing the ID, e.g. `P3.2: …`). A phase isn't left
until its **Exit gate** passes; the next isn't started before that. Items tagged
`(decision)` record the hard-to-reverse choice in `ARCHITECTURE.md` (consolidated
in P13.2) so the *why* outlives the diff.

## Commit & PR conventions

- One logical change per commit; **imperative** subject ("Add the PII detector",
  not "added the PII detector").
- **Never add an AI co-author or attribution trailer** — no `Co-Authored-By:
  Claude …` or similar. Never commit secrets, real credentials (even revoked), or
  built wasm binaries.
- A new detector is a new `detectors/` directory (source + manifest + goldens),
  never a runtime special case.
- Every PR must pass the full gate (`cargo xtask ci`).

## License

By contributing you agree your contributions are licensed under **Apache-2.0**, the
project's license (see [`LICENSE`](./LICENSE)).
