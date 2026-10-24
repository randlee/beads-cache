//! The concrete composition: the SQLite store, the flock drain lock and the
//! `bd` CLI behind one [`Cache`] (ADR-BDC-0004, ADR-SYNC-0001).

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bd_cache::Cache;
use bd_cache_model::{CacheError, TickConfig};
use bd_cache_store::{
    BEADS_DIR_ENV, BeadsDirError, BeadsDirOverride, ConfigLoadError, FlockDrainLock, SqliteStore,
    load_repo_config,
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
    /// The `$BEADS_DIR` value is relative and cannot be made absolute (the
    /// working directory cannot be read); the store was not opened (lead
    /// ruling S48).
    BeadsDir {
        /// The value as given.
        value: PathBuf,
        /// Why it cannot be made absolute.
        source: io::Error,
    },
}

/// The stable code of [`ComposeError::BeadsDir`] (lead ruling S48).
const BEADS_DIR_CODE: &str = "sync_beads_dir_unresolvable";

/// The fix of [`ComposeError::BeadsDir`] (lead ruling S48).
const BEADS_DIR_FIX: &str = "set an absolute BEADS_DIR, or run from a readable working directory";

impl From<BeadsDirError> for ComposeError {
    fn from(e: BeadsDirError) -> Self {
        let (value, source) = e.into_parts();
        Self::BeadsDir { value, source }
    }
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
    /// variant, lead ruling S34); for the cache, the `BD_CACHE.*` code of the
    /// cache error; for `$BEADS_DIR`, `sync_beads_dir_unresolvable` (S48).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config(e) => config_code(e),
            Self::Cache(e) => e.code(),
            Self::BeadsDir { .. } => BEADS_DIR_CODE,
        }
    }

    /// What to do about it: for the configuration, the loader's own fix
    /// ([`ConfigLoadError::fix`], the text its message states); for the
    /// cache, the cache error's own fix; for `$BEADS_DIR`, an absolute value
    /// or a readable working directory (S48).
    #[must_use]
    pub fn fix(&self) -> &'static str {
        match self {
            Self::Config(e) => e.fix(),
            Self::Cache(e) => e.fix(),
            Self::BeadsDir { .. } => BEADS_DIR_FIX,
        }
    }
}

impl fmt::Display for ComposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(_) => f.write_str("cannot load .beads/bd-cache.toml"),
            Self::Cache(_) => f.write_str("cannot open the cache of the repository"),
            Self::BeadsDir { value, .. } => write!(
                f,
                "cannot make BEADS_DIR {} absolute against the working directory",
                value.display()
            ),
        }
    }
}

impl std::error::Error for ComposeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(e) => Some(e),
            Self::Cache(e) => Some(e),
            Self::BeadsDir { source, .. } => Some(source),
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
/// [`ComposeError`] for a `$BEADS_DIR` that cannot be made absolute
/// ([`ComposeError::BeadsDir`], before the store opens), an invalid
/// configuration, a missing or unreadable `.beads/metadata.json`, a cache
/// file of another project or schema, or a store that cannot be opened.
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
/// `beads_dir_env` is validated once, into one [`BeadsDirOverride`], before
/// either uses it (lead rulings S44 and S49, RSH-035, QA9-003, RBP-F047): an
/// empty value is `None`, and a relative one is made absolute against the
/// process's working directory (per platform as [`BeadsDirOverride`]
/// states). The store and the `bd` child, which runs in `repo_root`,
/// therefore name one directory. A relative value that cannot be made
/// absolute fails the composition (fail closed, lead ruling S48): there is
/// no fallback to the relative value.
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
    compose_with_override(
        repo_root,
        backend,
        clock,
        BeadsDirOverride::new(beads_dir_env),
    )
}

/// [`compose_with`] from the result of validating `$BEADS_DIR`, so a test can
/// give it a value that could not be made absolute (lead ruling S48): that
/// error fails the composition before the configuration is read or the store
/// opened.
fn compose_with_override(
    repo_root: &Path,
    backend: BdCli,
    clock: Arc<dyn Clock>,
    beads_dir: Result<Option<BeadsDirOverride>, BeadsDirError>,
) -> Result<Composed, ComposeError> {
    let beads_dir = beads_dir?;
    let repo = load_repo_config(repo_root).map_err(ComposeError::Config)?;
    let store = SqliteStore::open_with(repo_root, &repo, beads_dir.as_ref())
        .map_err(ComposeError::Cache)?;
    let name = store
        .path()
        .file_name()
        .map_or_else(|| "cache".to_owned(), |n| n.to_string_lossy().into_owned());
    let lock = store.drain_lock();
    let project = ProjectRef::local(Some(repo_root.to_string_lossy().into_owned()));
    let backend = backend.with_clock(Arc::clone(&clock)).with_child_var(
        OsStr::new(BEADS_DIR_ENV),
        beads_dir.as_ref().map(BeadsDirOverride::as_os_str),
    );
    let cache = Cache::new(repo.cache_config(project), store, lock, backend, clock)
        .map_err(ComposeError::Cache)?;
    let mut tick = repo.tick.unwrap_or_default();
    tick.phases_dir = Some(rooted_phases_dir(tick.phases_dir.as_deref(), repo_root));
    Ok(Composed { cache, tick, name })
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;
    use sc_beads::SystemClock;

    /// A `$BEADS_DIR` that could not be made absolute fails the composition
    /// with its own code, fix and the I/O error as source, before the store
    /// opens: no cache file is created, though the repository has a valid
    /// `metadata.json` (lead ruling S48, QA10-001, RSH-038, RBP-F048).
    #[test]
    fn an_unresolvable_beads_dir_fails_the_composition_before_the_store_opens() {
        let root = tempfile::tempdir().unwrap();
        let beads = root.path().join(".beads");
        std::fs::create_dir_all(&beads).unwrap();
        std::fs::write(
            beads.join("metadata.json"),
            r#"{"dolt_database": "atm-dev", "project_id": "p-1"}"#,
        )
        .unwrap();
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let backend = BdCli::new("bd", Arc::new(ProjectLocks::new()));
        let unresolvable = BeadsDirError::new(
            "rel",
            io::Error::new(io::ErrorKind::NotFound, "getcwd failed"),
        );

        let err =
            compose_with_override(root.path(), backend, clock, Err(unresolvable)).unwrap_err();

        assert!(
            matches!(&err, ComposeError::BeadsDir { value, source }
                if value == Path::new("rel") && source.kind() == io::ErrorKind::NotFound),
            "{err:?}"
        );
        assert_eq!(err.code(), "sync_beads_dir_unresolvable");
        assert_eq!(
            err.fix(),
            "set an absolute BEADS_DIR, or run from a readable working directory"
        );
        assert!(err.to_string().contains("BEADS_DIR rel"), "{err}");
        assert!(!err.to_string().contains("getcwd"), "{err}");
        assert_eq!(err.source().unwrap().to_string(), "getcwd failed");
        assert!(
            !root.path().join(".bd-cache").exists(),
            "the store was not opened"
        );
    }
}
