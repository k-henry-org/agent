//! The adapter registry — the **one place** a data provider, AI model, or coding agent is named.
//!
//! This is what makes exuberance a pluggable, multi-vendor engine: the config resolves a *name*
//! (`"mock"`, `"massive"`, `"claude"`, `"claude-code"`, …) and the registry maps it to a boxed trait object
//! the engine drives. Adding a vendor is a new adapter crate + one arm here — nothing in the core or the
//! screens changes. The [`catalog`] lists every intended vendor and whether it's wired yet, so
//! `exub providers` shows the plug-in matrix at a glance.

use anyhow::{bail, Result};
use exub_core::{AiProvider, EchoAi, MarketDataProvider, ProviderKind};
#[cfg(feature = "massive-live")]
use market_data::MassiveSource;
use market_data::MockSource;

/// Whether a catalog entry is implemented, on the roadmap, or a dormant seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Implemented and selectable today.
    Wired,
    /// A named plug-in point on the active roadmap (see ROADMAP.md).
    Planned,
    /// Documents a **dormant seam**: stays here so the seam's shape is visible, but no
    /// phase wires it — permanently `dormant` unless explicitly re-scoped (see the ROADMAP
    /// Phases 15–16 and 19–22 tombstones). Selecting one is an error that names the truth.
    Dormant,
}

impl Status {
    /// A short tag for display.
    pub fn tag(self) -> &'static str {
        match self {
            Status::Wired => "wired",
            Status::Planned => "planned",
            Status::Dormant => "dormant",
        }
    }
}

/// One vendor the engine can (or will) plug in.
#[derive(Clone, Debug)]
pub struct CatalogEntry {
    /// The name used in config / `--data-provider` / `--ai-provider`.
    pub name: &'static str,
    /// Which seam it plugs into.
    pub kind: ProviderKind,
    /// Wired today, or planned.
    pub status: Status,
    /// A human note (what it is).
    pub note: &'static str,
}

/// The full plug-in catalog across every seam — data feeds, AI models, AI coding agents, and brokers.
/// This is the menu `exub providers` renders; `build_*` below turns a wired name into a live adapter.
pub fn catalog() -> Vec<CatalogEntry> {
    use ProviderKind::{Agent, Ai, Broker, MarketData};
    use Status::{Dormant, Planned, Wired};
    vec![
        // Market-data feeds.
        CatalogEntry {
            name: "mock",
            kind: MarketData,
            status: Wired,
            note: "in-memory demo/test fixture",
        },
        CatalogEntry {
            name: "massive",
            kind: MarketData,
            // Wired once the `massive-live` feature is on (the default build); `Planned` in a
            // lean `--no-default-features` build where the HTTP adapter isn't compiled.
            status: if cfg!(feature = "massive-live") {
                Wired
            } else {
                Planned
            },
            note: "Massive (formerly Polygon.io) — licensed EOD feed; IV snapshot → accumulate history",
        },
        CatalogEntry {
            name: "alpha-vantage",
            kind: MarketData,
            status: Planned,
            note:
                "Alpha Vantage — historical options w/ IV → backfill history (heavily rate-limited)",
        },
        // AI models — a DORMANT seam: the engine never calls a model; agents connect over
        // MCP with their own model + key (ROADMAP Phases 15–16 tombstone, Phase 17).
        CatalogEntry {
            name: "mock",
            kind: Ai,
            status: Wired,
            note: "deterministic echo model (keyless) — exercises the dormant seam",
        },
        CatalogEntry {
            name: "claude",
            kind: Ai,
            status: Dormant,
            note: "Anthropic Claude — not wired by design; connect via MCP (Phase 17)",
        },
        CatalogEntry {
            name: "gemini",
            kind: Ai,
            status: Dormant,
            note: "Google Gemini — not wired by design; connect via MCP (Phase 17)",
        },
        CatalogEntry {
            name: "openai",
            kind: Ai,
            status: Dormant,
            note: "OpenAI — not wired by design; connect via MCP (Phase 17)",
        },
        // AI coding agents — same dormant AiProvider seam; these connect as MCP *clients*.
        CatalogEntry {
            name: "claude-code",
            kind: Agent,
            status: Dormant,
            note: "Claude Code — connects as an MCP client, brings its own model",
        },
        CatalogEntry {
            name: "gemini-cli",
            kind: Agent,
            status: Dormant,
            note: "Gemini CLI — connects as an MCP client, brings its own model",
        },
        CatalogEntry {
            name: "codex",
            kind: Agent,
            status: Dormant,
            note: "OpenAI Codex — connects as an MCP client, brings its own model",
        },
        // Brokers — a DORMANT seam: execution is cut by design (Phases 19–22 tombstone);
        // the engine places no orders. PaperBroker is the seam's inert reference mock.
        CatalogEntry {
            name: "paper",
            kind: Broker,
            status: Wired,
            note: "no-network paper mock — the dormant seam's inert reference impl",
        },
        CatalogEntry {
            name: "tradier",
            kind: Broker,
            status: Dormant,
            note: "Tradier — not wired by design; the engine places no orders",
        },
        CatalogEntry {
            name: "alpaca",
            kind: Broker,
            status: Dormant,
            note: "Alpaca — not wired by design; the engine places no orders",
        },
    ]
}

/// The wired names for a kind, for an actionable "unknown/planned provider" message.
fn wired_names(kind: ProviderKind) -> Vec<&'static str> {
    catalog()
        .into_iter()
        .filter(|e| e.kind == kind && e.status == Status::Wired)
        .map(|e| e.name)
        .collect()
}

/// Resolve a market-data adapter by name. `mock` returns a demo-seeded source; everything else is a named
/// but not-yet-wired plug-in point (a clear error, not a silent fallback).
///
/// # Errors
/// If `name` is unknown or catalogued-but-planned.
pub fn build_data_provider(name: &str) -> Result<Box<dyn MarketDataProvider>> {
    match name {
        "mock" => Ok(Box::new(MockSource::demo())),
        // `?` turns a `ProviderError` (e.g. a missing `MASSIVE_API_KEY` → `Auth`) into the
        // CLI's `anyhow::Error` via the std::error::Error impl (pinned in P3.2).
        #[cfg(feature = "massive-live")]
        "massive" => Ok(Box::new(MassiveSource::from_env()?)),
        other => bail!(
            "data provider '{other}' is not wired yet (available: {}). \
             Run `exub providers` to see the catalog; the wiring plan is in ROADMAP.md.",
            wired_names(ProviderKind::MarketData).join(", ")
        ),
    }
}

/// Resolve an AI adapter by name. `mock` returns the keyless echo provider — the only impl
/// the **dormant** `AiProvider` seam will ever have unless in-engine model calls are
/// explicitly re-scoped (ROADMAP Phases 15–16 tombstone). Kept so the seam stays exercised;
/// `allow(dead_code)` because no production command drives it (by design).
///
/// # Errors
/// If `name` is unknown or a dormant vendor — the message says the truth: agents connect
/// over MCP, the engine never calls a model.
#[allow(dead_code)]
pub fn build_ai_provider(name: &str) -> Result<Box<dyn AiProvider>> {
    match name {
        "mock" => Ok(Box::new(EchoAi::new("echo", "echo-1"))),
        other => bail!(
            "ai provider '{other}' is not wired — by design, not yet: the engine never \
             calls a model (available: {}). Agents connect over MCP and bring their own \
             model (ROADMAP Phase 17). Run `exub providers` to see the catalog.",
            wired_names(ProviderKind::Ai).join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_every_seam_with_the_named_vendors() {
        let c = catalog();
        let has =
            |name: &str, kind: ProviderKind| c.iter().any(|e| e.name == name && e.kind == kind);
        // The vendors the product targets are all present as plug-in points.
        assert!(has("massive", ProviderKind::MarketData));
        assert!(has("alpha-vantage", ProviderKind::MarketData));
        assert!(has("claude", ProviderKind::Ai));
        assert!(has("gemini", ProviderKind::Ai));
        assert!(has("openai", ProviderKind::Ai));
        assert!(has("claude-code", ProviderKind::Agent));
        assert!(has("gemini-cli", ProviderKind::Agent));
        assert!(has("codex", ProviderKind::Agent));
    }

    /// The catalog tells the truth about the re-scope: data feeds are the only `planned`
    /// entries (an active roadmap); every AI-model / coding-agent / broker vendor is
    /// `dormant` (the seams no phase wires — tombstones 15–16 and 19–22).
    #[test]
    fn dormant_seams_are_marked_dormant_not_planned() {
        for e in catalog() {
            match e.kind {
                ProviderKind::MarketData => assert_ne!(
                    e.status,
                    Status::Dormant,
                    "{} is a data feed — the active seam is never dormant",
                    e.name
                ),
                _ => assert_ne!(
                    e.status,
                    Status::Planned,
                    "{} rides a dormant seam — it must be wired (a mock) or dormant, \
                     never 'planned' (nothing on the roadmap wires it)",
                    e.name
                ),
            }
        }
    }

    #[tokio::test]
    async fn build_resolves_mock_and_rejects_planned() {
        // The wired defaults build and actually work.
        let data = build_data_provider("mock").unwrap();
        assert!(data.daily_bars("MOVER", 100).await.is_ok());
        assert!(build_ai_provider("mock").is_ok());

        // A catalogued-but-planned vendor is a clear, actionable error, not a silent
        // fallback: it names the alternatives AND the command that shows the catalog.
        // `alpha-vantage` is always planned (never wired), unlike `massive` which flips with
        // the feature — asserted separately below. `.err()` (not `unwrap_err`) because the
        // Ok type `Box<dyn MarketDataProvider>` isn't `Debug`.
        let err = build_data_provider("alpha-vantage")
            .err()
            .expect("a planned provider must error")
            .to_string();
        assert!(
            err.contains("not wired yet") && err.contains("mock") && err.contains("exub providers"),
            "{err}"
        );

        // `massive` flips with the feature: under `massive-live` it's wired and selecting it
        // fails only on the missing key (Auth), not "not wired"; without the feature it's a
        // planned plug-in point like any other.
        #[cfg(feature = "massive-live")]
        {
            std::env::remove_var("MASSIVE_API_KEY");
            let err = build_data_provider("massive")
                .err()
                .expect("no key → error")
                .to_string();
            assert!(err.contains("MASSIVE_API_KEY"), "{err}");
        }
        #[cfg(not(feature = "massive-live"))]
        assert!(build_data_provider("massive").is_err());

        // A dormant AI vendor errors with the truth: by design, MCP is the path.
        let err = build_ai_provider("claude")
            .err()
            .expect("a dormant vendor must error")
            .to_string();
        assert!(err.contains("by design") && err.contains("MCP"), "{err}");
    }
}
