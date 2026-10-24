//! The concrete composition: the SQLite store, the flock drain lock and the
//! `bd` CLI behind one [`Cache`] (ADR-BDC-0004, ADR-SYNC-0001).

use std::ffi::OsStr;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use bd_cache::Cache;
use bd_cache_model::{CacheError, TickConfig};
use bd_cache_store::{
    BEADS_DIR_ENV, ConfigLoadError, FlockDrainLock, SqliteStore, load_repo_config,
};
use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::{Clock, ProjectRef};

use crate::phases::rooted_phases_dir;

/// The cache over the real store, drain lock and `bd`.
pub type RealCache = Cache<SqliteStore, FlockDrainLock, BdCli>;

/// A repository's cache with its `[tick]` table.
#[derive(Debug)]
pub struct Composed {
    /// The cache of the repository's file.
    pub cache: RealCache,
    /// The `[tick]` table of `.beads/bd-cache.toml` (empty when the file has
    /// none), with `phases_dir` resolved against the repository root: a
    /// relative directory, or the default `.atm-bd`, names the same place
    /// whatever the working directory is.
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

/// This crate's stable code of a configuration that cannot be loaded, one per
/// way it fails (lead ruling S34): `sync_config_unreadable`,
/// `sync_config_invalid_toml` (bad TOML, an unknown key, a wrong type) and
/// `sync_config_bad_scope`.
///
/// Mapped from the [`ConfigLoadError`] variant, not from the store's own
/// `BD_CACHE.CONFIG_*` code, so a change of the store's codes does not change
/// this crate's. The enum is `#[non_exhaustive]`: a way of failing the store
/// adds later reads as `sync_config_invalid_toml`, the file not being a valid
/// configuration, until it is given a code here.
#[expect(
    clippy::match_same_arms,
    reason = "Parse is named so its code is visible; the wildcard is the non_exhaustive fallback"
)]
fn config_code(e: &ConfigLoadError) -> &'static str {
    match e {
        ConfigLoadError::Read { .. } => "sync_config_unreadable",
        ConfigLoadError::Scope { .. } => "sync_config_bad_scope",
        ConfigLoadError::Parse { .. } => "sync_config_invalid_toml",
        _ => "sync_config_invalid_toml",
    }
}

impl ComposeError {
    /// A stable code: for the configuration, this crate's `sync_config_*`
    /// code of the way it failed (mapped from the [`ConfigLoadError`]
    /// variant, lead ruling S34), else the `BD_CACHE.*` code of the cache
    /// error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config(e) => config_code(e),
            Self::Cache(e) => e.code(),
        }
    }

    /// What to do about it: for the configuration, the loader's own fix
    /// ([`ConfigLoadError::fix`], the text its message states); for the
    /// cache, the cache error's own fix.
    #[must_use]
    pub fn fix(&self) -> &'static str {
        match self {
            Self::Config(e) => e.fix(),
            Self::Cache(e) => e.fix(),
        }
    }
}

impl fmt::Display for ComposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(_) => f.write_str("cannot load .beads/bd-cache.toml"),
            Self::Cache(_) => f.write_str("cannot open the cache of the repository"),
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
/// The beads directory honours `$BEADS_DIR` from the environment; a caller
/// that has its own value, or a test that must not depend on the machine,
/// uses [`compose_with`] (lead ruling S36).
///
/// `clock` is shared by the cache and the backend, so every deadline is
/// measured on one clock (ADR-CORE-0001); the job reads it from the cache.
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
    compose_with(
        repo_root,
        backend,
        clock,
        std::env::var_os(BEADS_DIR_ENV).as_deref(),
    )
}

/// As [`compose`], over a `BdCli` the caller built (a test's `BdCli` over a
/// recording or fake invoker, or one with a seeded version probe), and with
/// an explicit `$BEADS_DIR` value (`None` = unset): the environment is not
/// read (lead ruling S36, built on [`SqliteStore::open_with`]).
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
    beads_dir_env: Option<&OsStr>,
) -> Result<Composed, ComposeError> {
    let repo = load_repo_config(repo_root).map_err(ComposeError::Config)?;
    let store =
        SqliteStore::open_with(repo_root, &repo, beads_dir_env).map_err(ComposeError::Cache)?;
    let name = store
        .path()
        .file_name()
        .map_or_else(|| "cache".to_owned(), |n| n.to_string_lossy().into_owned());
    let lock = store.drain_lock();
    let project = ProjectRef::local(Some(repo_root.to_string_lossy().into_owned()));
    let cache = Cache::new(repo.cache_config(project), store, lock, backend, clock)
        .map_err(ComposeError::Cache)?;
    let mut tick = repo.tick.unwrap_or_default();
    tick.phases_dir = Some(rooted_phases_dir(tick.phases_dir.as_deref(), repo_root));
    Ok(Composed { cache, tick, name })
}
