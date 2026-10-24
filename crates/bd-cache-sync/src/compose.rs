//! The concrete composition: the SQLite store, the flock drain lock and the
//! `bd` CLI behind one [`Cache`] (ADR-BDC-0004, ADR-SYNC-0001).

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
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
/// `store_config_*` code (lead ruling S37), so a change of the store's codes
/// does not change this crate's. The match is exhaustive, with no wildcard
/// arm: `ConfigLoadError` is not `#[non_exhaustive]` (lead ruling S38), so a
/// way of failing the store adds later does not compile here until it is
/// given a code.
fn config_code(e: &ConfigLoadError) -> &'static str {
    match e {
        ConfigLoadError::Read { .. } => "sync_config_unreadable",
        ConfigLoadError::Parse { .. } => "sync_config_invalid_toml",
        ConfigLoadError::Scope { .. } => "sync_config_bad_scope",
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
/// The backend is rebound to `clock` ([`BdCli::with_clock`]), so the cache,
/// the store and the backend measure every deadline on one clock whatever
/// clock `backend` was built with (lead ruling S40, RBP-F046). Every `bd` it
/// spawns gets the same `$BEADS_DIR` the cache resolved from: set to that
/// value, or removed when there is none ([`BdCli::with_child_var`]; S40,
/// RSH-033). A backend over a test double spawns nothing and ignores it.
///
/// `beads_dir_env` is normalized once, before either uses it (lead ruling
/// S44, RSH-035, QA9-003): an empty value is `None`, and a relative one is
/// made absolute against the process's working directory. The store and the
/// `bd` child, which runs in `repo_root`, therefore name one directory.
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
    let beads_dir = normalized_beads_dir(beads_dir_env);
    let beads_dir_env = beads_dir.as_deref().map(Path::as_os_str);
    let repo = load_repo_config(repo_root).map_err(ComposeError::Config)?;
    let store =
        SqliteStore::open_with(repo_root, &repo, beads_dir_env).map_err(ComposeError::Cache)?;
    let name = store
        .path()
        .file_name()
        .map_or_else(|| "cache".to_owned(), |n| n.to_string_lossy().into_owned());
    let lock = store.drain_lock();
    let project = ProjectRef::local(Some(repo_root.to_string_lossy().into_owned()));
    let backend = backend
        .with_clock(Arc::clone(&clock))
        .with_child_var(OsStr::new(BEADS_DIR_ENV), beads_dir_env);
    let cache = Cache::new(repo.cache_config(project), store, lock, backend, clock)
        .map_err(ComposeError::Cache)?;
    let mut tick = repo.tick.unwrap_or_default();
    tick.phases_dir = Some(rooted_phases_dir(tick.phases_dir.as_deref(), repo_root));
    Ok(Composed { cache, tick, name })
}

/// The one `$BEADS_DIR` value the store and every `bd` child are given (lead
/// ruling S44): `None` for an unset or empty value, else the value made
/// absolute against the process's working directory ([`std::path::absolute`],
/// which neither touches the file system nor resolves `..` or links).
///
/// A relative value whose working directory cannot be read is kept as it is:
/// the store then cannot read `metadata.json` there and the composition fails
/// (`BD_CACHE.NO_DATABASE`) before any `bd` is spawned.
fn normalized_beads_dir(value: Option<&OsStr>) -> Option<PathBuf> {
    let dir = Path::new(value.filter(|v| !v.is_empty())?);
    Some(std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_or_unset_beads_dir_is_none() {
        assert_eq!(normalized_beads_dir(None), None);
        assert_eq!(normalized_beads_dir(Some(OsStr::new(""))), None);
    }

    #[test]
    fn a_relative_beads_dir_is_made_absolute_against_the_working_directory() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(
            normalized_beads_dir(Some(OsStr::new("rel"))),
            Some(cwd.join("rel"))
        );
        let abs = std::env::temp_dir();
        assert_eq!(normalized_beads_dir(Some(abs.as_os_str())), Some(abs));
    }
}
