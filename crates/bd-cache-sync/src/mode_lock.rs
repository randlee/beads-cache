//! The mode lock: every mode-changing command decides the rows' flags and
//! writes them inside one exclusive section (REQ-BDC-0720, 0724; ADR-SYNC-0008
//! as amended by sprint b-8).
//!
//! sc-cron's multi-row writes (`replace_and_set_enabled`, `set_enabled_many`)
//! are one `BEGIN IMMEDIATE` transaction each, but the rows they are decided
//! from are read before that transaction: two commands racing with stale
//! reads (a `schedule set` that read no Python row while `sync use-python`
//! created it) could leave both rows enabled. So `schedule set`, `schedule
//! enable|disable|clear`, `sync use-python` (its mode commit and its restore)
//! and `sync use-builtin` read the rows, decide, and write the one store
//! call while they hold an exclusive `flock` on the cache file's mode lock
//! file, `<cache file stem>.mode.lock` beside it. The lock is held for one
//! read and one write only, never while a run runs, so `sync use-builtin`
//! still works while a Python run holds the sync lease (REQ-BDC-0722). It is
//! released when the guard drops, and by the OS when the process dies.
//!
//! The section is short: one table read and one store transaction, each
//! bounded by the store's busy timeout. A command waits at most
//! [`MODE_LOCK_WAIT`] for it, then fails with `STORE_FAILED`.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bd_cache_model::CacheError;

use crate::run::blocking;

/// The longest a mode-changing command waits for the mode lock: far more
/// than one read and one write take (each bounded by the store's busy
/// timeout), so a timeout means a holder that hangs.
pub const MODE_LOCK_WAIT: Duration = Duration::from_secs(30);

/// How often a waiter tries the lock again.
const RETRY: Duration = Duration::from_millis(5);

/// The mode lock of one cache file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModeLock {
    path: PathBuf,
}

/// Holds the mode lock until dropped.
#[derive(Debug)]
pub(crate) struct ModeGuard {
    // Never read: the open handle is the lock; closing it releases it.
    _file: File,
}

impl ModeLock {
    /// The mode lock of `cache_file`: its last extension replaced by `mode.lock`.
    pub(crate) fn for_cache_file(cache_file: &Path) -> Self {
        Self {
            path: cache_file.with_extension("mode.lock"),
        }
    }

    /// Takes the lock, waiting at most [`MODE_LOCK_WAIT`] on the blocking pool.
    ///
    /// # Errors
    ///
    /// `STORE_FAILED` when the file cannot be opened or locked, or another
    /// process holds the lock past the wait.
    pub(crate) async fn hold(&self) -> Result<ModeGuard, CacheError> {
        let path = self.path.clone();
        blocking(move || hold_within(&path, MODE_LOCK_WAIT)).await
    }
}

fn failed(path: &Path, what: &str, e: impl std::fmt::Display) -> CacheError {
    CacheError::StoreFailed {
        detail: format!("the sync mode lock {}: {what}: {e}", path.display()),
        source: None,
    }
}

fn hold_within(path: &Path, wait: Duration) -> Result<ModeGuard, CacheError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| failed(path, "cannot open it", e))?;
    let until = Instant::now()
        .checked_add(wait)
        .unwrap_or_else(Instant::now);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(ModeGuard { _file: file }),
            Err(TryLockError::Error(e)) => return Err(failed(path, "cannot lock it", e)),
            Err(TryLockError::WouldBlock) => {}
        }
        let now = Instant::now();
        if now >= until {
            return Err(failed(
                path,
                "another command holds it",
                format!("still held after {} s", wait.as_secs()),
            ));
        }
        std::thread::sleep((until - now).min(RETRY));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two holders exclude each other, in one process too; the second
    /// waits and then fails, and takes the lock once the first lets go.
    #[test]
    fn the_mode_lock_is_exclusive_and_released_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let lock = ModeLock::for_cache_file(&dir.path().join("dev.bd-cache.sqlite3"));
        assert!(lock.path.ends_with("dev.bd-cache.mode.lock"));
        let first = hold_within(&lock.path, Duration::ZERO).unwrap();
        let second = hold_within(&lock.path, Duration::from_millis(20));
        assert_eq!(
            second.err().map(|e| e.code()),
            Some("BD_CACHE.STORE_FAILED")
        );
        drop(first);
        assert!(hold_within(&lock.path, Duration::ZERO).is_ok());
    }
}
