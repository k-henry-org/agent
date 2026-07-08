# Architecture decisions

The record [`ROADMAP.md`](./ROADMAP.md) §0 references: every roadmap item tagged
`(decision)` produces a dated entry here — the decision, the alternatives considered, and
the why — so the reasoning outlives the diff. Entries are append-only and numbered;
reversing one is a new entry, not an edit. (Roadmap *re-scopes* — cut phases and why — are
recorded in the roadmap's tombstones, not duplicated here. P13.2 consolidates this file
before any release.)

The prior project's decision log (the retired trading engine) was cleared with the
2026-07-08 repurpose; its entries live in git history if ever needed.

Decisions queued by the roadmap, to be recorded here as they're made:
- **P1.2** — ABI v0 shape: WASM component model (WIT) vs plain core-wasm exports.
- **P3.3** — instance lifecycle on the hot path: pooling vs instance-per-call.
- **P4.2** — PII locale scope for v0.
- **P5.1** — inference approach inside the artifact (pure-Rust linear vs compiled inference
  lib; fixed-point vs float, with the cross-host determinism requirement).
- **P10.2** — registry transport: OCI vs plain HTTPS index.
- **P12.1** — sidecar protocol surface: HTTP-only vs +gRPC.

---

*(no entries yet — the first lands with P1.2)*
