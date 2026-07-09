//! The `agent` CLI — run a detector over text and print a cited `Verdict`.
//!
//! Configuration is layered **flags > env (`AGENT_*`) > file (TOML) > defaults** (see
//! [`config`]). Phase 1 wired the `check` subcommand to the **native** mock detector — no
//! wasmtime yet; the Phase-3 host swaps in a `WasmDetector` behind the same
//! `agent_abi::Detector` trait, and this surface doesn't change. Exit codes are part of the
//! wire contract: `0` clean, `1` a detection fired, `2` an operational error.
#![forbid(unsafe_code)]

mod config;

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use agent_abi::{Detector, Verdict};
use clap::{Parser, Subcommand};

use crate::config::{resolve, Config, Partial};

#[derive(Parser)]
#[command(
    name = "agent",
    about = "guardrail detection — detects and cites, never decides"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,

    /// TOML config file. Precedence: flags > env (`AGENT_*`) > this file > defaults.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Detector to run (overrides `AGENT_DETECTOR` and the config file).
    #[arg(long, global = true, value_name = "NAME")]
    detector: Option<String>,

    /// Log filter for stderr, e.g. `warn`, `debug`, `agent=debug` (overrides `AGENT_LOG`).
    #[arg(long, global = true, value_name = "FILTER")]
    log: Option<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a detector over text (an argument, or stdin) and print a cited Verdict.
    Check(CheckArgs),
}

#[derive(clap::Args)]
struct CheckArgs {
    /// Emit the Verdict as JSON on stdout (the wire contract) instead of a human summary.
    #[arg(long)]
    json: bool,
    /// The text to scan. If omitted, read all of stdin.
    text: Option<String>,
}

/// The outcome of a check, mapped to the process exit code — avoids boolean blindness at the
/// exit boundary. (Operational errors are the `Err` arm and map to exit 2.)
enum Outcome {
    /// No findings.
    Clean,
    /// The detector fired.
    Detected,
}

impl Outcome {
    fn exit_code(self) -> ExitCode {
        match self {
            Outcome::Clean => ExitCode::SUCCESS,    // 0
            Outcome::Detected => ExitCode::from(1), // a detection fired
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(outcome) => outcome.exit_code(),
        Err(e) => {
            eprintln!("agent: {e:#}");
            ExitCode::from(2) // operational error
        }
    }
}

fn run() -> anyhow::Result<Outcome> {
    // Destructure so the flag values move straight into the config layer — nothing is cloned.
    let Cli {
        cmd,
        config,
        detector,
        log,
    } = Cli::parse();
    let cfg = load_config(config, detector, log)?;
    init_tracing(&cfg.log);
    tracing::debug!(detector = %cfg.detector, log = %cfg.log, "resolved config");
    match cmd {
        Cmd::Check(args) => run_check(&cfg, args),
    }
}

/// Initialize stderr logging, filtered by the resolved log directive. **stdout stays reserved
/// for verdicts**, so `agent check … 2>/dev/null` is pipe-clean. An invalid filter falls back to
/// `warn` rather than failing the run (a bad log setting must never break detection).
fn init_tracing(filter: &str) {
    let env_filter = tracing_subscriber::EnvFilter::try_new(filter)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(env_filter)
        .with_target(false)
        .try_init();
}

/// Fold the config layers: flags > env (`AGENT_*`) > `--config` file > defaults. Takes the flag
/// values by value so nothing is cloned.
fn load_config(
    config: Option<PathBuf>,
    detector: Option<String>,
    log: Option<String>,
) -> anyhow::Result<Config> {
    let file = match config {
        Some(path) => Partial::from_toml_file(&path)?,
        None => Partial::default(),
    };
    let env = Partial::from_env();
    let flags = Partial {
        detector,
        log,
        artifact_dir: None,
    };
    Ok(resolve(file, env, flags))
}

/// Run `agent check`, returning the [`Outcome`] that maps to the exit code.
fn run_check(cfg: &Config, args: CheckArgs) -> anyhow::Result<Outcome> {
    let text = match args.text {
        Some(t) => t,
        None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };

    tracing::info!(detector = %cfg.detector, bytes = text.len(), "running check");
    let verdict = detect(&cfg.detector, &text)?;
    tracing::debug!(
        findings = verdict.findings.len(),
        fired = verdict.fired(),
        "verdict"
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&verdict)?);
    } else {
        render(&verdict, &text);
    }
    Ok(if verdict.fired() {
        Outcome::Detected
    } else {
        Outcome::Clean
    })
}

/// Resolve a detector name to its `Detector` and run it. An unknown name is an operational
/// error (exit 2), never a panic.
fn detect(name: &str, text: &str) -> anyhow::Result<Verdict> {
    match name {
        "mock" => Ok(agent_abi::mock::MockDetector::new().detect(text)),
        other => anyhow::bail!("unknown detector '{other}' (available: mock)"),
    }
}

/// Human-readable render to stdout: a header plus one line per finding with its span, score,
/// and the matched excerpt.
fn render(verdict: &Verdict, text: &str) {
    let p = &verdict.provenance;
    if verdict.findings.is_empty() {
        println!(
            "clean — no findings ({} v{})",
            p.detector_id, p.detector_version
        );
        return;
    }
    println!(
        "{} finding(s) — {} v{}:",
        verdict.findings.len(),
        p.detector_id,
        p.detector_version
    );
    for f in &verdict.findings {
        let (start, end) = (f.span.start as usize, f.span.end as usize);
        let excerpt = text.get(start..end).unwrap_or("");
        println!(
            "  [{start}..{end}] {} (score {:.2}) \u{201c}{excerpt}\u{201d}",
            f.label, f.score
        );
    }
}
