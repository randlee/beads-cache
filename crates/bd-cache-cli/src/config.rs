//! Configuration (contract §2, REQ-BDC-0810): flag > `BD_CACHE__*` env > TOML > default.
//!
//! - `--repo` (env `BD_CACHE__REPO`) is the repository root; the default is
//!   the working directory. There is **no upward search**: the same command
//!   always opens the same file.
//! - `.beads/bd-cache.toml` is read from `<repo>/.beads/` only. A missing
//!   file is the defaults. A top-level `bd` key is ignored with a warning: the
//!   bd binary is never chosen by a checked-in file.
//! - `metadata.json` is read from `$BEADS_DIR` when that is set (in the
//!   environment given to [`crate::run`]), else from `<repo>/.beads/`.
//! - The TOML keys can be overridden from the env under the same name
//!   (`BD_CACHE__CALL_TIMEOUT_S`; `BD_CACHE__SCOPES` comma-separated), and
//!   `--cache-path` wins over both. There is no `[tick]` table: the file
//!   cannot name code to run (REQ-BDC-0724).
//! - The cache file is found by the store's rules or named by `cache_path`;
//!   no consumer path is known (ADR-BDC-0015 amends ADR-CLI-0004).
//!
//! The merged [`RepoConfig`] is opened through
//! `bd_cache_sync::compose_with_config`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use bd_cache_api::ApiError;
use bd_cache_model::{BdKey, RepoConfig};
use bd_cache_store::{
    BEADS_DIR_ENV, BeadsDirOverride, ConfigLoadError, REPO_CONFIG_FILE, load_repo_config_with,
};
use serde_json::{Map, Value, json};

/// The prefix of every environment override (sc-config's naming rule).
pub(crate) const ENV_PREFIX: &str = "BD_CACHE__";

/// The `details.source` of a refusal this crate decides itself: an env value
/// that does not parse.
const SOURCE_ENV: &str = "bdc_env_invalid";

/// The env keys (after [`ENV_PREFIX`]) bdc reads.
const KNOWN_ENV: &[&str] = &[
    "REPO",
    "BD",
    "LOG",
    "CACHE_PATH",
    "SCOPES",
    "CALL_TIMEOUT_S",
    "MISS_FILL_TIMEOUT_MS",
    "MAX_ATTEMPTS",
];

/// The flags that take part in configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Flags {
    /// `--repo`.
    pub(crate) repo: Option<String>,
    /// `--cache-path`.
    pub(crate) cache_path: Option<String>,
    /// `--bd`.
    pub(crate) bd: Option<String>,
}

/// Everything a command needs to open its cache.
#[derive(Debug, Clone)]
pub(crate) struct Settings {
    /// The repository root, absolute.
    pub(crate) repo: PathBuf,
    /// The merged configuration.
    pub(crate) config: RepoConfig,
    /// The bd binary.
    pub(crate) bd: String,
    /// `$BEADS_DIR` from the given environment.
    pub(crate) beads_dir_env: Option<OsString>,
}

/// Resolves the configuration of one command.
///
/// # Errors
///
/// `CONFIG_INVALID` (with `details.source`) for a TOML file that cannot be
/// read or parsed, an env value that does not parse, a merged value that
/// cannot work, or an unresolvable `$BEADS_DIR`. The cache file and
/// `metadata.json` are resolved once, when the cache is opened.
pub(crate) fn resolve(flags: &Flags, env: &BTreeMap<String, String>) -> Result<Settings, ApiError> {
    let repo = repo_root(flags, env)?;
    let mut config = read_toml(&repo)?;
    apply_env(&mut config, env)?;
    if let Some(p) = &flags.cache_path {
        config.cache_path = Some(PathBuf::from(p));
    }
    validate(&config)?;
    let bd = flags
        .bd
        .clone()
        .or_else(|| env_value(env, "BD").map(str::to_owned))
        .unwrap_or_else(|| "bd".to_owned());
    let beads_dir_env = env
        .get(BEADS_DIR_ENV)
        .filter(|v| !v.is_empty())
        .map(OsString::from);
    // Validated here, before any command runs; the cache is opened (and the
    // cache file and `metadata.json` resolved) once, by the composition.
    BeadsDirOverride::new(beads_dir_env.as_deref()).map_err(|e| {
        ApiError::config_invalid(
            "sync_beads_dir_unresolvable",
            e.to_string(),
            details(&[("key", json!(BEADS_DIR_ENV))]),
        )
    })?;
    Ok(Settings {
        repo,
        config,
        bd,
        beads_dir_env,
    })
}

/// `BD_CACHE__<key>`, when set.
pub(crate) fn env_value<'a>(env: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    env.get(&format!("{ENV_PREFIX}{key}")).map(String::as_str)
}

/// The `BD_CACHE__*` keys bdc does not read (each one is logged as ignored).
pub(crate) fn unknown_env_keys(env: &BTreeMap<String, String>) -> Vec<&str> {
    env.keys()
        .filter_map(|k| k.strip_prefix(ENV_PREFIX))
        .filter(|k| !KNOWN_ENV.contains(k))
        .collect()
}

fn details(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect()
}

fn repo_root(flags: &Flags, env: &BTreeMap<String, String>) -> Result<PathBuf, ApiError> {
    let given = flags
        .repo
        .as_deref()
        .or_else(|| env_value(env, "REPO"))
        .unwrap_or(".");
    std::path::absolute(given).map_err(|e| {
        ApiError::config_invalid(
            "sync_beads_dir_unresolvable",
            format!("the repository root {given:?} cannot be made absolute: {e}"),
            details(&[("key", json!("repo"))]),
        )
    })
}

fn load_error(e: &ConfigLoadError) -> ApiError {
    let path = match e {
        ConfigLoadError::Read { path, .. }
        | ConfigLoadError::Parse { path, .. }
        | ConfigLoadError::Scope { path, .. } => path,
    };
    let cause = std::error::Error::source(e).map(ToString::to_string);
    let message = match cause {
        Some(c) => format!("{e} ({c})"),
        None => e.to_string(),
    };
    ApiError::config_invalid(
        e.code(),
        message,
        details(&[("path", json!(path.display().to_string()))]),
    )
    .with_suggested_action(e.fix())
}

/// Reads `<repo>/.beads/bd-cache.toml` through `bd-cache-store`'s one loader,
/// which drops a top-level `bd` key (never read from TOML) and says so; the
/// warning is the CLI's.
fn read_toml(repo: &Path) -> Result<RepoConfig, ApiError> {
    let loaded = load_repo_config_with(repo, BdKey::Strip).map_err(|e| load_error(&e))?;
    if loaded.bd_key_stripped {
        sc_beads::logging::tracing::warn!(
            file = %repo.join(REPO_CONFIG_FILE).display(),
            "bd in .beads/bd-cache.toml is ignored: the bd binary comes from --bd or BD_CACHE__BD"
        );
    }
    Ok(loaded.config)
}

fn env_invalid(key: &str, value: &str, expected: &str) -> ApiError {
    let name = format!("{ENV_PREFIX}{key}");
    ApiError::config_invalid(
        SOURCE_ENV,
        format!("{name}={value:?} is not {expected}"),
        details(&[
            ("key", json!(key.to_ascii_lowercase())),
            ("env", json!(name)),
        ]),
    )
    .with_suggested_action(format!("set {name} to {expected}, or unset it"))
}

fn env_number<T: std::str::FromStr>(
    env: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<T>, ApiError> {
    env_value(env, key)
        .map(|v| {
            v.trim()
                .parse::<T>()
                .map_err(|_unparsed| env_invalid(key, v, "a positive integer"))
        })
        .transpose()
}

fn comma_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Applies the `BD_CACHE__*` overrides of the TOML keys.
fn apply_env(config: &mut RepoConfig, env: &BTreeMap<String, String>) -> Result<(), ApiError> {
    if let Some(v) = env_value(env, "CACHE_PATH") {
        config.cache_path = Some(PathBuf::from(v));
    }
    if let Some(v) = env_value(env, "SCOPES") {
        config.scopes = comma_list(v);
    }
    if let Some(n) = env_number(env, "CALL_TIMEOUT_S")? {
        config.call_timeout_s = Some(n);
    }
    if let Some(n) = env_number(env, "MISS_FILL_TIMEOUT_MS")? {
        config.miss_fill_timeout_ms = Some(n);
    }
    if let Some(n) = env_number(env, "MAX_ATTEMPTS")? {
        config.max_attempts = Some(n);
    }
    Ok(())
}

/// The merged configuration's values must work, wherever they came from.
fn validate(config: &RepoConfig) -> Result<(), ApiError> {
    config.validate().map_err(|e| {
        ApiError::config_invalid(SOURCE_ENV, e.to_string(), Map::new()).with_suggested_action(
            "give each timeout and max_attempts a value in its range (the detail names it)",
        )
    })?;
    if let Err(bad) = config.configured_scopes() {
        return Err(ApiError::config_invalid(
            SOURCE_ENV,
            format!("scope {:?} is invalid: {}", bad.value, bad.reason),
            details(&[("key", json!("scopes"))]),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn env_overrides_replace_the_toml_values_key_by_key() {
        let mut c = RepoConfig {
            scopes: vec!["a".into()],
            call_timeout_s: Some(9),
            ..RepoConfig::default()
        };
        let applied = apply_env(&mut c, &env(&[("BD_CACHE__SCOPES", "x, y,")]));
        assert!(applied.is_ok(), "{applied:?}");
        assert_eq!(c.scopes, ["x", "y"]);
        assert_eq!(
            c.call_timeout_s,
            Some(9),
            "an unset key keeps the TOML value"
        );
    }

    #[test]
    fn an_env_value_that_does_not_parse_is_config_invalid() {
        let mut c = RepoConfig::default();
        let e = apply_env(&mut c, &env(&[("BD_CACHE__MAX_ATTEMPTS", "lots")])).err();
        assert_eq!(
            e.as_ref().map(ApiError::code),
            Some("BD_CACHE.CONFIG_INVALID")
        );
        let mut c = RepoConfig {
            max_attempts: Some(0),
            ..RepoConfig::default()
        };
        assert!(apply_env(&mut c, &BTreeMap::new()).is_ok());
        assert!(validate(&c).is_err(), "zero attempts cannot work");
    }

    #[test]
    fn unknown_env_keys_are_reported() {
        let e = env(&[
            ("BD_CACHE__FORMAT", "text"),
            ("BD_CACHE__REPO", "r"),
            ("PATH", "p"),
        ]);
        assert_eq!(unknown_env_keys(&e), ["FORMAT"]);
    }
}
