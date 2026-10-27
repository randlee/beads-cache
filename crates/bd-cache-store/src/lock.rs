//! [`FlockDrainLock`]: the cross-process single drain writer (REQ-BDC-0401).

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use bd_cache_model::{CacheError, DrainGuard, DrainLock};
use fs4::TryLockError;

use crate::error::{Step, failed};

/// An exclusive, non-blocking `flock` on the cache file's drain lock file.
///
/// The lock file sits next to the cache file and is named as BD-CACHE v3
/// names it: the cache file's last extension replaced by `drain.lock`
/// (`dev.bd-cache.sqlite3` → `dev.bd-cache.drain.lock`), so the Rust
/// drain and the v3 Python writer's drain exclude each other. Every
/// [`try_acquire`](DrainLock::try_acquire) opens its own file handle, so two
/// handles on one path contend even inside one process, and the lock is not
/// reentrant. The guard holds the open file; dropping it closes the file,
/// which releases the lock (also when the process dies). As with any `flock`,
/// a child process forked at that instant by another thread holds a copy of
/// the descriptor until it execs (Rust opens files close-on-exec), so the
/// release can lag a spawn on another thread by that long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlockDrainLock {
    path: PathBuf,
}

impl FlockDrainLock {
    /// The drain lock of the cache file `cache_file`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_store::FlockDrainLock;
    /// let cache = std::path::Path::new("repo").join(".bd-cache").join("dev.bd-cache.sqlite3");
    /// let lock = FlockDrainLock::for_cache_file(&cache);
    /// assert!(lock.path().ends_with("dev.bd-cache.drain.lock"));
    /// ```
    #[must_use]
    pub fn for_cache_file(cache_file: &Path) -> Self {
        Self {
            path: cache_file.with_extension("drain.lock"),
        }
    }

    /// The lock file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Holds the drain lock until dropped.
#[derive(Debug)]
struct FlockGuard {
    // Never read: the open handle is the lock; closing it releases the lock.
    _file: File,
}

impl DrainGuard for FlockGuard {}

impl DrainLock for FlockDrainLock {
    fn try_acquire(&self) -> Result<Option<Box<dyn DrainGuard>>, CacheError> {
        let step = |what| Step {
            what,
            path: &self.path,
        };
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .map_err(|e| failed(step("open the drain lock file"), e))?;
        match fs4::FileExt::try_lock(&file) {
            Ok(()) => Ok(Some(Box::new(FlockGuard { _file: file }))),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(e)) => Err(failed(step("lock the drain lock file"), e)),
        }
    }
}
