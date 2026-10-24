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

/// Why a cache could not be opened.
#[derive(Debug)]
pub(crate) enum OpenError {
    /// The config file or an argument is invalid: a `ValueError` with `kind` and `path`.
    Config {
        /// `read`-independent kind: `parse`, `scope`, `argument` or `config`.
        kind: &'static str,
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
                kind: "parse",
                path: Some(path),
                text,
            },
            ConfigLoadError::Scope { path, .. } => Self::Config {
                kind: "scope",
                path: Some(path),
                text,
            },
            _ => Self::Config {
                kind: "config",
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

fn utf8<'a>(what: &str, p: &'a Path) -> Result<&'a str, OpenError> {
    p.to_str().ok_or_else(|| OpenError::Config {
        kind: "argument",
        path: None,
        text: format!("{what} {} is not valid UTF-8", p.display()),
    })
}

/// Opens the cache of `repo_root`, running `bd_binary` in `project_dir`.
///
/// The store is located from `.beads/metadata.json`; no `bd` process starts here.
///
/// # Errors
///
/// [`OpenError::Config`] for an invalid config file, scope or non-UTF-8 path;
/// [`OpenError::Read`] for an unreadable config file; [`OpenError::Cache`]
/// for `NO_DATABASE`, `WRONG_DATABASE`, `SCHEMA_VERSION` or `STORE_FAILED`.
pub(crate) fn open(
    repo_root: &Path,
    bd_binary: &str,
    project_dir: Option<&Path>,
    overrides: Overrides,
) -> Result<Opened, OpenError> {
    utf8("repo_root", repo_root)?;
    let dir = project_dir.unwrap_or(repo_root);
    let project = ProjectRef::local(Some(utf8("project_dir", dir)?.to_owned()));
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
#[allow(
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

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_is_refused_not_converted_lossily() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let bad = Path::new(OsStr::from_bytes(b"/tmp/\xff\xfe"));
        assert!(matches!(
            open(bad, "bd", None, Overrides::default()),
            Err(OpenError::Config {
                kind: "argument",
                ..
            })
        ));
        assert!(matches!(
            open(Path::new("/tmp"), "bd", Some(bad), Overrides::default()),
            Err(OpenError::Config {
                kind: "argument",
                ..
            })
        ));
    }
}
