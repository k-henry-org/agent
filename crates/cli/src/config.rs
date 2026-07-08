//! Layered configuration (12-factor): **defaults < file (TOML) < env (`EXUB_*`) < flags**.
//!
//! Which market-data and AI adapter runs — and whether execution is paper or live — is *config, not code*:
//! a new adapter is reachable by name without touching a call site. **Secrets never live here.** API keys
//! come from provider-native env vars (`MASSIVE_API_KEY`, `ANTHROPIC_API_KEY`, …) read at the adapter edge,
//! never from this struct or the config file — so a config dump can't leak a key.
//!
//! IO and logic are separated: [`resolve`] is a pure fold over the layers (unit-tested for precedence),
//! while [`load`] does the impure env-read + file-read and then calls it.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

/// The resolved configuration the CLI runs with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Which market-data adapter to use (e.g. `"mock"`, later `"massive"`).
    pub data_provider: String,
    /// Which AI adapter to use (e.g. `"mock"`, later `"claude"`).
    pub ai_provider: String,
    /// Execution mode. Defaults to `"paper"` and is vestigial: the engine contains no execution
    /// path (see `.rules` guardrail #1 — ROADMAP Phases 19–22 tombstone), so no value of this
    /// field can place an order. Kept for the dormant broker seam.
    pub trading_mode: String,
    /// The `tracing` filter directive (e.g. `"warn"`, `"debug"`, `"exub=debug"`).
    pub log: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_provider: "mock".to_owned(),
            ai_provider: "mock".to_owned(),
            trading_mode: "paper".to_owned(),
            log: "warn".to_owned(),
        }
    }
}

/// One layer's overrides — every field optional, so layers merge cleanly. Deserialized from the TOML file
/// and also built from env vars and CLI flags.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Partial {
    /// Override the market-data adapter.
    pub data_provider: Option<String>,
    /// Override the AI adapter.
    pub ai_provider: Option<String>,
    /// Override the execution mode.
    pub trading_mode: Option<String>,
    /// Override the log filter.
    pub log: Option<String>,
}

impl Partial {
    /// Overrides from the `EXUB_*` environment (empty values are treated as unset).
    fn from_env() -> Self {
        Self {
            data_provider: env_var("EXUB_DATA_PROVIDER"),
            ai_provider: env_var("EXUB_AI_PROVIDER"),
            trading_mode: env_var("EXUB_TRADING_MODE"),
            log: env_var("EXUB_LOG"),
        }
    }

    /// Apply this layer over `base`: every `Some` field wins, `None` leaves `base` untouched.
    fn apply(self, base: &mut Config) {
        if let Some(v) = self.data_provider {
            base.data_provider = v;
        }
        if let Some(v) = self.ai_provider {
            base.ai_provider = v;
        }
        if let Some(v) = self.trading_mode {
            base.trading_mode = v;
        }
        if let Some(v) = self.log {
            base.log = v;
        }
    }
}

/// Read an `EXUB_*` env var, treating empty as unset.
fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

/// Fold the layers over the defaults in precedence order (lowest first): `file`, then `env`, then `flags`.
/// Pure — the whole point of the split, so precedence is testable without touching the real environment.
#[must_use]
pub fn resolve(file: Partial, env: Partial, flags: Partial) -> Config {
    let mut config = Config::default();
    file.apply(&mut config);
    env.apply(&mut config);
    flags.apply(&mut config);
    config
}

/// Load config from an optional file plus the environment, then apply the CLI `flags` on top.
///
/// The file is read only from an explicit path — the `--config` flag (`config_path`) or the `EXUB_CONFIG`
/// env var, in that order. A requested-but-unreadable/invalid file is an error; no path means no file layer
/// (there is no surprise CWD auto-scan).
///
/// # Errors
/// If a config-file path is given but can't be read or parsed as TOML.
pub fn load(flags: Partial, config_path: Option<&Path>) -> anyhow::Result<Config> {
    let path: Option<PathBuf> = config_path
        .map(Path::to_path_buf)
        .or_else(|| env_var("EXUB_CONFIG").map(PathBuf::from));

    let file = match path {
        Some(path) => {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading config file {}", path.display()))?;
            toml::from_str(&text)
                .with_context(|| format!("parsing config file {}", path.display()))?
        }
        None => Partial::default(),
    };

    Ok(resolve(file, Partial::from_env(), flags))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partial(data: Option<&str>, mode: Option<&str>, log: Option<&str>) -> Partial {
        Partial {
            data_provider: data.map(str::to_owned),
            ai_provider: None,
            trading_mode: mode.map(str::to_owned),
            log: log.map(str::to_owned),
        }
    }

    #[test]
    fn defaults_are_mock_paper_and_quiet() {
        let c = resolve(Partial::default(), Partial::default(), Partial::default());
        assert_eq!(c, Config::default());
        assert_eq!(
            (
                c.data_provider.as_str(),
                c.ai_provider.as_str(),
                c.trading_mode.as_str(),
                c.log.as_str()
            ),
            ("mock", "mock", "paper", "warn")
        );
    }

    #[test]
    fn precedence_is_flags_over_env_over_file_over_defaults() {
        let file = partial(Some("file-data"), Some("live"), Some("file-log"));
        let env = partial(Some("env-data"), None, Some("env-log"));
        let flags = partial(Some("flag-data"), None, None);
        let c = resolve(file, env, flags);
        assert_eq!(c.data_provider, "flag-data"); // flag wins
        assert_eq!(c.trading_mode, "live"); // only the file set it
        assert_eq!(c.log, "env-log"); // env over file, no flag
    }

    #[test]
    fn empty_partials_fall_through_to_defaults() {
        let c = resolve(
            partial(None, None, None),
            partial(None, None, Some("debug")),
            partial(None, None, None),
        );
        assert_eq!(c.data_provider, "mock");
        assert_eq!(c.trading_mode, "paper");
        assert_eq!(c.log, "debug");
    }

    #[test]
    fn toml_file_parses_into_a_partial() {
        let p: Partial = toml::from_str("data_provider = \"massive\"\ntrading_mode = \"paper\"\n")
            .expect("valid toml");
        assert_eq!(p.data_provider.as_deref(), Some("massive"));
        assert_eq!(p.ai_provider, None);
        assert_eq!(p.trading_mode.as_deref(), Some("paper"));
    }

    #[test]
    fn unknown_toml_key_is_rejected() {
        assert!(toml::from_str::<Partial>("provdier = \"massive\"\n").is_err());
    }

    #[test]
    fn load_errors_on_a_missing_requested_file() {
        let missing = Path::new("/no/such/exuberance-config.toml");
        assert!(load(Partial::default(), Some(missing)).is_err());
    }
}
