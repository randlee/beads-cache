//! How the store reports its own failures (ADR-STORE-0003).
//!
//! A failure of the store itself is `BD_CACHE.STORE_FAILED` (a-1 amendment
//! 1), with `detail` naming the failing step and the file. SQLite, file I/O
//! and `serde_json` failures keep the underlying error as the source
//! ([`failed`], [`CacheError::store_failed`]). A failure with no underlying
//! error has no source ([`corrupt`]): a stored value that does not decode or
//! is inconsistent, a refused one-connection claim, or a journal mode SQLite
//! will not set to WAL. A type-conversion error rusqlite raises while
//! reading a column keeps that rusqlite error as its source ([`failed`]); a
//! value the store's own validation refuses after reading has no source
//! ([`corrupt`]). `NO_DATABASE` stays for missing or invalid
//! `metadata.json` only. This module makes the mapping.

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
        // Writing to a String cannot fail.
        write!(text, ": {cause}").unwrap_or_default();
        next = cause.source();
    }
    text
}

/// A failure of the store's own machinery: `STORE_FAILED`, keeping `e` as the source.
pub(crate) fn failed(step: Step<'_>, e: impl Error + Send + Sync + 'static) -> CacheError {
    CacheError::store_failed(format!("{step}: {}", chain(&e)), e)
}

/// A store failure with no underlying error (a stored value that does not
/// decode or is inconsistent, a refused one-connection claim, WAL refused):
/// `STORE_FAILED`, no source.
pub(crate) fn corrupt(step: Step<'_>, detail: impl fmt::Display) -> CacheError {
    CacheError::StoreFailed {
        detail: format!("{step}: {detail}"),
        source: None,
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
            path: &Path::new("cache").join("x.sqlite3"),
        };
        let err = failed(step, e);
        assert_eq!(err.code(), "BD_CACHE.STORE_FAILED");
        let source = std::error::Error::source(&err).expect("the source is kept");
        assert_eq!(source.to_string(), "outer");
        assert_eq!(
            source.source().map(ToString::to_string).as_deref(),
            Some("disk full"),
            "the whole chain below it too"
        );
        let file = Path::new("cache").join("x.sqlite3");
        assert_eq!(
            err.detail(),
            format!("cache store {}: upsert: outer: disk full", file.display())
        );
    }
}
