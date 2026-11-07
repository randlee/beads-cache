//! Explicit daemon JSON adapter, separate from strict repository cache TOML.

use sc_config::ConfigError;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Transport/logging and worker shutdown settings for the foreground daemon.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// Plain logging settings from the pinned scaffold.
    #[serde(default)]
    pub logging: sc_observability::LogSettings,
    /// Runtime transport settings, embedded unchanged.
    #[serde(flatten)]
    pub daemon: sc_runtime::DaemonConfig,
    /// Existing scheduler grace; the full worker stop takes at most twice this.
    #[serde(default = "shutdown_default")]
    pub shutdown_timeout_ms: u64,
    /// The bd executable, never invoked by readiness.
    #[serde(default = "bd_default")]
    pub bd_binary: String,
}
fn shutdown_default() -> u64 {
    5_000
}
fn bd_default() -> String {
    "bd".to_owned()
}

/// Reads one explicitly requested JSON file with public typed config failures.
///
/// # Errors
/// Returns `ConfigError::Read`, `Parse` or `Deserialize` with safe diagnostics.
pub fn load(path: &Path) -> Result<AppConfig, ConfigError> {
    let bytes = std::fs::read(path).map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| ConfigError::Parse {
            path: path.to_owned(),
            line: e.line(),
            column: e.column(),
            // Do not include user values from parser diagnostics.
            message: "invalid JSON syntax or encoding".to_owned(),
        })?;
    serde_path_to_error::deserialize(value).map_err(|e| ConfigError::Deserialize {
        message: format!("invalid type at {} ({:?})", e.path(), e.inner().classify()),
        files: vec![path.to_owned()],
        variables: vec![],
    })
}

/// Resolves the same instance root as the pinned runtime without creating it.
///
/// # Errors
/// Returns the public transport failure if the per-user root is unavailable.
pub fn instance_root(
    config: &AppConfig,
    app: &sc_transport::AppName,
) -> Result<PathBuf, sc_transport::TransportError> {
    config
        .daemon
        .transport
        .instance_root
        .clone()
        .map_or_else(|| sc_transport::instance_root(app), Ok)
}
