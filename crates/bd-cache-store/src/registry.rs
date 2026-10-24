//! One connection per cache file per process (NFR-BDC-0007).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Cache files this process holds a connection to.
///
/// A `Mutex` because stores are opened and dropped from any thread; the set is
/// only touched in [`Registration::claim`] and `Drop`, never across I/O, so
/// contention is negligible. A poisoned lock still holds a consistent set
/// (every critical section is a single insert or remove), so it is recovered.
static OPEN: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

fn open_files() -> MutexGuard<'static, BTreeSet<PathBuf>> {
    OPEN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// This process's claim on a cache file; released on drop.
#[derive(Debug)]
pub(crate) struct Registration {
    key: PathBuf,
}

impl Registration {
    /// Claims `key` (a canonical path); `None` when the process already holds it.
    pub(crate) fn claim(key: &Path) -> Option<Self> {
        open_files().insert(key.to_path_buf()).then(|| Self {
            key: key.to_path_buf(),
        })
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        open_files().remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_claimed_once_until_released() {
        let key = Path::new("/registry-test/x.sqlite3");
        let first = Registration::claim(key);
        assert!(first.is_some());
        assert!(Registration::claim(key).is_none());
        drop(first);
        assert!(Registration::claim(key).is_some());
    }
}
