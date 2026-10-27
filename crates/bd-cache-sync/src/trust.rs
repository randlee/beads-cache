//! The trust check before any row's or hook's code runs (REQ-BDC-0723, ADR-SYNC-0008).
//!
//! [`TrustedCacheFile`] is a proof value only [`TrustedCacheFile::check`]
//! builds: the cache file is owned by this process's effective user and is
//! neither group- nor world-writable. The post-sync hook runner and the Python
//! row take it by value, so no code runs without a check made for that
//! launch; it is neither `Clone` nor `Copy`, so the `cron.stop` and
//! `cron.tick` launches of one run each make their own.
//!
//! The effective uid comes from `rustix::process::geteuid`, a safe wrapper
//! (QA97R2-014): this crate keeps no `unsafe` call. On Windows the ACL check
//! is a documented gap, and every file passes.

use std::fmt;
use std::path::{Path, PathBuf};

/// Proof that the cache file passed the trust check for one launch.
#[derive(Debug)]
pub struct TrustedCacheFile {
    path: PathBuf,
}

/// The cache file failed the trust check: no code is run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Untrusted {
    /// The cache file.
    pub path: PathBuf,
    /// Why, e.g. `group- or world-writable (mode 0664)`.
    pub reason: String,
}

impl fmt::Display for Untrusted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the cache file {} is not trusted: {}; no hook or Python row runs from it",
            self.path.display(),
            self.reason
        )
    }
}

impl std::error::Error for Untrusted {}

impl TrustedCacheFile {
    /// Checks `path`: owned by the effective user, not group- or world-writable (Unix).
    ///
    /// # Errors
    ///
    /// [`Untrusted`] when the file cannot be read, has another owner, or is
    /// group- or world-writable.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(unix)] {
    /// use std::os::unix::fs::PermissionsExt;
    /// use bd_cache_sync::TrustedCacheFile;
    ///
    /// let file = tempfile::NamedTempFile::new().unwrap();
    /// std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o600)).unwrap();
    /// assert!(TrustedCacheFile::check(file.path()).is_ok());
    /// std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o664)).unwrap();
    /// assert!(TrustedCacheFile::check(file.path()).is_err());
    /// # }
    /// ```
    pub fn check(path: &Path) -> Result<Self, Untrusted> {
        check_platform(path)?;
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// The checked file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(unix)]
fn check_platform(path: &Path) -> Result<(), Untrusted> {
    use std::os::unix::fs::MetadataExt as _;
    let untrusted = |reason: String| Untrusted {
        path: path.to_path_buf(),
        reason,
    };
    let meta = std::fs::metadata(path).map_err(|e| untrusted(format!("cannot read it: {e}")))?;
    let euid = rustix::process::geteuid().as_raw();
    if meta.uid() != euid {
        return Err(untrusted(format!(
            "owned by uid {}, not by the current user (uid {euid})",
            meta.uid()
        )));
    }
    let mode = meta.mode() & 0o7777;
    if mode & 0o022 != 0 {
        return Err(untrusted(format!(
            "group- or world-writable (mode {mode:04o})"
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_platform(path: &Path) -> Result<(), Untrusted> {
    // The Windows ACL check is a documented gap (REQ-BDC-0723): the file
    // must exist, and nothing more is checked.
    std::fs::metadata(path).map(|_| ()).map_err(|e| Untrusted {
        path: path.to_path_buf(),
        reason: format!("cannot read it: {e}"),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    #[test]
    fn a_private_file_is_trusted_and_a_group_or_world_writable_one_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.sqlite3");
        std::fs::write(&path, b"").unwrap();
        for (mode, trusted) in [(0o600, true), (0o644, true), (0o664, false), (0o646, false)] {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            assert_eq!(TrustedCacheFile::check(&path).is_ok(), trusted, "{mode:o}");
        }
        assert!(TrustedCacheFile::check(&dir.path().join("missing")).is_err());
    }
}
