//! The concrete composition: the SQLite store, the flock drain lock and the
//! `bd` CLI behind one [`Cache`] (ADR-BDC-0004, ADR-SYNC-0001).

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

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
    /// ruling S48). The store's own error, whole (RBP-F050): its message and
    /// its I/O source are this error's.
    BeadsDir(BeadsDirError),
    /// The repository root is not valid UTF-8, so it cannot be the text of the
    /// cache's `ProjectRef` without being changed (S54); nothing was opened.
    RepoRootNotUtf8(PathBuf),
}

/// The stable code of [`ComposeError::BeadsDir`] (lead ruling S48).
const BEADS_DIR_CODE: &str = "sync_beads_dir_unresolvable";

/// The fix of [`ComposeError::BeadsDir`] (lead ruling S48).
const BEADS_DIR_FIX: &str = "set an absolute BEADS_DIR, or run from a readable working directory";

/// The stable code of [`ComposeError::RepoRootNotUtf8`] (S54).
const REPO_ROOT_CODE: &str = "sync_repo_root_not_utf8";

/// The fix of [`ComposeError::RepoRootNotUtf8`] (S54).
const REPO_ROOT_FIX: &str = "run the cache from a repository whose path is valid UTF-8";

impl From<BeadsDirError> for ComposeError {
    fn from(e: BeadsDirError) -> Self {
        Self::BeadsDir(e)
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
    /// cache error; for `$BEADS_DIR`, `sync_beads_dir_unresolvable` (S48);
    /// for a repository root that is not UTF-8, `sync_repo_root_not_utf8`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config(e) => config_code(e),
            Self::Cache(e) => e.code(),
            Self::BeadsDir(_) => BEADS_DIR_CODE,
            Self::RepoRootNotUtf8(_) => REPO_ROOT_CODE,
        }
    }

    /// What to do about it: for the configuration, the loader's own fix
    /// ([`ConfigLoadError::fix`], the text its message states); for the
    /// cache, the cache error's own fix; for `$BEADS_DIR`, an absolute value
    /// or a readable working directory (S48); for the repository root, a
    /// UTF-8 path.
    #[must_use]
    pub fn fix(&self) -> &'static str {
        match self {
            Self::Config(e) => e.fix(),
            Self::Cache(e) => e.fix(),
            Self::BeadsDir(_) => BEADS_DIR_FIX,
            Self::RepoRootNotUtf8(_) => REPO_ROOT_FIX,
        }
    }
}

impl fmt::Display for ComposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(_) => f.write_str("cannot load .beads/bd-cache.toml"),
            Self::Cache(_) => f.write_str("cannot open the cache of the repository"),
            // The store's message, once: the I/O error is the source below.
            Self::BeadsDir(e) => fmt::Display::fmt(e, f),
            Self::RepoRootNotUtf8(root) => write!(
                f,
                "the repository root {} is not valid UTF-8",
                root.display()
            ),
        }
    }
}

impl std::error::Error for ComposeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(e) => Some(e),
            Self::Cache(e) => Some(e),
            // Delegated: the wrapped error's own source (the I/O error), so a
            // reporter that walks the chain does not print its message twice.
            Self::BeadsDir(e) => std::error::Error::source(e),
            Self::RepoRootNotUtf8(_) => None,
        }
    }
}

/// Every `bd` call of every [`compose`] in the process goes through this one
/// lock set (S54, PE-RSH-003).
///
/// `bd` on embedded Dolt crashes when two `bd` processes use one database at
/// once, so [`ProjectLocks`] holds one mutex per project directory for the
/// life of each child. Two compositions of one project (a recomposed cache,
/// or a host that composes twice) must share the set, or their children
/// overlap: hence one process-wide set, as `bd-cache-py` keeps its own. It is
/// correctness state, not a cache. A host with its own set passes it to
/// [`compose_with`] instead; `sc-bd-cli` normalizes the key, so every
/// spelling of a directory shares one lock.
static LOCKS: OnceLock<Arc<ProjectLocks>> = OnceLock::new();

/// The process-wide lock set of [`compose`].
fn project_locks() -> Arc<ProjectLocks> {
    Arc::clone(LOCKS.get_or_init(|| Arc::new(ProjectLocks::new())))
}

/// Builds the cache of the repository at `repo_root`: loads
/// `.beads/bd-cache.toml` (the one loader, `bd-cache-store`), opens the cache
/// file it names, and wires a `BdCli` running `bd_binary` in that repository.
///
/// The beads directory honours `$BEADS_DIR` from the environment; a caller
/// that has its own value, or a test that must not depend on the machine,
/// uses [`compose_with`] (lead ruling S36).
///
/// Every `BdCli` it builds shares one process-wide [`ProjectLocks`] (S54): two
/// compositions of one project, in any thread, never run `bd` at once. A
/// host that keeps its own set uses [`compose_with`].
///
/// `clock` is shared by the cache and the backend, so every deadline is
/// measured on one clock (ADR-CORE-0001); the job reads it from the cache.
///
/// # Errors
///
/// [`ComposeError`] for a repository root that is not UTF-8
/// ([`ComposeError::RepoRootNotUtf8`]), a `$BEADS_DIR` that cannot be made
/// absolute ([`ComposeError::BeadsDir`], before the store opens), an invalid
/// configuration, a missing or unreadable `.beads/metadata.json`, a cache
/// file of another project or schema, or a store that cannot be opened.
pub fn compose(
    repo_root: &Path,
    bd_binary: &str,
    clock: Arc<dyn Clock>,
) -> Result<Composed, ComposeError> {
    compose_in(
        repo_root,
        bd_binary,
        clock,
        std::env::var_os(BEADS_DIR_ENV).as_deref(),
    )
}

/// [`compose`] with the `$BEADS_DIR` value given, over the process-wide lock
/// set, so a test exercises the very lock set `compose` uses without reading
/// the environment.
fn compose_in(
    repo_root: &Path,
    bd_binary: &str,
    clock: Arc<dyn Clock>,
    beads_dir_env: Option<&OsStr>,
) -> Result<Composed, ComposeError> {
    compose_with(
        repo_root,
        |locks| BdCli::new(bd_binary, locks),
        project_locks(),
        clock,
        beads_dir_env,
    )
}

/// As [`compose`], over a `BdCli` the caller builds, a lock set the caller
/// owns, and an explicit `$BEADS_DIR` value (`None` = unset): the
/// environment is not read (lead ruling S36, built on
/// [`SqliteStore::open_with`]).
///
/// `backend` is given `locks` and returns the `BdCli` (`BdCli::new(bin,
/// locks)`, or a test's `BdCli` over a recording or fake invoker, which may
/// ignore them). A host that composes more than once, or shares `bd` with
/// other code, passes the same `Arc<ProjectLocks>` every time so their `bd`
/// children never overlap on one project (S54).
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
    backend: impl FnOnce(Arc<ProjectLocks>) -> BdCli,
    locks: Arc<ProjectLocks>,
    clock: Arc<dyn Clock>,
    beads_dir_env: Option<&OsStr>,
) -> Result<Composed, ComposeError> {
    compose_with_override(
        repo_root,
        backend,
        locks,
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
    backend: impl FnOnce(Arc<ProjectLocks>) -> BdCli,
    locks: Arc<ProjectLocks>,
    clock: Arc<dyn Clock>,
    beads_dir: Result<Option<BeadsDirOverride>, BeadsDirError>,
) -> Result<Composed, ComposeError> {
    // The project's text is the root as it is, never a lossy copy (S54).
    let root_text = repo_root
        .to_str()
        .ok_or_else(|| ComposeError::RepoRootNotUtf8(repo_root.to_path_buf()))?;
    let beads_dir = beads_dir?;
    let repo = load_repo_config(repo_root).map_err(ComposeError::Config)?;
    let store = SqliteStore::open_with(repo_root, &repo, beads_dir.as_ref())
        .map_err(ComposeError::Cache)?;
    let name = store
        .path()
        .file_name()
        .map_or_else(|| "cache".to_owned(), |n| n.to_string_lossy().into_owned());
    let lock = store.drain_lock();
    let project = ProjectRef::local(Some(root_text.to_owned()));
    let backend = backend(locks)
        .with_clock(Arc::clone(&clock))
        .with_child_var(
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
    use std::io;
    use std::time::Duration;

    use super::*;
    use sc_beads::{CacheBackend as _, Deadline, SystemClock};

    /// A repository root with a valid `metadata.json` and no configuration.
    fn repo() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let beads = root.path().join(".beads");
        std::fs::create_dir_all(&beads).unwrap();
        std::fs::write(
            beads.join("metadata.json"),
            r#"{"dolt_database": "atm-dev", "project_id": "p-1"}"#,
        )
        .unwrap();
        root
    }

    /// A `$BEADS_DIR` that could not be made absolute fails the composition
    /// with its own code, fix and the I/O error as source, before the store
    /// opens: no cache file is created, though the repository has a valid
    /// `metadata.json` (lead ruling S48, QA10-001, RSH-038, RBP-F048). The
    /// store's error is kept whole and its message is stated once (RBP-F050).
    #[test]
    fn an_unresolvable_beads_dir_fails_the_composition_before_the_store_opens() {
        let root = repo();
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let unresolvable = BeadsDirError::new(
            "rel",
            io::Error::new(io::ErrorKind::NotFound, "getcwd failed"),
        );
        let expected = unresolvable.to_string();

        let err = compose_with_override(
            root.path(),
            |locks| BdCli::new("bd", locks),
            Arc::new(ProjectLocks::new()),
            clock,
            Err(unresolvable),
        )
        .unwrap_err();

        assert!(
            matches!(&err, ComposeError::BeadsDir(e)
                if e.value() == Path::new("rel")),
            "{err:?}"
        );
        assert_eq!(err.code(), "sync_beads_dir_unresolvable");
        assert_eq!(
            err.fix(),
            "set an absolute BEADS_DIR, or run from a readable working directory"
        );
        assert_eq!(err.to_string(), expected, "the store's message, delegated");
        assert!(err.to_string().contains("BEADS_DIR rel"), "{err}");
        assert!(!err.to_string().contains("getcwd"), "{err}");
        let source = err.source().unwrap();
        assert_eq!(source.to_string(), "getcwd failed");
        assert!(
            source.downcast_ref::<io::Error>().is_some(),
            "the source is the I/O error, not the store's error again"
        );
        assert!(
            !root.path().join(".bd-cache").exists(),
            "the store was not opened"
        );
    }

    /// A repository root that is not UTF-8 is refused with its own code before
    /// anything is opened: the project text is never a lossy copy (S54).
    #[cfg(unix)]
    #[test]
    fn a_repository_root_that_is_not_utf8_is_refused() {
        use std::os::unix::ffi::OsStrExt as _;

        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join(OsStr::from_bytes(b"repo-\xff"));
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);

        let err = compose_with(
            &root,
            |locks| BdCli::new("bd", locks),
            Arc::new(ProjectLocks::new()),
            clock,
            None,
        )
        .unwrap_err();

        assert!(
            matches!(&err, ComposeError::RepoRootNotUtf8(p) if p == &root),
            "{err:?}"
        );
        assert_eq!(err.code(), "sync_repo_root_not_utf8");
        assert!(err.to_string().contains("not valid UTF-8"), "{err}");
        assert!(err.source().is_none());
    }

    /// The process-wide set is one set: every call hands out the same `Arc`.
    #[test]
    fn compose_hands_out_one_lock_set() {
        assert!(Arc::ptr_eq(&project_locks(), &project_locks()));
    }

    /// Two compositions of one project through `compose` share one lock: their
    /// `bd` children never overlap (S54, PE-RSH-003).
    ///
    /// The `bd` is a script that holds a directory for a while; a second child
    /// that finds it held while it runs marks an overlap.
    #[cfg(unix)]
    #[test]
    fn two_compositions_of_one_project_never_run_bd_at_once() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = repo();
        let work = tempfile::tempdir().unwrap();
        let held = work.path().join("held");
        let overlap = work.path().join("overlap");
        let bd = work.path().join("bd");
        std::fs::write(
            &bd,
            format!(
                "#!/bin/sh\nif mkdir '{held}' 2>/dev/null; then sleep 0.2; rmdir '{held}'; \
                 else touch '{overlap}'; fi\necho 'bd version 1.3.0'\n",
                held = held.display(),
                overlap = overlap.display(),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&bd, std::fs::Permissions::from_mode(0o755)).unwrap();
        let bd = bd.to_str().unwrap().to_owned();
        let project = ProjectRef::local(Some(root.path().to_str().unwrap().to_owned()));

        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let a = compose_in(root.path(), &bd, Arc::clone(&clock), None).unwrap();
        // A second cache file of the same project: its own store, the same `bd`.
        std::fs::write(
            root.path().join(".beads/bd-cache.toml"),
            "cache_path = \".bd-cache/second.sqlite3\"\n",
        )
        .unwrap();
        let b = compose_in(root.path(), &bd, Arc::clone(&clock), None).unwrap();
        assert_ne!(a.name, b.name, "two cache files");
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let run = |composed: Composed| {
            let project = project.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..3 {
                    let end = Deadline::after(&SystemClock, Duration::from_secs(20));
                    // The call fails at `bd schema` (the script prints no JSON);
                    // only whether two children overlapped matters.
                    let _ = bd_cache::testing::backend(&composed.cache).fingerprint(&project, end);
                }
            })
        };
        let (ta, tb) = (run(a), run(b));
        ta.join().unwrap();
        tb.join().unwrap();

        assert!(
            !overlap.exists(),
            "two bd children of one project ran at once"
        );
    }
}
