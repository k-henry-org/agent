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
use signals::{scan_with, CheapVolCriteria, CheapVolResult};

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
    Scan {
        /// Comma-separated symbols to scan (e.g. `AAPL,TSLA,NVDA`). Defaults to the built-in
        /// demo universe. Real universe input — index constituents, watchlists — is Phase 10.
        #[arg(long, value_delimiter = ',', value_name = "SYM,…")]
        symbols: Option<Vec<String>>,
    },
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
        Cmd::Scan { symbols } => run_scan(&cfg, symbols).await,
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
    print_group(
        "ai models (dormant seam — agents connect over MCP)",
        ProviderKind::Ai,
    );
    print_group(
        "ai coding agents (dormant seam — they connect as MCP clients)",
        ProviderKind::Agent,
    );
    print_group(
        "brokers (dormant seam — the engine places no orders)",
        ProviderKind::Broker,
    );

    println!(
        "\nadd a data feed = a new adapter + one registry arm; the engine only ever sees the trait.\n\
         dormant entries document seams no phase wires (see the ROADMAP tombstones)."
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
///
/// Rows **stream** as each candidate is found (arrival order — the point once a slow real
/// feed is scanning a big universe); the footer then names the most-underpriced hit from
/// the sorted result, so the ranked answer survives streaming. A fully sortable/filterable
/// view is `--json`'s job (ROADMAP P11.3), which stays atomic.
async fn run_scan(cfg: &config::Config, symbols: Option<Vec<String>>) -> anyhow::Result<()> {
    let source = registry::build_data_provider(&cfg.data_provider)?;
    let criteria = CheapVolCriteria {
        lookback_days: 100,
        ..Default::default()
    };
    // Explicit `--symbols`, else the built-in demo universe (real universe input is Phase 10).
    let owned: Vec<String> = symbols.unwrap_or_else(|| {
        MockSource::DEMO_UNIVERSE
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    });
    let universe: Vec<&str> = owned.iter().map(String::as_str).collect();

    println!(
        "cheap-vol screen ({}) — scanning {} symbols, rows stream as found\n",
        cfg.data_provider,
        universe.len()
    );
    println!(
        "{:<8} {:>6} {:>8} {:>8} {:>8} {:>7}",
        "SYMBOL", "IV", "IVrank", "RVol", "RV/IV", "moves"
    );
    let hits = scan_with(source.as_ref(), &universe, &criteria, print_hit_row).await;

    println!(
        "\n{} candidate(s) from {} scanned",
        hits.len(),
        universe.len()
    );
    if hits.len() > 1 {
        // The sorted return ranks most-underpriced first; surface that answer even though
        // the rows above streamed in arrival order.
        if let (Some(best), Some(spread)) = (hits.first(), hits[0].implied_realized_spread) {
            println!(
                "most underpriced: {} (implied−realized {:+.0}%)",
                best.symbol,
                spread * 100.0
            );
        }
    }
    println!(
        "(demo data — evidence only, not a recommendation; wire a live feed for real results)"
    );
    Ok(())
}

/// Print one evidence row of the streaming scan table. `-` marks a value the engine could
/// not compute (never a fabricated zero).
fn print_hit_row(r: &CheapVolResult) {
    println!(
        "{:<8} {:>5.0}% {:>8} {:>8} {:>8} {:>7}",
        r.symbol,
        r.iv * 100.0,
        r.iv_rank
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "-".into()),
        r.realized_vol
            .map(|v| format!("{:.0}%", v * 100.0))
            .unwrap_or_else(|| "-".into()),
        r.realized_over_implied
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "-".into()),
        r.big_moves,
    );
}
