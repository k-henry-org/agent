//! `exub` — the command-line face of the exuberance engine.
//!
//! A grounded trade-*discovery* surface: it renders what the engine finds (cited candidates + evidence), it
//! never recommends. This binary is the composition root — it resolves layered config, initializes stderr
//! logging, then dispatches subcommands. Which data feed, AI model, or coding agent runs is **config, not
//! code**: names resolve to adapters in the [registry]. `mock` is the keyless default until real vendors
//! land (see ROADMAP.md).

#![forbid(unsafe_code)]

mod config;
mod logging;
mod registry;

use clap::{Parser, Subcommand};
use exub_core::ProviderKind;
use market_data::MockSource;
use signals::{scan, CheapVolCriteria};

#[derive(Parser)]
#[command(
    name = "exub",
    version,
    about = "Find trades, any market, any strategy — grounded, cited, never advised"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,

    /// Market-data adapter (default: `mock`). Also `EXUB_DATA_PROVIDER` / the config file.
    #[arg(long, global = true, value_name = "NAME")]
    data_provider: Option<String>,

    /// AI adapter — model or coding agent (default: `mock`). Also `EXUB_AI_PROVIDER` / the config file.
    #[arg(long, global = true, value_name = "NAME")]
    ai_provider: Option<String>,

    /// Execution mode: `paper` (default). Vestigial — the engine places no orders (see .rules
    /// guardrail #1); kept for the dormant broker seam.
    #[arg(long, global = true, value_name = "MODE")]
    trading_mode: Option<String>,

    /// Log filter, e.g. `debug` or `exub=debug` (default: `warn`). Also `EXUB_LOG`.
    #[arg(long, global = true)]
    log: Option<String>,

    /// Path to a TOML config file. Also `EXUB_CONFIG`.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<std::path::PathBuf>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the cheap-vol / proven-mover screen over the configured data provider.
    Scan,
    /// List the plug-in catalog (data feeds, AI models, coding agents, brokers) + the configured selection.
    Providers,
}

// A single-thread runtime is plenty: the CLI issues one sequential command over the async seams.
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // CLI flags are the top config layer; env (`EXUB_*`) and an optional TOML file sit under them.
    let flags = config::Partial {
        data_provider: cli.data_provider,
        ai_provider: cli.ai_provider,
        trading_mode: cli.trading_mode,
        log: cli.log,
    };
    let cfg = config::load(flags, cli.config.as_deref())?;
    logging::init(&cfg);
    tracing::debug!(
        data_provider = %cfg.data_provider,
        ai_provider = %cfg.ai_provider,
        trading_mode = %cfg.trading_mode,
        "exub: resolved config"
    );

    match cli.cmd {
        Cmd::Scan => run_scan(&cfg).await,
        Cmd::Providers => {
            list_providers(&cfg);
            Ok(())
        }
    }
}

/// Render the plug-in catalog — every data feed, AI model, coding agent, and broker the engine can (or
/// will) plug in — grouped by seam, plus the currently-configured selection. This is the pluggable,
/// multi-vendor surface made visible: a wired name is selectable today, a planned one is a named slot.
fn list_providers(cfg: &config::Config) {
    println!("configured selection (flags > env > file > defaults):\n");
    println!("  data provider   {}", cfg.data_provider);
    println!("  ai provider     {}", cfg.ai_provider);
    println!("  trading mode    {}", cfg.trading_mode);
    println!();

    print_group("market-data feeds", ProviderKind::MarketData);
    print_group("ai models", ProviderKind::Ai);
    print_group("ai coding agents", ProviderKind::Agent);
    print_group("brokers (human-initiated execution)", ProviderKind::Broker);

    println!(
        "\nadd a vendor = a new adapter + one registry arm; the engine only ever sees the trait."
    );
}

/// Print every catalog entry for one seam, aligned, tagged wired/planned.
fn print_group(title: &str, kind: ProviderKind) {
    println!("{title}:");
    for e in registry::catalog().iter().filter(|e| e.kind == kind) {
        println!("  [{:<7}] {:<14} {}", e.status.tag(), e.name, e.note);
    }
    println!();
}

/// Run the cheap-vol screen over the configured data provider. Today only `mock` is wired (a demo-seeded
/// universe); selecting a planned feed returns an actionable error from the registry rather than a silent
/// fallback. The output is cited evidence, not a recommendation.
async fn run_scan(cfg: &config::Config) -> anyhow::Result<()> {
    let source = registry::build_data_provider(&cfg.data_provider)?;
    let criteria = CheapVolCriteria {
        lookback_days: 100,
        ..Default::default()
    };
    let universe = MockSource::DEMO_UNIVERSE;
    let hits = scan(source.as_ref(), &universe, &criteria).await;

    println!(
        "cheap-vol screen ({}) — {} candidate(s) from {} scanned\n",
        cfg.data_provider,
        hits.len(),
        universe.len()
    );
    println!(
        "{:<8} {:>6} {:>8} {:>8} {:>8} {:>7}",
        "SYMBOL", "IV", "IVrank", "RVol", "RV/IV", "moves"
    );
    for r in &hits {
        println!(
            "{:<8} {:>5.0}% {:>8} {:>7.0}% {:>8} {:>7}",
            r.symbol,
            r.iv * 100.0,
            r.iv_rank
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "-".into()),
            r.realized_vol * 100.0,
            r.realized_over_implied
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "-".into()),
            r.big_moves,
        );
    }
    println!(
        "\n(demo data — evidence only, not a recommendation; wire a live feed for real results)"
    );
    Ok(())
}
