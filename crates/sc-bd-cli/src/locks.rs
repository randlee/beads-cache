//! [`ProjectLocks`]: one mutex per project working directory.
//!
//! bd 0.55 uses embedded Dolt, which crashes (SIGSEGV) when two bd processes access
//! the same database simultaneously, so every `--json` invocation holds the lock of
//! its working directory for the lifetime of the child process. Entries are never
//! removed: the map grows by one entry per distinct working directory.
//!
//! The key of a lock is the directory's canonical path, resolved when the lock is
//! taken (PE-RBP-F010): `/p/link` and `/p/sub/..` share the lock of the directory they
//! name. A directory that does not exist keys on its lexical [`WorkingDir`] form.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

/// A project working directory, normalised so that spellings of one directory share a
/// lock: trailing path separators are dropped (`/p/` is `/p`), and a root keeps its
/// separator (`/`, `C:\`).
///
/// Built by [`run::resolve_project_dir`](crate::run::resolve_project_dir) and taken by
/// [`ProjectLocks::guard_dir`] and [`bounded::run_bounded`](crate::bounded::run_bounded),
/// so a lock key is never a raw string. Building one does not touch the filesystem;
/// the symlink, `..` and case resolution happen when a lock is taken
/// ([`ProjectLocks::try_guard_dir`]), since they need the directory to exist.
///
/// # Examples
///
/// ```
/// use sc_bd_cli::locks::WorkingDir;
/// assert_eq!(WorkingDir::new("/p/").as_str(), "/p");
/// assert_eq!(WorkingDir::new("/").as_str(), "/");
/// assert_eq!(WorkingDir::new("/p"), WorkingDir::new("/p//"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkingDir(String);

impl WorkingDir {
    /// Normalises `dir`.
    #[must_use]
    pub fn new(dir: &str) -> Self {
        let separators: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
        let trimmed = dir.trim_end_matches(separators);
        let normalised = if trimmed.is_empty() {
            // Only separators (a root) or nothing at all.
            dir.chars().take(1).collect()
        } else if cfg!(windows) && trimmed.ends_with(':') {
            format!("{trimmed}\\")
        } else {
            trimmed.to_owned()
        };
        Self(normalised)
    }

    /// The normalised directory.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WorkingDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The canonical path of a working directory is not UTF-8, so it cannot be a lock key.
///
/// A [`WorkingDir`] is UTF-8, but a symlink in it can name a directory whose path is
/// not; keying such a directory on its lexical form would let two spellings of it
/// take two locks, so a lock for it is refused (PE-RBP-F010).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonUtf8WorkingDir {
    /// The canonical path that is not UTF-8.
    pub canonical: PathBuf,
}

impl std::fmt::Display for NonUtf8WorkingDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the canonical path {} of the working directory is not UTF-8",
            self.canonical.display()
        )
    }
}

impl std::error::Error for NonUtf8WorkingDir {}

/// Per-project mutexes, keyed by the canonical path of the working directory.
///
/// Shared by every runner of one app through an `Arc`, so two runners for the same
/// project serialize their calls.
///
/// # Sharing a lock set with the cache
///
/// The cache's calls ([`bounded::run_bounded`](crate::bounded::run_bounded), the
/// [`CacheBackend`](sc_beads::CacheBackend) methods of [`BdCli`](crate::BdCli)) wait
/// for a lock by polling and give up at their deadline. btit's calls
/// ([`run::spawn_json`](crate::run::spawn_json), and through it every
/// [`BeadsBackend`](sc_beads::backend::BeadsBackend) method of a `BdCli`) keep btit's
/// contract: they wait for the lock without a bound and hold it until their `bd`
/// exits, however long that takes (PE-RSH-012). A hung btit call on a project would
/// therefore make every cache call on that project time out waiting for the lock.
/// **Do not share one `ProjectLocks` between btit's unbounded calls and the cache**:
/// give the cache its own lock set (and so its own `BdCli`), or only make btit calls
/// on projects the cache does not serve.
#[derive(Debug, Default)]
pub struct ProjectLocks {
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl ProjectLocks {
    /// An empty lock map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The mutex for `working_dir`, created on first use.
    ///
    /// A poisoned map (a thread panicked while inserting) is recovered rather than
    /// propagated: the map holds no invariant a panic could break.
    #[must_use]
    pub fn guard(&self, working_dir: &str) -> Arc<Mutex<()>> {
        self.guard_dir(&WorkingDir::new(working_dir))
    }

    /// The mutex for an already normalised `working_dir`; [`guard`](Self::guard) is this
    /// after [`WorkingDir::new`].
    ///
    /// As [`try_guard_dir`](Self::try_guard_dir), except that a directory whose
    /// canonical path is not UTF-8 keys on its lexical form instead of being refused
    /// (this signature cannot fail). The crate's own spawns use `try_guard_dir`.
    #[must_use]
    pub fn guard_dir(&self, working_dir: &WorkingDir) -> Arc<Mutex<()>> {
        let key = lock_key(working_dir).unwrap_or_else(|_| working_dir.as_str().to_owned());
        self.entry(key)
    }

    /// The mutex for `working_dir`, keyed by its canonical path, created on first use
    /// (PE-RBP-F010).
    ///
    /// The directory is canonicalized now, when the lock is taken, so every spelling
    /// of one existing directory (a trailing separator, a symlink, a `..`, on Windows
    /// another letter case) shares one lock. A directory that does not exist (or
    /// cannot be resolved) keys on its lexical [`WorkingDir`] form.
    ///
    /// # Errors
    ///
    /// [`NonUtf8WorkingDir`] when the canonical path is not UTF-8.
    pub fn try_guard_dir(
        &self,
        working_dir: &WorkingDir,
    ) -> Result<Arc<Mutex<()>>, NonUtf8WorkingDir> {
        lock_key(working_dir).map(|key| self.entry(key))
    }

    /// The mutex stored under `key`, created on first use.
    fn entry(&self, key: String) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(locks.entry(key).or_insert_with(|| Arc::new(Mutex::new(()))))
    }
}

/// The lock key of `dir`: its canonical path, or its lexical form when it cannot be
/// canonicalized (it does not exist).
fn lock_key(dir: &WorkingDir) -> Result<String, NonUtf8WorkingDir> {
    if dir.as_str().is_empty() {
        // `canonicalize("")` fails on every platform; say so instead of asking.
        return Ok(String::new());
    }
    match std::fs::canonicalize(dir.as_str()) {
        Ok(path) => path
            .into_os_string()
            .into_string()
            .map(strip_verbatim)
            .map_err(|os| NonUtf8WorkingDir {
                canonical: PathBuf::from(os),
            }),
        Err(_) => Ok(dir.as_str().to_owned()),
    }
}

/// On Windows, `canonicalize` returns a verbatim path (`\\?\C:\p`,
/// `\\?\UNC\host\share`); the key drops that prefix so it reads as the directory
/// does elsewhere (`C:\p`, `\\host\share`). Elsewhere the path is unchanged.
fn strip_verbatim(path: String) -> String {
    if cfg!(windows) {
        if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc}");
        }
        if let Some(rest) = path.strip_prefix(r"\\?\") {
            if rest.as_bytes().get(1) == Some(&b':') {
                return rest.to_owned();
            }
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_working_dir_shares_one_mutex() {
        let locks = ProjectLocks::new();
        let a = locks.guard("/p/one");
        let b = locks.guard("/p/one");
        let c = locks.guard("/p/two");
        assert!(Arc::ptr_eq(&a, &b));
        assert!(!Arc::ptr_eq(&a, &c));
    }

    #[test]
    fn spellings_of_one_directory_share_a_lock() {
        let locks = ProjectLocks::new();
        assert!(Arc::ptr_eq(&locks.guard("/p"), &locks.guard("/p/")));
        assert!(Arc::ptr_eq(&locks.guard("/p"), &locks.guard("/p//")));
        assert!(!Arc::ptr_eq(&locks.guard("/p"), &locks.guard("/q")));
        assert_eq!(WorkingDir::new("/").as_str(), "/");
        assert_eq!(WorkingDir::new("").as_str(), "");
    }

    #[test]
    fn poisoned_map_is_recovered() {
        let locks = Arc::new(ProjectLocks::new());
        let first = locks.guard("/p");
        let poisoner = Arc::clone(&locks);
        let joined = std::thread::spawn(move || {
            let _held = poisoner.locks.lock().unwrap();
            panic!("poison the map");
        })
        .join();
        assert!(joined.is_err());
        assert!(locks.locks.is_poisoned());
        assert!(Arc::ptr_eq(&first, &locks.guard("/p")));
    }

    /// A fresh directory under the system temp directory for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sc-bd-cli-locks-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("project").join("sub")).unwrap();
        dir
    }

    fn utf8(path: &std::path::Path) -> String {
        path.to_str().unwrap().to_owned()
    }

    /// Two spellings of one existing directory, a `..` one and (on Unix) a symlink
    /// one, take the lock of the directory itself (PE-RBP-F010), while another
    /// directory keeps its own.
    #[test]
    fn spellings_of_one_existing_directory_share_a_lock() {
        let root = scratch("spellings");
        let project = root.join("project");
        let locks = ProjectLocks::new();
        let real = locks.guard(&utf8(&project));
        let dotdot = utf8(&project.join("sub").join(".."));
        assert!(Arc::ptr_eq(&real, &locks.guard(&dotdot)), "{dotdot}");
        assert!(Arc::ptr_eq(
            &real,
            &locks.try_guard_dir(&WorkingDir::new(&dotdot)).unwrap()
        ));
        assert!(!Arc::ptr_eq(
            &real,
            &locks.guard(&utf8(&project.join("sub")))
        ));
        #[cfg(unix)]
        {
            let link = root.join("link");
            std::os::unix::fs::symlink(&project, &link).unwrap();
            assert!(Arc::ptr_eq(&real, &locks.guard(&utf8(&link))));
            assert!(Arc::ptr_eq(
                &real,
                &locks.try_guard_dir(&WorkingDir::new(&utf8(&link))).unwrap()
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A directory that does not exist keys on its lexical form.
    #[test]
    fn a_missing_directory_keys_on_its_lexical_form() {
        let locks = ProjectLocks::new();
        let missing = "/definitely/not/a/real/sc-bd-cli/dir";
        assert_eq!(
            lock_key(&WorkingDir::new(&format!("{missing}/"))).unwrap(),
            missing
        );
        assert!(Arc::ptr_eq(
            &locks.guard(missing),
            &locks.try_guard_dir(&WorkingDir::new(missing)).unwrap()
        ));
    }

    /// A directory whose canonical path is not UTF-8 is refused by `try_guard_dir`
    /// (PE-RBP-F010). Linux only: macOS and Windows file systems refuse such names.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_non_utf8_canonical_path_is_refused() {
        use std::os::unix::ffi::OsStrExt as _;
        let root = scratch("non-utf8");
        let target = root.join(std::ffi::OsStr::from_bytes(b"caf\xe9"));
        std::fs::create_dir_all(&target).unwrap();
        let link = root.join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let err = ProjectLocks::new()
            .try_guard_dir(&WorkingDir::new(&utf8(&link)))
            .unwrap_err();
        assert_eq!(err.canonical, std::fs::canonicalize(&target).unwrap());
        assert!(err.to_string().contains("not UTF-8"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
