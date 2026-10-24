//! How the store reports its own failures (ADR-STORE-0003).
//!
//! a-1's [`CacheError`] has a code for every BD-CACHE outcome but none for a
//! failure of the store itself (SQLite, file I/O, a stored value that no
//! longer decodes). Until a-1 adds one, every such failure is reported as
//! `BD_CACHE.NO_DATABASE` (the cache file cannot be used), with the failing
//! step, the file and the whole source chain in `detail`. This module is the
//! one place that mapping is made, so the amendment changes only it.

use std::error::Error;
use std::fmt::{self, Write as _};
use std::path::Path;

use bd_cache_model::CacheError;

/// The step that failed and the file it failed on.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Step<'a> {
    pub(crate) what: &'a str,
    pub(crate) path: &'a Path,
}

impl fmt::Display for Step<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cache store {}: {}", self.path.display(), self.what)
    }
}

/// Renders an error and every `source` below it, outermost first.
pub(crate) fn chain(e: &dyn Error) -> String {
    let mut text = e.to_string();
    let mut next = e.source();
    while let Some(cause) = next {
        let _ = write!(text, ": {cause}");
        next = cause.source();
    }
    text
}

/// A failure of the store's own machinery (SQLite or the file system).
pub(crate) fn failed(step: Step<'_>, e: &dyn Error) -> CacheError {
    CacheError::NoDatabase {
        detail: format!("{step}: {}", chain(e)),
    }
}

/// A stored value that does not decode (a corrupt or foreign row).
pub(crate) fn corrupt(step: Step<'_>, detail: impl fmt::Display) -> CacheError {
    CacheError::NoDatabase {
        detail: format!("{step}: {detail}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Outer(std::io::Error);

    impl fmt::Display for Outer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("outer")
        }
    }

    impl Error for Outer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn failures_carry_the_step_the_file_and_the_whole_chain() {
        let e = Outer(std::io::Error::other("disk full"));
        let step = Step {
            what: "upsert",
            path: Path::new("/tmp/x.sqlite3"),
        };
        let err = failed(step, &e);
        assert_eq!(err.code(), "BD_CACHE.NO_DATABASE");
        assert_eq!(
            err.detail(),
            "cache store /tmp/x.sqlite3: upsert: outer: disk full"
        );
    }
}
