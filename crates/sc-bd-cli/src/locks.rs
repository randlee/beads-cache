//! [`ProjectLocks`]: one mutex per project working directory.
//!
//! bd 0.55 uses embedded Dolt, which crashes (SIGSEGV) when two bd processes access
//! the same database simultaneously, so every `--json` invocation holds the lock of
//! its working directory for the lifetime of the child process. Entries are never
//! removed: the map grows by one entry per distinct working directory.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

/// A project working directory, normalised so that spellings of one directory share a
/// lock: trailing path separators are dropped (`/p/` is `/p`), and a root keeps its
/// separator (`/`, `C:\`).
///
/// Built by [`run::resolve_project_dir`](crate::run::resolve_project_dir) and taken by
/// [`ProjectLocks::guard_dir`] and [`bounded::run_bounded`](crate::bounded::run_bounded),
/// so a lock key is never a raw string. It does not touch the filesystem: no symlink or
/// case resolution, which would need the directory to exist.
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

/// Per-project mutexes, keyed by working directory.
///
/// Shared by every runner of one app through an `Arc`, so two runners for the same
/// project serialize their calls.
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
    /// after [`WorkingDir::new`], so every spelling of a directory shares one lock.
    #[must_use]
    pub fn guard_dir(&self, working_dir: &WorkingDir) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(
            locks
                .entry(working_dir.as_str().to_owned())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }
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
}
