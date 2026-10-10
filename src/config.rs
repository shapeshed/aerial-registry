//! Layered application configuration.
//!
//! Values are read from a TOML file — `config/default.toml` by default, or the
//! path named by `AERIAL_CONFIG` — and then overridden by environment variables
//! using the `AERIAL__SECTION__KEY` convention:
//!
//! ```text
//! AERIAL__RADIO_FRANCE__API_KEY=...
//! AERIAL__SKIP_LIVENESS=true
//! ```
//!
//! The file is for local development; CI passes secrets as environment
//! variables, which take precedence. See `config/default.toml.example`.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    /// Comma-separated provider slugs to run (e.g. `global,bbc`). Empty or
    /// absent runs every provider, which is the normal nightly behaviour.
    #[serde(default)]
    pub providers: Option<String>,
    /// Skip the liveness probes entirely (nothing is pruned).
    #[serde(default)]
    pub skip_liveness: Option<bool>,
    /// Skip Radio Browser tag enrichment (faster local builds).
    #[serde(default)]
    pub skip_enrich: Option<bool>,
    /// Station state store (SQLite) used for liveness hysteresis. An empty
    /// string disables it; absent uses the default path.
    #[serde(default)]
    pub state_db: Option<String>,
    /// Previously shipped registry (`registry.json`/`.gz`) to guard against and
    /// diff against. Absent skips the guard and diff report.
    #[serde(default)]
    pub previous_registry_path: Option<String>,
    #[serde(default)]
    pub radio_france: RadioFranceConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RadioFranceConfig {
    /// API key from <https://developers.radiofrance.fr/>, sent as `x-token`.
    #[serde(default)]
    pub api_key: Option<String>,
}

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

/// The process-wide configuration, loaded once on first access.
pub fn get() -> &'static AppConfig {
    CONFIG.get_or_init(|| {
        let path = std::env::var("AERIAL_CONFIG").unwrap_or_else(|_| DEFAULT_PATH.to_string());
        load(&path)
    })
}

const DEFAULT_PATH: &str = "config/default";

fn load(path: &str) -> AppConfig {
    let parsed = config::Config::builder()
        .add_source(config::File::with_name(path).required(false))
        .add_source(env_source())
        .build()
        .and_then(|c| c.try_deserialize::<AppConfig>());

    match parsed {
        Ok(config) => config,
        Err(error) => {
            eprintln!("aerial-registry: invalid configuration ({path}): {error}");
            std::process::exit(1);
        }
    }
}

/// The `AERIAL__SECTION__KEY` environment source. Env values override the file.
fn env_source() -> config::Environment {
    config::Environment::with_prefix("AERIAL")
        .separator("__")
        .try_parsing(true)
        .ignore_empty(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn env_vars_map_to_nested_config_keys() {
        let mut env = HashMap::new();
        env.insert(
            "AERIAL__RADIO_FRANCE__API_KEY".to_string(),
            "secret".to_string(),
        );
        env.insert("AERIAL__SKIP_LIVENESS".to_string(), "true".to_string());
        env.insert("AERIAL__PROVIDERS".to_string(), "bbc,global".to_string());

        let config: AppConfig = config::Config::builder()
            .add_source(env_source().source(Some(env)))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(config.radio_france.api_key.as_deref(), Some("secret"));
        assert_eq!(config.skip_liveness, Some(true));
        assert_eq!(config.providers.as_deref(), Some("bbc,global"));
        assert_eq!(config.skip_enrich, None);
    }
}
