//! The concrete composition behind the Python handle (pure Rust, no Python types).
//!
//! `Cache<SqliteStore, FlockDrainLock, BdCli>`, opened from a repository the way
//! `docs/architecture.md` describes: `.beads/bd-cache.toml` through
//! `bd_cache_store::load_repo_config`, with explicit arguments overriding the
//! file's keys.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{CacheError, RepoConfig, ScopeId};
use bd_cache_store::{ConfigLoadError, FlockDrainLock, SqliteStore, load_repo_config};
use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::{Clock, ProjectRef, SystemClock};

/// The cache type every verb of the module runs on.
pub(crate) type Composition = Cache<SqliteStore, FlockDrainLock, BdCli>;

/// Explicit arguments that override `.beads/bd-cache.toml`, already validated.
#[derive(Debug, Default)]
pub(crate) struct Overrides {
    /// Replaces `cache_path`.
    pub(crate) cache_path: Option<PathBuf>,
    /// Replaces `scopes`.
    pub(crate) scopes: Option<Vec<ScopeId>>,
    /// Replaces `call_timeout_s`.
    pub(crate) call_timeout: Option<Duration>,
    /// Replaces `miss_fill_timeout_ms`.
    pub(crate) miss_fill_timeout: Option<Duration>,
    /// Replaces `max_attempts`.
    pub(crate) max_attempts: Option<u32>,
}

/// What kind of problem an [`OpenError::Config`] is (a `ValueError`'s `kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigKind {
    /// `.beads/bd-cache.toml` is not valid TOML, has an unknown key or a mistyped value.
    Parse,
    /// A `scopes` entry is not a phase name.
    Scope,
    /// An argument is invalid: a path that is not UTF-8, missing or not a directory.
    Argument,
    /// Any other problem the loader reports.
    Config,
}

/// The `kind` attribute of an open failure: a [`ConfigKind`], or a read failure.
///
/// `Read` is the kind of [`OpenError::Read`] only; [`OpenError::Config`] cannot hold it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenKind {
    /// A configuration problem (a `ValueError`).
    Config(ConfigKind),
    /// `.beads/bd-cache.toml` exists but cannot be read (an `OSError`).
    Read,
}

impl From<ConfigKind> for OpenKind {
    fn from(kind: ConfigKind) -> Self {
        Self::Config(kind)
    }
}

impl OpenKind {
    /// The text of the `kind` attribute.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Config(ConfigKind::Parse) => "parse",
            Self::Config(ConfigKind::Scope) => "scope",
            Self::Config(ConfigKind::Argument) => "argument",
            Self::Config(ConfigKind::Config) => "config",
            Self::Read => "read",
        }
    }
}

/// Why a cache could not be opened.
#[derive(Debug)]
pub(crate) enum OpenError {
    /// The config file or an argument is invalid: a `ValueError` with `kind` and `path`.
    Config {
        /// What is wrong.
        kind: ConfigKind,
        /// The file the problem is in, when there is one.
        path: Option<PathBuf>,
        /// The message, with the cause chain.
        text: String,
    },
    /// `.beads/bd-cache.toml` exists but cannot be read: an `OSError`.
    Read {
        /// The file.
        path: PathBuf,
        /// The message, with the cause.
        text: String,
    },
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
        match e {
            ConfigLoadError::Read { path, .. } => Self::Read { path, text },
            ConfigLoadError::Parse { path, .. } => Self::Config {
                kind: ConfigKind::Parse,
                path: Some(path),
                text,
            },
            ConfigLoadError::Scope { path, .. } => Self::Config {
                kind: ConfigKind::Scope,
                path: Some(path),
                text,
            },
            _ => Self::Config {
                kind: ConfigKind::Config,
                path: None,
                text,
            },
        }
    }
}

/// The handle's parts: the cache, the clock it shares with its backend, and its file.
pub(crate) struct Opened {
    pub(crate) cache: Composition,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) path: PathBuf,
}

/// The one message for a phase name that does not parse.
pub(crate) fn phase_problem(phase: &str) -> String {
    format!(
        "{phase:?} is not a phase name (letters, digits, '-', '_', '.', optional phase- prefix)"
    )
}

/// Parses a phase name (with or without `phase-`).
///
/// # Errors
///
/// [`phase_problem`] text.
pub(crate) fn parse_phase(phase: &str) -> Result<ScopeId, String> {
    ScopeId::parse(phase).ok_or_else(|| phase_problem(phase))
}

/// Every `bd` call of every handle in the process goes through this one lock set.
///
/// `bd` on embedded Dolt crashes when two `bd` processes use one database at
/// once, so `ProjectLocks` holds one mutex per project directory for the life of
/// each child. Two handles on one project (different cache files) must share
/// the set, or their children overlap: hence a process-wide static. This is
/// correctness state, not a cache, and it lives in this module, not in the
/// libraries (`sc-bd-cli` takes the set as an argument and owns no static).
static LOCKS: OnceLock<Arc<ProjectLocks>> = OnceLock::new();

fn project_locks() -> Arc<ProjectLocks> {
    Arc::clone(LOCKS.get_or_init(|| Arc::new(ProjectLocks::new())))
}

fn refuse(text: String) -> OpenError {
    OpenError::Config {
        kind: ConfigKind::Argument,
        path: None,
        text,
    }
}

/// Checks the repository root: valid UTF-8 and an existing directory.
///
/// Validated on its own, before `project_dir`, so a bad root is the same
/// `ValueError` (`kind` `argument`) whether or not `project_dir` is given.
fn check_repo_root(p: &Path) -> Result<(), OpenError> {
    p.to_str()
        .ok_or_else(|| refuse(format!("repo_root {} is not valid UTF-8", p.display())))?;
    match std::fs::metadata(p) {
        Ok(m) if m.is_dir() => Ok(()),
        Ok(_) => Err(refuse(format!(
            "repo_root {} is not a directory",
            p.display()
        ))),
        Err(e) => Err(refuse(format!(
            "repo_root {} does not resolve: {e}",
            p.display()
        ))),
    }
}

/// The directory `bd` runs in, resolved to its one canonical spelling.
///
/// The lock key is the directory text, so `root`, `root/.`, `root/` and a symlink to
/// `root` must all become one string, or two handles on one project would run `bd`
/// at the same time. A directory that does not resolve is refused. `dunce`
/// canonicalizes like `std::fs::canonicalize` but, on Windows, returns the plain
/// `C:\...` form where it is valid, not the `\\?\` verbatim form (R12).
fn project_dir_text(what: &str, p: &Path) -> Result<String, OpenError> {
    p.to_str()
        .ok_or_else(|| refuse(format!("{what} {} is not valid UTF-8", p.display())))?;
    let canonical = dunce::canonicalize(p)
        .map_err(|e| refuse(format!("{what} {} does not resolve: {e}", p.display())))?;
    if !canonical.is_dir() {
        return Err(refuse(format!("{what} {} is not a directory", p.display())));
    }
    canonical
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| refuse(format!("{what} {} is not valid UTF-8", canonical.display())))
}

/// Opens the cache of `repo_root`, running `bd_binary` in `project_dir`.
///
/// The store is located from `.beads/metadata.json`; no `bd` process starts here.
///
/// # Errors
///
/// [`OpenError::Config`] for an invalid config file or scope, a `repo_root` that is not
/// UTF-8 or not an existing directory (checked first, whether or not `project_dir` is
/// given), or a `project_dir` that is not UTF-8 or does not resolve to a directory;
/// [`OpenError::Read`] for an unreadable config file; [`OpenError::Cache`]
/// for `NO_DATABASE`, `WRONG_DATABASE`, `SCHEMA_VERSION` or `STORE_FAILED`.
pub(crate) fn open(
    repo_root: &Path,
    bd_binary: &str,
    project_dir: Option<&Path>,
    overrides: Overrides,
) -> Result<Opened, OpenError> {
    check_repo_root(repo_root)?;
    let (what, dir) = project_dir.map_or(("repo_root", repo_root), |d| ("project_dir", d));
    let project = ProjectRef::local(Some(project_dir_text(what, dir)?));
    let mut config: RepoConfig = load_repo_config(repo_root)?;
    if let Some(p) = overrides.cache_path {
        config.cache_path = Some(p);
    }
    if let Some(scopes) = &overrides.scopes {
        config.scopes = scopes.iter().map(|s| s.name().to_owned()).collect();
    }
    if overrides.max_attempts.is_some() {
        config.max_attempts = overrides.max_attempts;
    }
    let store = SqliteStore::open(repo_root, &config)?;
    let path = store.path().to_path_buf();
    let lock = store.drain_lock();
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let backend = BdCli::new_with_clock(bd_binary, project_locks(), Arc::clone(&clock));
    let mut cache_config = config.cache_config(project);
    if let Some(d) = overrides.call_timeout {
        cache_config.call_timeout = d;
    }
    if let Some(d) = overrides.miss_fill_timeout {
        cache_config.miss_fill_timeout = d;
    }
    let cache = Cache::new(cache_config, store, lock, backend, Arc::clone(&clock))?;
    Ok(Opened { cache, clock, path })
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a failed unwrap is a test failure"
)]
mod tests {
    use super::*;

    #[test]
    fn handles_share_one_lock_set() {
        assert!(Arc::ptr_eq(&project_locks(), &project_locks()));
    }

    #[test]
    fn phase_names_parse_once_with_one_message() {
        assert_eq!(
            parse_phase("phase-bk").map(|s| s.name().to_owned()),
            Ok("bk".to_owned())
        );
        let text = parse_phase("b k").unwrap_err();
        assert!(
            text.contains("\"b k\"") && text.contains("phase name"),
            "{text}"
        );
    }

    #[test]
    fn spellings_of_one_directory_become_one_lock_key_and_a_missing_one_is_refused() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();
        let plain = project_dir_text("project_dir", &dir).unwrap();
        let dotted = project_dir_text("project_dir", &dir.join(".")).unwrap();
        let slashed = project_dir_text("project_dir", &dir.join("")).unwrap();
        assert_eq!(plain, dotted);
        assert_eq!(plain, slashed);
        assert!(matches!(
            project_dir_text("project_dir", &dir.join("no-such-dir-bd-cache-py")),
            Err(OpenError::Config {
                kind: ConfigKind::Argument,
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_is_refused_not_converted_lossily() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let tmp = tempfile::TempDir::new().unwrap();
        let bad = tmp.path().join(OsStr::from_bytes(b"\xff\xfe"));
        assert!(matches!(
            open(&bad, "bd", None, Overrides::default()),
            Err(OpenError::Config {
                kind: ConfigKind::Argument,
                ..
            })
        ));
        assert!(matches!(
            open(tmp.path(), "bd", Some(&bad), Overrides::default()),
            Err(OpenError::Config {
                kind: ConfigKind::Argument,
                ..
            })
        ));
    }

    #[test]
    fn a_repo_root_that_is_missing_or_a_file_is_one_argument_error_with_or_without_project_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let missing = tmp.path().join("no-such-root");
        let file = tmp.path().join("a-file");
        std::fs::write(&file, "x").unwrap();
        for root in [&missing, &file] {
            for project_dir in [None, Some(tmp.path())] {
                let r = open(root, "bd", project_dir, Overrides::default());
                assert!(
                    matches!(&r, Err(OpenError::Config { kind: ConfigKind::Argument, text, .. }) if text.starts_with("repo_root")),
                    "{root:?} {project_dir:?}: {:?}",
                    r.err()
                );
            }
        }
    }
}
