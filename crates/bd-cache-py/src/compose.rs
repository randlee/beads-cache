//! The concrete composition behind the Python handle (pure Rust, no Python types).
//!
//! `Cache<SqliteStore, FlockDrainLock, BdCli>`, opened from a repository the way
//! `docs/architecture.md` describes: `.beads/bd-cache.toml` through
//! `bd_cache_store::load_repo_config`, with explicit arguments overriding the
//! file's keys. The cache itself is composed by `bd-cache-sync`'s one
//! composition root (`compose_tuned`), so every `bd` child of the process, a
//! handle's or a sync run's, shares one process-wide `ProjectLocks` (the lock
//! that keeps two `bd` processes off one embedded Dolt database).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_api::CacheFacts;
use bd_cache_model::{CacheError, RepoConfig, ScopeId};
use bd_cache_store::{
    BeadsDirError, BeadsDirOverride, ConfigLoadError, DatabaseIdentity, FlockDrainLock,
    SCHEMA_VERSION, SqliteStore, load_repo_config,
};
use bd_cache_sync::{ComposeError, ComposeTuning, ScheduleTable, SyncFiles, compose_tuned};
use sc_bd_cli::BdCli;
use sc_beads::{Clock, SystemClock};

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
    /// Any other problem the loader reports. Reserved: `ConfigLoadError` is
    /// exhaustive (lead ruling S38), so the match in `From<ConfigLoadError>`
    /// names every variant and none maps here; the `config` kind text stays.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "ConfigLoadError is exhaustive (S38): no loader error maps here; the `config` kind text is kept"
        )
    )]
    Other,
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
            Self::Config(ConfigKind::Other) => "config",
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
        }
    }
}

/// The handle's parts: the cache, the clock it shares with its backend, and its file.
pub(crate) struct Opened {
    pub(crate) cache: Composition,
    /// The cache file's schedule table, or why it cannot serve (ADR-BDC-0014).
    pub(crate) schedules: ScheduleTable,
    /// The files a sync run of this cache uses (the cache file, the beads
    /// directory `metadata.json` was read from, the repository root).
    pub(crate) files: SyncFiles,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) path: PathBuf,
    /// What `context` and `cache status` report of the file and `metadata.json`.
    pub(crate) facts: CacheFacts,
}

/// The one message for a phase name that does not parse.
pub(crate) fn phase_problem(phase: &str) -> String {
    format!(
        "{phase:?} is not a phase name (letters, digits, '-', '_', '.', optional phase- prefix)"
    )
}

/// Parses a 0.1 phase argument: `x` or the legacy-prefixed `x` names the
/// phase-a scope stored as `x` (REQ-BDC-0812 as amended).
///
/// # Errors
///
/// [`phase_problem`] text.
pub(crate) fn parse_phase(phase: &str) -> Result<ScopeId, String> {
    bd_cache_model::legacy_scope(phase).ok_or_else(|| phase_problem(phase))
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

/// Checks the directory `bd` runs in: valid UTF-8 and an existing directory;
/// returns its text. The lock key is `sc-bd-cli`'s (the canonical path, so
/// every spelling of one directory shares one lock); this only refuses a
/// directory that cannot be one.
fn check_project_dir(p: &Path) -> Result<String, OpenError> {
    let text = p
        .to_str()
        .ok_or_else(|| refuse(format!("project_dir {} is not valid UTF-8", p.display())))?;
    match std::fs::metadata(p) {
        Ok(m) if m.is_dir() => Ok(text.to_owned()),
        Ok(_) => Err(refuse(format!(
            "project_dir {} is not a directory",
            p.display()
        ))),
        Err(e) => Err(refuse(format!(
            "project_dir {} does not resolve: {e}",
            p.display()
        ))),
    }
}

/// A composition failure as the handle's open error.
fn open_error(e: ComposeError) -> OpenError {
    match e {
        ComposeError::Config(e) => OpenError::from(e),
        ComposeError::Cache(e) => OpenError::Cache(e),
        ComposeError::BeadsDir(e) => unresolvable_beads_dir(&e),
        ComposeError::RepoRootNotUtf8(root) => {
            refuse(format!("repo_root {} is not valid UTF-8", root.display()))
        }
        other => OpenError::Cache(CacheError::StoreFailed {
            detail: format!("cannot open the cache ({}): {other}", other.code()),
            source: None,
        }),
    }
}

/// Opens the cache of `repo_root`, running `bd_binary` in `project_dir`.
///
/// The store is located from `.beads/metadata.json`; no `bd` process starts here.
/// `$BEADS_DIR` is read once ([`BeadsDirOverride::from_env`]) and made absolute
/// against the process's working directory; see [`open_with`].
///
/// # Errors
///
/// As [`open_with`].
pub(crate) fn open(
    repo_root: &Path,
    bd_binary: &str,
    project_dir: Option<&Path>,
    overrides: Overrides,
) -> Result<Opened, OpenError> {
    open_with(
        repo_root,
        bd_binary,
        project_dir,
        overrides,
        BeadsDirOverride::from_env(),
    )
}

/// The `BD_CACHE.NO_DATABASE` of a `$BEADS_DIR` that cannot be made absolute.
///
/// The same code and detail as `SqliteStore::open` gives it (lead ruling S49), so
/// the binding's open error does not depend on which layer read the variable.
fn unresolvable_beads_dir(e: &BeadsDirError) -> OpenError {
    OpenError::Cache(e.to_cache_error())
}

/// [`open`] with the validated `$BEADS_DIR` given, so a test does not read or
/// change the process environment.
///
/// The one [`BeadsDirOverride`] locates the store and is set as `BEADS_DIR` in
/// every `bd` child's environment (removed there when it is `None`). `bd` runs
/// in `project_dir`, so a relative value would otherwise name one directory for
/// the store (the process's working directory) and another for `bd`
/// (PE-ARCH-001).
///
/// # Errors
///
/// [`OpenError::Config`] for an invalid config file or scope, a `repo_root` that is not
/// UTF-8 or not an existing directory (checked first, whether or not `project_dir` is
/// given), or a `project_dir` that is not UTF-8 or does not resolve to a directory;
/// [`OpenError::Read`] for an unreadable config file; [`OpenError::Cache`]
/// for `NO_DATABASE` (including a `$BEADS_DIR` that cannot be made absolute, checked
/// before the config file is read), `WRONG_DATABASE`, `SCHEMA_VERSION`, `STORE_FAILED`
/// or `STORE_IN_USE` (a second open of the same file in this process).
pub(crate) fn open_with(
    repo_root: &Path,
    bd_binary: &str,
    project_dir: Option<&Path>,
    overrides: Overrides,
    beads_dir: Result<Option<BeadsDirOverride>, BeadsDirError>,
) -> Result<Opened, OpenError> {
    check_repo_root(repo_root)?;
    let project_dir = project_dir.map(check_project_dir).transpose()?;
    let beads_dir = beads_dir.map_err(|e| unresolvable_beads_dir(&e))?;
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
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let composed = compose_tuned(
        repo_root,
        config,
        bd_binary,
        Arc::clone(&clock),
        Ok(beads_dir),
        &ComposeTuning {
            project_dir,
            call_timeout: overrides.call_timeout,
            miss_fill_timeout: overrides.miss_fill_timeout,
        },
    )
    .map_err(open_error)?;
    let identity = DatabaseIdentity::read(&composed.beads_dir)?;
    let path = composed.cache_path.clone();
    let facts = CacheFacts {
        cache_path: path.to_string_lossy().into_owned(),
        store_schema_version: i64::from(SCHEMA_VERSION),
        database: identity.dolt_database.as_str().to_owned(),
        project_id: identity.project_id.as_str().to_owned(),
        beads_dir: Some(composed.beads_dir.to_string_lossy().into_owned()),
        repo_root: Some(repo_root.to_string_lossy().into_owned()),
    };
    let files = composed.files();
    Ok(Opened {
        cache: composed.cache,
        schedules: composed.schedules,
        files,
        clock,
        path,
        facts,
    })
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    /// [`open_with`] with `$BEADS_DIR` unset, whatever the environment of the test run.
    fn open_unset(root: &Path, project_dir: Option<&Path>) -> Result<Opened, OpenError> {
        open_with(root, "bd", project_dir, Overrides::default(), Ok(None))
    }

    /// A repository whose `.beads/metadata.json` names `database`.
    fn repo_named(database: &str) -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let beads = tmp.path().join(".beads");
        std::fs::create_dir(&beads).unwrap();
        std::fs::write(
            beads.join("metadata.json"),
            format!(r#"{{"dolt_database": "{database}", "project_id": "p-1"}}"#),
        )
        .unwrap();
        tmp
    }

    /// The override, not `<repo_root>/.beads`, locates the store (PE-ARCH-001): the
    /// same value is what every `bd` child gets as `BEADS_DIR`.
    #[test]
    fn the_beads_dir_override_locates_the_store() {
        let root = repo_named("from-repo");
        let other = repo_named("from-override");
        let dir = BeadsDirOverride::new(Some(other.path().join(".beads").as_os_str())).unwrap();
        let opened = open_with(root.path(), "bd", None, Overrides::default(), Ok(dir))
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(
            opened.path.file_name().and_then(OsStr::to_str),
            Some("from-override.bd-cache.sqlite3")
        );
    }

    /// A `$BEADS_DIR` that cannot be made absolute is `NO_DATABASE`, naming the value
    /// and the I/O error, before the config file is read (an invalid one is not
    /// reported) and before any cache file is created.
    #[test]
    fn an_unresolvable_beads_dir_is_no_database_before_the_config_is_read() {
        let root = repo_named("dev");
        std::fs::write(
            root.path().join(".beads").join("bd-cache.toml"),
            "not toml [",
        )
        .unwrap();
        let unresolvable = BeadsDirError::new(
            "rel",
            std::io::Error::new(std::io::ErrorKind::NotFound, "getcwd failed"),
        );
        let r = open_with(
            root.path(),
            "bd",
            None,
            Overrides::default(),
            Err(unresolvable),
        );
        match r {
            Err(OpenError::Cache(CacheError::NoDatabase { detail, .. })) => assert!(
                detail.contains("BEADS_DIR rel")
                    && detail.contains(": getcwd failed; ")
                    && detail.ends_with("run from a readable working directory"),
                "{detail}"
            ),
            other => panic!("{:?}", other.err()),
        }
        assert!(!root.path().join(".bd-cache").exists());
    }

    #[test]
    fn the_kind_texts_are_exactly_parse_scope_argument_config_and_read() {
        let kinds = [
            OpenKind::Config(ConfigKind::Parse),
            OpenKind::Config(ConfigKind::Scope),
            OpenKind::Config(ConfigKind::Argument),
            OpenKind::Config(ConfigKind::Other),
            OpenKind::Read,
        ];
        let texts: std::collections::BTreeSet<&str> =
            kinds.into_iter().map(OpenKind::as_str).collect();
        let want: std::collections::BTreeSet<&str> =
            ["parse", "scope", "argument", "config", "read"].into();
        assert_eq!(texts, want);
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

    /// A project directory that does not resolve, or is a file, is refused
    /// with the argument kind; planted negative: an existing one is taken.
    /// (Its spellings share one lock in `sc-bd-cli`, which keys locks by the
    /// canonical path; `tests/test_threading.py` checks that end to end.)
    #[test]
    fn a_missing_project_dir_is_refused() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();
        assert!(check_project_dir(&dir).is_ok());
        for bad in [dir.join("no-such-dir-bd-cache-py"), {
            let f = dir.join("a-file");
            std::fs::write(&f, "x").unwrap();
            f
        }] {
            assert!(matches!(
                check_project_dir(&bad),
                Err(OpenError::Config {
                    kind: ConfigKind::Argument,
                    ..
                })
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_is_refused_not_converted_lossily() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let tmp = tempfile::TempDir::new().unwrap();
        let bad = tmp.path().join(OsStr::from_bytes(b"\xff\xfe"));
        assert!(matches!(
            open_unset(&bad, None),
            Err(OpenError::Config {
                kind: ConfigKind::Argument,
                ..
            })
        ));
        assert!(matches!(
            open_unset(tmp.path(), Some(&bad)),
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
                let r = open_unset(root, project_dir);
                assert!(
                    matches!(&r, Err(OpenError::Config { kind: ConfigKind::Argument, text, .. }) if text.starts_with("repo_root")),
                    "{root:?} {project_dir:?}: {:?}",
                    r.err()
                );
            }
        }
    }
}
