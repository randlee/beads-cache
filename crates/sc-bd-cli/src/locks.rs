//! [`ProjectLocks`]: one mutex per project working directory, and the cross-process
//! lock file beside it (lead ruling S81, RSH-004).
//!
//! bd 0.55 uses embedded Dolt, which crashes (SIGSEGV) when two bd processes access
//! the same database simultaneously, so every `--json` invocation holds the lock of
//! its working directory for the lifetime of the child process. Entries are never
//! removed: the map grows by one entry per distinct working directory.
//!
//! A mutex only excludes the threads of one process, while two processes (a cron
//! `cache tick` and an agent's `bdc get`) may call `bd` on one project at once. So a
//! deadline-bounded call ([`bounded`](crate::bounded)) also holds an advisory lock on
//! a file of the project, `ProjectFileLock` (crate-private), for as long as it holds
//! the mutex (S81; ADR-BDCLI-0006). btit's unbounded calls ([`run`](crate::run)) take
//! the mutex only, as btit did.
//!
//! The file lock is taken only for an embedded-Dolt project: when the project's
//! `.beads/metadata.json` says bd runs against a Dolt server
//! ([`DoltMode::Server`](crate::DoltMode::Server), by bd's own rule:
//! `dolt_mode: "server"`, or a non-local `dolt_server_host`), many `bd` processes are
//! concurrent by design and the lock would only queue every call behind a long
//! sync (atm-bd runs bd that way). A missing or unreadable `metadata.json`, and every
//! other mode, count as embedded: the lock is taken. The protection is partial in
//! embedded mode too: agents and bd's git hooks run `bd` directly, without this lock.
//!
//! The key of a lock is the directory's canonical path, resolved when the lock is
//! taken (PE-RBP-F010): `/p/link` and `/p/sub/..` share the lock of the directory they
//! name. A directory that does not exist keys on its lexical [`WorkingDir`] form.
//!
//! # The lock file
//!
//! `<project>/.beads/sc-bd-cli.lock`, where `<project>` is the canonical key above,
//! when that file can be opened (created read-write, or an existing one read-only);
//! otherwise `<temp>/sc-bd-cli-<hash>.lock` in the OS temp directory, `<hash>` a
//! stable 64-bit FNV-1a of the key, so every process that cannot use `.beads/` meets
//! in the same file. The lock is `flock` (Unix) or `LockFileEx` (Windows) on an open
//! handle, taken without blocking ([`std::fs::File::try_lock`]) and polled against the
//! call's deadline; closing the handle releases it, on every return and when the
//! process dies, so a crashed host never leaves it held. The file is never deleted
//! (deleting a lock file races a process that just opened it). Two processes that
//! disagree on whether `.beads/` is usable (one may create the file there, the other
//! may not even read it) would use different files; an existing file is opened
//! read-only for that reason, which `flock` allows.
//!
//! A child that another thread of the process forks while a guard is held inherits
//! the open file (and so the lock) until it execs; Rust opens files close-on-exec,
//! so the lock is released then. Dropping a guard can therefore release the lock a
//! moment late, and callers poll for it (the bounded calls do, against their
//! deadline) rather than expecting it free at once.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
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

    /// The mutex of `working_dir` and its cross-process lock file, both keyed by its
    /// canonical path (resolved once).
    ///
    /// # Errors
    ///
    /// As [`try_guard_dir`](Self::try_guard_dir).
    pub(crate) fn try_guard_and_file(
        &self,
        working_dir: &WorkingDir,
    ) -> Result<(Arc<Mutex<()>>, Option<ProjectFileLock>), NonUtf8WorkingDir> {
        let key = lock_key(working_dir)?;
        let file = ProjectFileLock::for_project(&key);
        Ok((self.entry(key), file))
    }

    /// The mutex stored under `key`, created on first use.
    fn entry(&self, key: String) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(locks.entry(key).or_insert_with(|| Arc::new(Mutex::new(()))))
    }
}

/// The name of the lock file in a project's `.beads/` directory.
pub(crate) const LOCK_FILE_NAME: &str = "sc-bd-cli.lock";

/// The cross-process lock of one project (lead ruling S81, RSH-004): see the module
/// docs, "The lock file".
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectFileLock {
    /// `<project>/.beads/sc-bd-cli.lock`, tried first.
    in_project: Option<PathBuf>,
    /// The OS temp directory's file for the same key.
    in_temp: PathBuf,
}

/// Holds a project's cross-process lock until dropped: the open handle is the lock.
#[derive(Debug)]
pub(crate) struct FileGuard {
    // Never read: closing the handle releases the lock.
    _file: File,
}

impl ProjectFileLock {
    /// The cross-process lock the project whose canonical key is `key` needs: `None`
    /// for a Dolt server-mode project (module docs), the lock otherwise.
    pub(crate) fn for_project(key: &str) -> Option<Self> {
        let server = !key.is_empty()
            && crate::dolt::project_dolt_mode(&Path::new(key).join(".beads"))
                == Some(crate::DoltMode::Server);
        (!server).then(|| Self::for_key(key))
    }

    /// The lock of the project whose canonical key is `key`.
    pub(crate) fn for_key(key: &str) -> Self {
        let in_project =
            (!key.is_empty()).then(|| Path::new(key).join(".beads").join(LOCK_FILE_NAME));
        let in_temp = std::env::temp_dir().join(format!("sc-bd-cli-{:016x}.lock", fnv1a(key)));
        Self {
            in_project,
            in_temp,
        }
    }

    /// Takes the lock without blocking: `Ok(None)` while another handle holds it.
    ///
    /// # Errors
    ///
    /// The error of opening the temp-directory file when neither file can be opened,
    /// or of the lock call itself.
    pub(crate) fn try_acquire(&self) -> io::Result<Option<FileGuard>> {
        let file = match self.in_project.as_deref().map(open_lock_file) {
            Some(Ok(file)) => file,
            _ => open_lock_file(&self.in_temp)?,
        };
        match file.try_lock() {
            Ok(()) => Ok(Some(FileGuard { _file: file })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }

    /// The file the lock lives in now: the project's when it can be opened.
    #[cfg(test)]
    pub(crate) fn path(&self) -> PathBuf {
        match &self.in_project {
            Some(p) if open_lock_file(p).is_ok() => p.clone(),
            _ => self.in_temp.clone(),
        }
    }
}

/// Opens (creating it if need be) the lock file at `path` read-write, or an existing
/// one read-only, which `flock` and `LockFileEx` accept too. On Unix a symlink at the
/// path is not followed (`O_NOFOLLOW`): the temp directory is shared.
fn open_lock_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    match options.open(path) {
        Ok(file) => Ok(file),
        Err(e)
            if e.kind() == io::ErrorKind::PermissionDenied
                || e.kind() == io::ErrorKind::ReadOnlyFilesystem =>
        {
            let mut read_only = OpenOptions::new();
            read_only.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                read_only.custom_flags(libc::O_NOFOLLOW);
            }
            read_only.open(path)
        }
        Err(e) => Err(e),
    }
}

/// The 64-bit FNV-1a hash of `text`: stable across processes, builds and Rust
/// versions (unlike `DefaultHasher`), so every process names one temp file per key.
fn fnv1a(text: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    text.bytes().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
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

    /// The lock file is the project's `.beads/sc-bd-cli.lock` when `.beads/` exists,
    /// else the temp directory's file for the key; two handles on it exclude each
    /// other (also inside one process), and dropping the guard releases it.
    #[test]
    fn the_file_lock_lives_in_beads_or_the_temp_dir_and_excludes() {
        let root = scratch("file-lock");
        let project = root.join("project");
        let key = lock_key(&WorkingDir::new(&utf8(&project))).unwrap();
        let without = ProjectFileLock::for_key(&key);
        assert_eq!(without.path(), without.in_temp, "no .beads/ yet");
        std::fs::create_dir_all(project.join(".beads")).unwrap();
        let lock = ProjectFileLock::for_key(&key);
        assert_eq!(
            lock.path(),
            Path::new(&key).join(".beads").join(LOCK_FILE_NAME)
        );
        let held = lock.try_acquire().unwrap();
        assert!(held.is_some());
        assert!(
            lock.try_acquire().unwrap().is_none(),
            "a second handle must wait"
        );
        drop(held);
        // Released on drop, but a sibling test's fork may hold the file until its
        // child execs (module docs), so the release is polled for (RCR-001).
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if lock.try_acquire().unwrap().is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < until,
                "not released on drop within 2 s"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Only a Dolt server-mode project goes without the file lock; an embedded one,
    /// one with another or no mode, and one without `metadata.json` take it.
    #[test]
    fn a_server_mode_project_takes_no_file_lock() {
        let root = scratch("server-mode");
        let project = root.join("project");
        std::fs::create_dir_all(project.join(".beads")).unwrap();
        let key = lock_key(&WorkingDir::new(&utf8(&project))).unwrap();
        assert!(
            ProjectFileLock::for_project(&key).is_some(),
            "no metadata.json"
        );
        let meta = project.join(".beads").join("metadata.json");
        for (text, locked) in [
            (r#"{"dolt_mode":"server"}"#, false),
            (r#"{"dolt_mode":"Server"}"#, false),
            (r#"{"dolt_mode":"embedded"}"#, true),
            (r#"{"dolt_mode":"proxied-server"}"#, true),
            ("{}", true),
            ("not json", true),
        ] {
            std::fs::write(&meta, text).unwrap();
            assert_eq!(
                ProjectFileLock::for_project(&key).is_some(),
                locked,
                "{text}"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The temp-directory name is a stable hash of the key.
    #[test]
    fn the_temp_name_is_a_stable_hash_of_the_key() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("a"), 0xaf63_dc4c_8601_ec8c);
        let lock = ProjectFileLock::for_key("/p");
        assert!(lock
            .in_temp
            .ends_with(format!("sc-bd-cli-{:016x}.lock", fnv1a("/p"))));
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
