# Roadmap

## §0 The spine

**`agent` (working name) is a guardrail-detection kernel (Rust + WASM).** Tiny classifiers —
prompt-injection, PII, secrets, toxicity/jailbreak — compiled into **portable, signed WASM
artifacts** that run *anywhere*: embedded in a Rust/Go/Python service via **wasmtime**, in an
edge worker, in a proxy hot path, in a browser. One artifact, one frozen ABI, identical
verdicts everywhere. The guardrail *market* is saturated at the service/library layer
(Lakera, llm-guard, NeMo Guardrails); nobody ships the detector as a **portable artifact**.
The wedge is the packaging, not the classifier.

The shape is **ports & adapters**: a headless **host runtime** (`agent-host`, wasmtime
embedding with fuel/memory/epoch limits) drives one contract — the **Detector ABI** — over
which every detector artifact plugs in. A canonical **`Verdict`** wire type sits in the
middle: labels, scores, spans, and **provenance** (detector id + version + threshold + eval
scorecard). Every **surface** (the `agent` CLI now; SDKs and a sidecar later) only *renders*
what the runtime returns — no surface reaches into an artifact directly. The kernel does
**detection only**: policy (block/allow/redact/route) is the control plane's job — ⟐ the Go
suite (`operator`) loads these artifacts and decides; this repo never does. The operating
manual is [`.rules`](./.rules); hard-to-reverse decisions land in `ARCHITECTURE.md`.

Three keystones hold it up:

1. **The artifact is the product — agnostic by ABI, never by host.** A detector is a signed,
   versioned `.wasm` implementing the frozen ABI and nothing else; a new detector, a new
   inference technique, or a new host language changes **zero kernel code**. Hosts are
   swappable (wasmtime server-side, web runtime in the browser, any WASI-compliant edge);
   detectors are swappable (rules-based, linear, tiny-transformer — the ABI doesn't care).
   If a change makes the runtime special-case one detector or one host, the design is wrong.
2. **Deterministic, local, private — by construction.** Same input + same artifact → the
   **same verdict**, every time, on every host: no wall-clock, no randomness, no network in
   the detection path — the sandbox has no way to phone home *because the imports aren't
   there*. Detection runs where the data already is; nothing leaves the process. Every
   verdict carries provenance, and every artifact ships with its **eval scorecard**
   (precision/recall on public corpora, measured in CI): **measured, not marketed**.
3. **The differentiator is packaging + the feed — never the classifier.** Classifiers are
   commodity research; the moat is (a) the **stable ABI + toolchain** that turns any small
   model into a portable artifact, (b) **signed, content-addressed distribution** with
   versioning and rollback, and (c) — the open-core seam — a **continuously retrained
   artifact feed** (the virus-definitions model: attacks evolve, subscribers get fresh
   detectors; the runtime and reference detectors stay OSS forever). If a change turns this
   into one more guardrail *service*, it has lost its reason to exist.

The discipline test for every step: *"does this make detection more portable, more
deterministic, or better measured — without adding network to the hot path, LLM code, or
policy logic to the kernel?"* If no, it sinks to a later phase — or out entirely. **The mock
detector keeps every phase keyless, offline, and toolless.** We do not start a phase until
the one before it is green on the gate.

---

## §0.5 How to work this roadmap (the working loop)

*(Terminology: `agent` is this project's name; "the coding agent" below means the AI
assistant working this file. Where a sentence could read either way, it says which.)*

This file is the **single source of truth for progress**. The checkboxes are the state; no
other tracker exists. **Every box below is unchecked and the count starts at zero: the repo's
prior code belongs to a retired project and is ignored — audit nothing, inherit nothing;
Phase 1 scaffolds fresh.** Work it as a loop:

1. **Locate.** The current item is the first unchecked box in the lowest-numbered phase with
   unchecked boxes. Work strictly in ID order (`P3.2` before `P3.3`) unless a box says
   otherwise.
2. **Implement exactly the item.** One item ≈ one iteration ≈ one reviewable change. Don't
   reach ahead into later items "while you're in there" — that's how phases bleed together.
3. **Gate.** `cargo xtask ci` must be green before an item is done (fmt · clippy
   `-D warnings` · build · test · docs · feature powerset · `cargo-deny`, plus — once P2.3
   lands them — detector artifact builds + goldens; all keyless and offline). An item whose
   box mentions a test or doc isn't done until that test/doc exists.
4. **Check the box in the same commit as the work**, and reference the ID in the commit
   message (e.g. `P4.2: fuel + epoch limits on every instantiation`). A checked box with no
   commit behind it is a lie; a landed change with an unchecked box is invisible.
5. **Advance.** A phase is done only when its **Exit gate** line passes end-to-end. Never
   start phase N+1 before phase N's exit gate is green.

**Epics.** An item tagged `(epic)` is too big for one iteration: before implementing, expand
it in-place into lettered sub-boxes (`P5.2a`, `P5.2b`, …) sized to one iteration each — that
expansion is itself one iteration — then work the sub-boxes.

**Decision items** are tagged `(decision)`: they produce a dated entry in `ARCHITECTURE.md`
(the decision, the alternatives, the why) and get checked when it's merged.

**When the map is wrong.** If an item turns out to be obsolete, mis-scoped, or blocked by a
decision above your pay grade: don't silently skip it and don't silently do something else.
Edit this file (reword / split / move the item) in its own commit with a one-line rationale,
or stop and ask. The roadmap must always describe reality.

---

## Phase 1 — Fresh workspace + the ABI + canonical `Verdict` + mock detector
Goal: the contract everything else implements, provable offline — a fresh workspace, the
Detector ABI pinned as a spec, the `Verdict` wire type, and a mock artifact + CLI demo so
the whole pipeline is visible end-to-end before any real model exists.

- [ ] **P1.1** Fresh workspace scaffold: `crates/abi` (contract + `Verdict` types),
      `crates/host` (runtime, Phase 3), `crates/cli`, `detectors/` (artifact sources),
      `xtask`. The retired project's **code** — its `crates/*`, `data/`, `xtask`, and
      manifests — is removed in this same step (the docs were already rewritten 2026-07-08);
      the repo describes exactly one project again.
- [ ] **P1.2** (decision) **ABI v0 shape:** WASM **component model (WIT)** vs plain core-wasm
      exports (`alloc` / `detect(ptr,len) → ptr`). Decide for reach (browser + edge runtimes
      lag components) vs ergonomics (WIT typed interfaces); record the migration story for
      whichever loses. The ABI is versioned from day one (`abi_version` export).
- [ ] **P1.3** Canonical **`Verdict`** in `crates/abi`: labels + scores + byte-offset spans +
      provenance (detector id, semver, threshold, scorecard hash). `#[non_exhaustive]`,
      constructors, **serde-stable field naming** (explicit `rename_all`, additive-only
      evolution), and a round-trip test pinning the JSON shape — it will be serialized by
      the CLI, three SDKs, and a sidecar; a breaking wire change after hosts script against
      it is not cheap.
- [ ] **P1.4** The **mock detector**: a trivial rules artifact (fixed keyword hits) built
      from `detectors/mock/` by `xtask` to whatever wasm target the P1.2 decision fixed
      (`wasm32-unknown-unknown` if core-wasm won), checked in as source (never as a binary
      blob). It is the permanent keyless fixture every later phase tests against.
- [ ] **P1.5** `agent check --detector mock "some text"` — parses the text, returns a rendered
      `Verdict` (and `--json`), **without wasmtime yet**: the mock runs via a native
      `Detector` trait impl proving the abstraction before the runtime exists.

**Exit gate:** all P1 boxes checked · `cargo xtask ci` green · `agent check --detector mock`
renders a cited `Verdict` offline, keyless, toolless · the JSON round-trip test pins the wire
shape.

## Phase 2 — Config, CI & xtask scaffolding
Goal: the 12-factor + rigor substrate, before any real I/O — every later phase inherits the
gate instead of relitigating discipline by hand.

- [ ] **P2.1** Layered `Config` (**flags > env (`AGENT_*`) > file (TOML) > defaults**) with a
      pure `resolve()` fold and precedence pinned by unit tests; detector/artifact selection
      is config, not code. **This tool holds no secrets** — there is no API key to read,
      and no phase may add one to the detection path.
- [ ] **P2.2** `tracing` logs to **stderr**, filtered by config; stdout reserved for
      verdicts, so `agent check … 2>/dev/null` stays pipe-clean. Exit codes are part of the
      wire contract: `0` clean · `1` detection fired · `2`+ operational error.
- [ ] **P2.3** `cargo xtask ci` — the local gate: fmt · clippy `-D warnings` · build · test ·
      docs (`RUSTDOCFLAGS=-D warnings`) · feature powerset (`cargo-hack`) · `cargo-deny` ·
      **artifact build** (every `detectors/*` source **compiles** to wasm; goldens run via
      the P1.5 native path for now — they switch to executing the built artifact in P3.4,
      once a runtime exists to run wasm at all). Keyless, offline, stops at the first
      failure.
- [ ] **P2.4** A GitHub Actions workflow mirroring the gate step-for-step, plus one aggregate
      required status check so branch protection needs a single rule.

**Exit gate:** all P2 boxes checked · gate green locally **and** in CI with no secrets ·
precedence tests pin flags > env > file > defaults · a deliberately non-compiling mock
detector fails the artifact-build step.

## Phase 3 — The host runtime (`agent-host`, wasmtime)
Goal: the irreversible shape decision — the sandboxed execution environment every artifact
runs in, made while mock-only (cheap now, expensive after real detectors exist).

- [ ] **P3.1** wasmtime embedding: load + instantiate an ABI-conformant artifact; **fuel
      metering** (bounded compute per call), **memory limits**, and **epoch interruption**
      (wall-clock kill switch) on every instantiation — a hostile or buggy artifact is a
      contained error, never a hang or a resource leak.
- [ ] **P3.2** **Determinism enforced by absence:** the linker exposes *no* WASI clocks, no
      randomness, no network, no filesystem — an artifact that imports anything beyond the
      ABI fails to load with a clear typed error. A determinism test runs the same input 100×
      and asserts byte-identical verdicts; the **CI matrix** repeats it on a second OS/arch
      runner (the local gate is single-machine and can't — don't block on it locally).
- [ ] **P3.3** Instance lifecycle for the hot path: pooled instantiation (or
      instance-per-call — measure, then decide `(decision)`), pre-compiled module caching,
      and a micro-benchmark harness pinning **p99 per-call latency and cold-start budgets**
      as **generous absolute thresholds** the gate enforces (never run-to-run diffs — shared
      CI runners make relative perf comparisons flaky; a budget breach is red, noise is not).
- [ ] **P3.4** `agent check` now runs the mock **through wasmtime** — the native-trait path
      from P1.5 stays as the test double; both must return identical verdicts (a golden test
      proves the seam is honest).

**Exit gate:** all P3 boxes checked · gate green · the same artifact returns byte-identical
verdicts across 100 runs locally and across the CI matrix's second target · a fuel-bomb
artifact (infinite loop) is killed and surfaces as a typed error · benchmark budgets are
recorded in-repo.

## Phase 4 — First real detectors (pattern class: secrets + PII)
Goal: the first artifacts with real utility — the *deterministic-by-nature* detector class
(pattern + entropy + validation), pure Rust compiled to wasm. No ML yet: prove the artifact
pipeline on detectors whose correctness is testable to the byte. (Batch/single-pass only —
stream sessions are Phase 8; don't build stream state here.)

- [ ] **P4.1** `detectors/secrets`: single-pass pattern + entropy detection for credentials
      (cloud keys, tokens, private-key headers), with span-accurate offsets and per-pattern
      sub-labels in the `Verdict`. Corpus: synthetic fixtures only — **never real keys**,
      not even revoked ones.
- [ ] **P4.2** `detectors/pii`: emails, phone numbers, government-ID shapes, IP addresses —
      validation-aware (checksums where they exist) to hold precision. Locale scope for v0
      is `(decision)`-documented: US/EU shapes first, extension is additive.
- [ ] **P4.3** The **golden-verdict harness**: every detector directory carries
      `cases/*.txt` + expected-verdict JSON; `xtask` runs them against the built artifact.
      Adding a detector = source + cases, never runtime changes.
- [ ] **P4.4** Size/latency budgets become per-artifact manifests (`agent.toml`): max wasm
      bytes, max p99 µs — enforced by the gate, recorded in the scorecard.

**Exit gate:** all P4 boxes checked · gate green · `agent check --detector secrets` catches a
fixture AWS key with correct spans and exit code 1 · both artifacts hold their size/latency
budgets in CI.

## Phase 5 — The ML toolchain + the injection detector (the flagship)
Goal: the reason the ABI exists — a *learned* classifier (prompt-injection/jailbreak) inside
the same portable artifact, and the reusable toolchain that turns any tiny model into one.

- [ ] **P5.1** (decision) **Inference approach inside the artifact:** hand-rolled linear /
      hashed-n-gram model in pure Rust vs a no-std-friendly inference lib (e.g. tract/candle
      subset) compiled to wasm. Decide on artifact size, determinism (fixed-point vs float
      drift across hosts), and toolchain simplicity; record the revisit trigger (if quality
      plateaus, escalate model class).
- [ ] **P5.2** (epic) **`agent-train` toolchain** (`crates/train`, dev-only, never shipped
      in the runtime path): dataset prep (public injection corpora + synthetic augmentation)
      → train tiny classifier → quantize → embed weights → emit an ABI-conformant wasm
      artifact + scorecard. Deterministic builds: same data + seed → byte-identical artifact.
- [ ] **P5.3** `detectors/injection` v0 shipped through that toolchain, with a documented
      threshold and its scorecard in the manifest.
- [ ] **P5.4** **Float-determinism audit:** verify verdict-identity across x86/ARM hosts for
      the learned model (quantized integer math preferred precisely to make this trivially
      true); the P3.2 determinism test extends to every learned artifact.

**Exit gate:** all P5 boxes checked · gate green · the injection artifact classifies the
held-out fixture set at or above its scorecard numbers, byte-identically on x86 and ARM
(via the CI matrix) · rebuilding from the same inputs reproduces the artifact hash.

## Phase 6 — The eval harness (measured, not marketed — in CI)
Goal: the honesty backstop, standing in CI — every detector's quality is a number computed
from public corpora, recomputed on every change, and shipped in the artifact's provenance.

- [ ] **P6.1** Known-answer evals per detector: public benchmark corpora — documented,
      license-checked, and **vendored (or committed as a pinned snapshot) so the gate stays
      offline**; fetching happens in a human-run refresh script, never in CI → precision /
      recall / F1 per label; the scorecard in each manifest is **generated, never
      hand-written**.
- [ ] **P6.2** A **regression fence:** a change that drops any shipped detector's F1 below
      its floor fails CI — quality regressions are caught like API breaks, not noticed in
      production.
- [ ] **P6.3** A false-positive corpus (benign text that *looks* alarming: code, key-shaped
      strings in docs, security writeups) — precision is the adoption-killer metric for
      guardrails; measure it explicitly.

**Exit gate:** all P6 boxes checked · gate green · every shipped artifact's scorecard is
CI-generated · a deliberately degraded model fails the fence.

## Phase 7 — Host SDKs (⟐ the operator seam)
Goal: prove "runs anywhere" where it matters most — embedded in other people's programs.
The Rust crate is first-class; Go is the ⟐ pairing (operator's proxies load detectors
through it); Python covers the ML world.

- [ ] **P7.1** `agent-host` published as the **Rust SDK**: embed, load artifact, get
      `Verdict` — three lines; the CLI becomes a pure view over it (it likely already is;
      prove it with a golden test).
- [ ] **P7.2** **Go SDK** over wasmtime-go, returning the same wire-stable `Verdict` JSON —
      ⟐ this is the artifact-loading seam `operator`'s Go control plane consumes; the kernel
      still knows nothing about policy.
- [ ] **P7.3** **Python SDK** (wasmtime-py), same contract; golden cross-SDK test: one
      artifact, one input, three SDKs, byte-identical verdicts.
- [ ] **P7.4** Version/compat story documented: ABI semver, artifact-manifest minimums,
      SDK support matrix — additive evolution only, mirroring the `Verdict` discipline.

**Exit gate:** all P7 boxes checked · gate green · the cross-SDK golden test passes in CI
(Rust + Go + Python against the same artifacts).

## Phase 8 — Streaming detection
Goal: the differentiator over batch scanners — verdicts over **token streams** (LLM output
as it's generated), so a proxy can act mid-stream instead of after the fact.

- [ ] **P8.1** ABI v0.x extension: stateful sessions (`open → feed(chunk) → close`) with
      carry-over state inside the artifact, so patterns spanning chunk boundaries are caught;
      batch `detect` remains the degenerate single-chunk case — one code path, tested as
      such.
- [ ] **P8.2** Incremental verdicts: a stream can fire early (secret detected at token 40 —
      surface it *then*, with the span so far), with a final consolidated `Verdict` at close.
- [ ] **P8.3** Latency budget per chunk pinned in the benchmark harness — streaming is only
      real if a chunk verdict fits inside an inter-token gap (single-digit ms, generous).

**Exit gate:** all P8 boxes checked · gate green · a fixture stream chunked at hostile
boundaries (secret split across three chunks) is caught with correct offsets · chunk-latency
budget holds in CI.

## Phase 9 — Edge & browser targets
Goal: cash the "runs anywhere" check — the *same artifact bytes* verified in a browser and
an edge-worker runtime, no recompile.

- [ ] **P9.1** A browser demo page (static, no backend): loads a shipped artifact via the
      web WASM runtime, runs detection fully client-side — the privacy story made visible.
      If P1.2 chose components, this is where the polyfill/transpile cost is paid and
      documented.
- [ ] **P9.2** An edge-worker example (WASI-compliant runtime of one mainstream provider),
      same artifact bytes, verdict parity asserted against the wasmtime host in a recorded
      test.
- [ ] **P9.3** The compatibility matrix (host × ABI feature × artifact) generated into docs —
      claims about "anywhere" become a table, not adjectives.

**Exit gate:** all P9 boxes checked · gate green · one artifact hash verified byte-identical
in verdicts across wasmtime, browser, and edge fixtures.

## Phase 10 — Signed distribution & the registry seam
Goal: artifacts become *distributable* — content-addressed, signed, versioned — and the
open-core seam gets its structural line: the protocol and tooling are OSS; the continuously
retrained **feed** is the product.

- [ ] **P10.1** Artifact packaging: manifest (`agent.toml` → embedded), content-addressed
      naming, **keyless signing (sigstore/cosign)** + verification in `agent-host` — an
      unsigned or tampered artifact is refused by default (config can allow local dev
      artifacts explicitly).
- [ ] **P10.2** `agent pull <detector>@<version>` against a dumb static registry (an OCI
      registry or plain HTTPS index — `(decision)`, lean toward OCI for free infra);
      lockfile pinning hashes so a deploy is reproducible.
- [ ] **P10.3** The **feed seam documented**: the OSS tooling speaks to *any* registry; the
      commercial feed (fresh injection models as attacks evolve) is one more registry URL —
      additive, never required, and the kernel cannot tell the difference. This is the
      open-core line; write it into `ARCHITECTURE.md` as the structural decision it is.

**Exit gate:** all P10 boxes checked · gate green · a tampered artifact is refused with a
typed error · `agent pull` + lockfile reproduces an exact artifact set on a clean machine.

## Phase 11 — ~~Policy engine / redaction actions in the kernel~~ — cut by design
A tombstone, pre-dug. The kernel **detects and cites; it never decides**. Block/allow,
redaction, routing, rate responses, tenant policy — all of it belongs to the control plane
(⟐ `operator`, or whatever host embeds the SDK). The moment policy enters this repo, it
competes with its own embedders and the guardrail *services* it exists to underlie. What the
kernel legitimately owns: spans precise enough that a host can redact losslessly. Reviving
policy here is an explicit re-scope, never drift. The slot is numbered like a real phase so
the cut is a standing, citable decision — not a silence someone refills later.

**Exit gate:** none — nothing to verify. Proceed to Phase 12.

## Phase 12 — Sidecar surface (`agent serve`)
Goal: the non-embedding on-ramp — a local process boundary for stacks that can't link an
SDK; a pure view, same contract.

- [ ] **P12.1** `agent serve`: local HTTP (and optionally gRPC — `(decision)` by demand, not
      speculation) exposing check/stream over loopback; the same `Verdict` JSON, the same
      exit-code philosophy mapped to status codes; **loopback-only by default** — this is a
      sidecar, not a service.
- [ ] **P12.2** Goldens asserting CLI, SDK, and sidecar return identical verdicts for
      identical inputs — three surfaces, one engine, provably.

**Exit gate:** all P12 boxes checked · gate green · the parity golden passes across all
three surfaces.

## Phase 13 — Packaging, docs & the benchmark writeup
Goal: ship it honestly — reproducible releases, a written architecture record, and the
rigorous public writeup that is this project's marketing.

- [ ] **P13.1** Tag-triggered release: `--locked` builds, checksums, signed artifacts for the
      reference detector set, SDK publishes (crates.io / Go module / PyPI).
- [ ] **P13.2** Consolidate `ARCHITECTURE.md` from the accumulated `(decision)` entries; the
      compatibility matrix and eval scorecards generated into docs.
- [ ] **P13.3** The **benchmark writeup**: portable-artifact guardrails vs the incumbent
      Python services — cold-start, p99 latency, memory, deployment surface, determinism —
      with honest numbers and reproduction scripts. One rigorous post; it is the launch.

**Exit gate:** all P13 boxes checked · gate green · a clean machine goes from `git clone` to
verified signed artifacts + passing evals with one documented command sequence.

---

## Architectural invariants (never traded away)
- **Agnostic by ABI, not by host or detector:** a new detector, inference technique, host
  language, or runtime target is a new artifact or SDK behind the frozen contract — never a
  special case in the kernel. The ABI is versioned; evolution is additive.
- **Deterministic by absence:** no clocks, no randomness, no network, no filesystem inside
  the sandbox — an artifact *cannot* be nondeterministic or exfiltrate, because the imports
  don't exist. Same input + same artifact = same verdict, on every host, forever.
- **Detects, never decides:** the kernel returns cited `Verdict`s with lossless spans;
  policy, redaction, blocking, and routing live in the embedding host / control plane
  (⟐ operator). No phase adds an action to the kernel.
- **Measured, not marketed:** every shipped detector carries a CI-generated scorecard from
  public corpora; a quality regression fails the gate like an API break. No hand-written
  accuracy claims, ever.
- **The wire contract is sacred:** `Verdict` JSON (field names, exit codes, status mapping)
  is golden-tested and evolves additively; CLI, three SDKs, and the sidecar return
  byte-identical verdicts for identical inputs.
- **No LLM code, no model keys:** detectors are tiny local models inside artifacts; the
  kernel never calls a model API. LLM-as-judge is someone else's product.
- **Mock-first, keyless, offline-testable core:** the mock detector keeps every command and
  test green on a machine with no secrets, no network, and no registry access.
- **Twelve-factor config; no secrets anywhere:** flags > env (`AGENT_*`) > file (TOML) >
  defaults; there is no API key in the detection path by design — a phase that needs one is
  mis-scoped.
- **Structured errors, no panics:** a hostile artifact, a fuel exhaustion, a malformed
  input — every failure is a typed value that degrades to a clear message; `unwrap` /
  `expect` / `panic!` denied outside tests.
- **Artifacts are source, builds are reproducible:** detector sources live in-repo; wasm
  binaries are built by the gate, signed at release, and never hand-committed. Same inputs →
  same artifact hash.
- **Detection, not everything:** agent does *not* do policy engines, hosted inference APIs,
  model training as a service, LLM orchestration, or content moderation adjudication. It
  detects and cites; the host decides.
