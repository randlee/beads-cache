//! The concrete composition behind the Python handle (pure Rust, no Python types).
//!
//! `Cache<SqliteStore, FlockDrainLock, BdCli>`, opened from a repository the way
//! `docs/architecture.md` describes: `.beads/bd-cache.toml` through
//! `bd_cache_store::load_repo_config`, with explicit arguments overriding the
//! file's keys.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bd_cache::Cache;
use bd_cache_model::{CacheError, RepoConfig, ScopeId};
use bd_cache_store::{ConfigLoadError, FlockDrainLock, SqliteStore, load_repo_config};
use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::{Clock, ProjectRef, SystemClock};

/// The cache type every verb of the module runs on.
pub(crate) type Composition = Cache<SqliteStore, FlockDrainLock, BdCli>;

/// Explicit arguments that override `.beads/bd-cache.toml`.
#[derive(Debug, Default)]
pub(crate) struct Overrides {
    /// Replaces `cache_path`.
    pub(crate) cache_path: Option<PathBuf>,
    /// Replaces `scopes`.
    pub(crate) scopes: Option<Vec<String>>,
    /// Replaces `call_timeout_s`.
    pub(crate) call_timeout_s: Option<u64>,
    /// Replaces `miss_fill_timeout_ms`.
    pub(crate) miss_fill_timeout_ms: Option<u64>,
    /// Replaces `max_attempts`.
    pub(crate) max_attempts: Option<u32>,
}

/// Why a cache could not be opened.
#[derive(Debug)]
pub(crate) enum OpenError {
    /// `.beads/bd-cache.toml` or an explicit argument is invalid: a `ValueError`.
    Config(String),
    /// The store refused: a typed [`CacheError`].
    Cache(CacheError),
}

impl From<CacheError> for OpenError {
    fn from(e: CacheError) -> Self {
        Self::Cache(e)
    }
}

impl From<ConfigLoadError> for OpenError {
    fn from(e: ConfigLoadError) -> Self {
        // `Display` names the file and the fix; the cause is the source.
        let mut text = e.to_string();
        let mut cause = std::error::Error::source(&e);
        while let Some(c) = cause {
            text.push_str(": ");
            text.push_str(&c.to_string());
            cause = c.source();
        }
        Self::Config(text)
    }
}

/// The handle's parts: the cache, its clock and its file.
#[derive(Debug)]
pub(crate) struct Opened {
    pub(crate) cache: Composition,
    pub(crate) clock: SystemClock,
    pub(crate) path: PathBuf,
}

/// Opens the cache of `repo_root`, running `bd_binary` in `project_dir`.
///
/// The store is located from `.beads/metadata.json`; no `bd` process starts here.
///
/// # Errors
///
/// [`OpenError::Config`] for an invalid config file or scope; [`OpenError::Cache`]
/// for `NO_DATABASE`, `WRONG_DATABASE`, `SCHEMA_VERSION` or `STORE_FAILED`.
pub(crate) fn open(
    repo_root: &Path,
    bd_binary: &str,
    project_dir: Option<&Path>,
    overrides: Overrides,
) -> Result<Opened, OpenError> {
    let mut config: RepoConfig = load_repo_config(repo_root)?;
    if let Some(p) = overrides.cache_path {
        config.cache_path = Some(p);
    }
    if let Some(scopes) = overrides.scopes {
        if let Some(bad) = scopes.iter().find(|s| ScopeId::parse(s).is_none()) {
            return Err(OpenError::Config(format!(
                "scopes entry {bad:?} is not a phase name (letters, digits, '-', '_', '.', with or without the phase- prefix)"
            )));
        }
        config.scopes = scopes;
    }
    if overrides.call_timeout_s.is_some() {
        config.call_timeout_s = overrides.call_timeout_s;
    }
    if overrides.miss_fill_timeout_ms.is_some() {
        config.miss_fill_timeout_ms = overrides.miss_fill_timeout_ms;
    }
    if overrides.max_attempts.is_some() {
        config.max_attempts = overrides.max_attempts;
    }
    let store = SqliteStore::open(repo_root, &config)?;
    let path = store.path().to_path_buf();
    let lock = store.drain_lock();
    let clock = SystemClock;
    let shared: Arc<dyn Clock> = Arc::new(clock);
    let backend = BdCli::new_with_clock(
        bd_binary,
        Arc::new(ProjectLocks::new()),
        Arc::clone(&shared),
    );
    let dir = project_dir.unwrap_or(repo_root);
    let project = ProjectRef::local(Some(dir.to_string_lossy().into_owned()));
    let cache = Cache::new(config.cache_config(project), store, lock, backend, shared)?;
    Ok(Opened { cache, clock, path })
}
