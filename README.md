# agent *(working name)*

**Guardrail detectors as portable WASM artifacts.** Tiny classifiers — prompt-
injection, PII, secrets, toxicity — compiled into **signed, content-addressed
`.wasm` artifacts** that run identically everywhere: embedded in a Rust, Go, or
Python service via **wasmtime**, in an edge worker, in a proxy hot path, in a
browser. **It detects and cites; it never decides** — policy stays in your host.

## Why

Every LLM application needs guardrails, and today that means either calling a
hosted classification API (latency, cost, your users' data leaving the process) or
vendoring a Python service into every stack that needs checking. The guardrail
market is crowded at the *service* layer — but **nobody ships the detector as a
portable artifact**. agent's bet: package detection like a codec, not a SaaS.

- **One artifact, every runtime.** A detector is a `.wasm` file implementing a
  frozen, versioned ABI. The same bytes return the same verdict under wasmtime, at
  the edge, or in a browser tab — proven by cross-target parity tests, not claimed.
- **Deterministic, local, private — by construction.** The sandbox exposes no
  clock, no randomness, no network, no filesystem. An artifact *cannot* be flaky
  and *cannot* phone home, because the imports don't exist. Detection runs where
  the data already is; nothing leaves your process.
- **Measured, not marketed.** Every detector ships with a CI-generated scorecard
  (precision/recall on public corpora). A quality regression fails CI like an API
  break.
- **Contained by design.** The runtime runs artifacts under fuel metering, memory
  limits, and epoch interruption — a buggy or hostile artifact is a typed error,
  never a hang.

## Quick start

```bash
cargo run -p cli -- check --detector mock "some text to scan"   # keyless, offline, toolless
cargo run -p cli -- check --detector mock --json < input.txt    # machine output; exit 1 = detection
cargo xtask ci                                                  # the full local gate
```

Exit codes are part of the contract: `0` clean · `1` detection fired · `2`+
operational error — so a CI step or a shell pipeline can gate on agent directly.
Config is layered **flags > env (`AGENT_*`) > file (TOML) > defaults**. There are
**no API keys**: the detection path needs none, by design.

## How it fits together

```
text/stream → agent-host (wasmtime: fuel/memory/epoch, deterministic linker)
            → detector artifact (.wasm, frozen ABI, embedded weights)
            → canonical Verdict (labels + scores + spans + provenance)
```

Two contracts carry everything: the **Detector ABI** (what an artifact implements —
versioned, frozen, inference-technique-agnostic) and the **`Verdict`** wire type
(what every surface returns — serde-stable, additive-only, golden-tested). The CLI,
the SDKs (Rust/Go/Python), and the sidecar are pure views over the same runtime and
must return byte-identical verdicts — a standing parity test proves it.

## Layout

| Path | Role |
|------|------|
| `crates/abi` | `agent-abi` — the Detector ABI + the canonical `Verdict` wire type. |
| `crates/host` | `agent-host` — the wasmtime runtime: sandboxing, determinism, instance pooling. |
| `crates/cli` | The `agent` binary: `check`, later `pull` and `serve`. |
| `detectors/` | Artifact **sources** (Rust → wasm32), one per detector, each with a manifest + golden cases. |
| `xtask` | Dev orchestration — `cargo xtask ci`, including artifact builds + goldens. Never shipped. |

## Scope — kernel, not service

**In scope:** detection, the ABI, the runtime, the toolchain that turns tiny models
into artifacts, signed distribution, and SDKs. **Out of scope, permanently:**
policy engines (block/allow/redact/route — your host's job, ⟐ the Go `operator`
suite's product), hosted inference APIs, LLM-as-judge, model training as a service.
The kernel returns spans precise enough that *you* can redact losslessly; it never
does it for you.

## Open core

OSS forever: the runtime, the ABI + toolchain, the CLI/SDKs, and the reference
detectors with their eval harness. The commercial seam is the **feed**: attacks
evolve, so continuously retrained detector artifacts — signed, versioned, delivered
through the same registry protocol the OSS tooling speaks — are the subscription.
The kernel cannot tell a paid registry from a free one; the feed is additive, never
required.

## Roadmap

The arc: ABI + `Verdict` + mock detector → config/CI gate → the wasmtime host →
pattern detectors (secrets/PII) → the ML toolchain + injection flagship → the eval
harness in CI → Rust/Go/Python SDKs → streaming detection → browser/edge parity →
signed distribution → sidecar → release + benchmark writeup. The full plan — with
its tombstone (policy in the kernel: cut by design) — is in
[`ROADMAP.md`](ROADMAP.md); its checkboxes are the **single source of truth** for
progress.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) — prerequisites, the local gate
(`cargo xtask ci`), the testing approach, and the invariants. The operating manual
is [`.rules`](.rules).

## License

Apache-2.0 — see [`LICENSE`](LICENSE).
