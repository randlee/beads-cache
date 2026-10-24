//! The concrete composition: the SQLite store, the flock drain lock and the
//! `bd` CLI behind one [`Cache`] (ADR-BDC-0004, ADR-SYNC-0001).

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use bd_cache::Cache;
use bd_cache_model::{CacheError, TickConfig};
use bd_cache_store::{
    CacheLocation, ConfigLoadError, FlockDrainLock, SqliteStore, load_repo_config,
};
use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::{Clock, ProjectRef};

/// The cache over the real store, drain lock and `bd`.
pub type RealCache = Cache<SqliteStore, FlockDrainLock, BdCli>;

/// A repository's cache with its `[tick]` table.
#[derive(Debug)]
pub struct Composed {
    /// The cache of the repository's file.
    pub cache: RealCache,
    /// The `[tick]` table of `.beads/bd-cache.toml` (empty when the file has none).
    pub tick: TickConfig,
    /// The cache file's name, for naming the schedule.
    pub name: String,
}

/// The composition could not be built.
#[derive(Debug)]
#[non_exhaustive]
pub enum ComposeError {
    /// `.beads/bd-cache.toml` could not be loaded.
    Config(ConfigLoadError),
    /// The store could not be opened, or a scope could not be registered.
    Cache(CacheError),
}

impl fmt::Display for ComposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(e) => write!(f, "{e}"),
            Self::Cache(e) => write!(f, "{}: {}", e.code(), e.detail()),
        }
    }
}

impl std::error::Error for ComposeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(e) => Some(e),
            Self::Cache(e) => Some(e),
        }
    }
}

/// Builds the cache of the repository at `repo_root`: loads
/// `.beads/bd-cache.toml` (the one loader, `bd-cache-store`), opens the cache
/// file it names, and wires a `BdCli` running `bd_binary` in that repository.
///
/// `clock` is shared by the cache and the backend, so every deadline is
/// measured on one clock (ADR-CORE-0001).
///
/// # Errors
///
/// [`ComposeError`] for an invalid configuration, a missing or unreadable
/// `.beads/metadata.json`, a cache file of another project or schema, or a
/// store that cannot be opened.
pub fn compose(
    repo_root: &Path,
    bd_binary: &str,
    clock: Arc<dyn Clock>,
) -> Result<Composed, ComposeError> {
    let backend =
        BdCli::new_with_clock(bd_binary, Arc::new(ProjectLocks::new()), Arc::clone(&clock));
    compose_with(repo_root, backend, clock)
}

/// As [`compose`], over a `BdCli` the caller built (a test's `BdCli` over a
/// recording or fake invoker, or one with a seeded version probe).
///
/// `backend` must have been built with the same `clock`.
///
/// # Errors
///
/// As [`compose`].
pub fn compose_with(
    repo_root: &Path,
    backend: BdCli,
    clock: Arc<dyn Clock>,
) -> Result<Composed, ComposeError> {
    let repo = load_repo_config(repo_root).map_err(ComposeError::Config)?;
    let location = CacheLocation::resolve(repo_root, &repo).map_err(ComposeError::Cache)?;
    let store = SqliteStore::open_at(&location.cache_path, &location.identity)
        .map_err(ComposeError::Cache)?;
    let lock = store.drain_lock();
    let project = ProjectRef::local(Some(repo_root.to_string_lossy().into_owned()));
    let cache = Cache::new(repo.cache_config(project), store, lock, backend, clock)
        .map_err(ComposeError::Cache)?;
    let name = location
        .cache_path
        .file_name()
        .map_or_else(|| "cache".to_owned(), |n| n.to_string_lossy().into_owned());
    Ok(Composed {
        cache,
        tick: repo.tick.unwrap_or_default(),
        name,
    })
}
