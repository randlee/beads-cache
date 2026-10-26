//! Size limits on what a caller can put in the journal (PE-RSH-009).
//!
//! The journal keeps every write's args, idempotency key and, for a resolved
//! entry, the lead's note, and every read replays the pending ones. An
//! unbounded value would grow the shared cache file and every read without
//! limit, so each is capped here and checked by the core before anything is
//! journaled ([`check_write_size`], [`check_note_size`]). An oversized value
//! is `BD_CACHE.BAD_ARGS`, a validation error the caller fixes, never a
//! store failure.
//!
//! The caps are far above any real write: a bd-cache write is one bead's
//! status, labels, metadata or a note, a few hundred bytes to a few KiB.

use crate::error::CacheError;
use crate::journal::IdemKey;

/// The largest `args_json` a write may journal, in bytes (256 KiB).
///
/// The biggest real args are an `append_notes` text; 256 KiB holds a long
/// report and keeps a replay of many pending entries cheap.
pub const MAX_WRITE_ARGS_BYTES: usize = 256 * 1024;

/// The largest note `resolve` may append to an entry's `last_error`, in bytes (16 KiB).
///
/// A note says how the lead settled the entry: a sentence or a short paragraph.
pub const MAX_NOTE_BYTES: usize = 16 * 1024;

/// The largest idempotency key, in bytes (1 KiB).
///
/// A key is derived from the operation (REQ-BDC-0004), e.g. a hash or
/// `<bead>:<op>:<run>`; it is indexed, so it stays short.
pub const MAX_IDEM_KEY_BYTES: usize = 1024;

/// The literal of [`MAX_TIMEOUT_MS`], so a `concat!` (a refusal's range, a
/// command's help line) states the ceiling from this one value; `concat!`
/// takes literals only.
#[macro_export]
macro_rules! max_timeout_ms {
    () => {
        600000
    };
}

/// The largest timeout or budget any command takes, in milliseconds: ten
/// minutes.
///
/// One ceiling for the sync row's `budget_ms`, the Python row's
/// `--timeout-ms`, `cache refresh --timeout-ms` and `cron run
/// --shutdown-timeout-ms`. Past it a request would hold a command (and its
/// locks) for any `u64` of milliseconds, and the sync lease the store accepts
/// ([`MAX_LEASE_TTL`](crate::MAX_LEASE_TTL)) is sized from it. Raising it
/// lengthens all of them together.
pub const MAX_TIMEOUT_MS: u64 = max_timeout_ms!();

fn too_large(what: &str, len: usize, max: usize) -> CacheError {
    CacheError::BadArgs {
        detail: format!("{what} is {len} bytes, over the {max}-byte limit; nothing journaled"),
    }
}

/// Refuses a write whose args or key is over its limit, before it is parsed or journaled.
///
/// # Errors
///
/// `BAD_ARGS` naming the value, its size and the limit
/// ([`MAX_WRITE_ARGS_BYTES`], [`MAX_IDEM_KEY_BYTES`]).
///
/// # Examples
///
/// ```
/// use bd_cache_model::{IdemKey, MAX_WRITE_ARGS_BYTES, check_write_size};
/// let key = IdemKey::new("k1").unwrap();
/// assert!(check_write_size("{}", &key).is_ok());
/// let big = "x".repeat(MAX_WRITE_ARGS_BYTES + 1);
/// assert_eq!(check_write_size(&big, &key).unwrap_err().code(), "BD_CACHE.BAD_ARGS");
/// ```
pub fn check_write_size(args_json: &str, key: &IdemKey) -> Result<(), CacheError> {
    if args_json.len() > MAX_WRITE_ARGS_BYTES {
        return Err(too_large("args", args_json.len(), MAX_WRITE_ARGS_BYTES));
    }
    let key_len = key.as_str().len();
    if key_len > MAX_IDEM_KEY_BYTES {
        return Err(too_large("idem_key", key_len, MAX_IDEM_KEY_BYTES));
    }
    Ok(())
}

/// Refuses a `resolve` note over [`MAX_NOTE_BYTES`], before it is journaled.
///
/// # Errors
///
/// `BAD_ARGS` naming the note's size and the limit.
pub fn check_note_size(note: &str) -> Result<(), CacheError> {
    if note.len() > MAX_NOTE_BYTES {
        return Err(too_large("note", note.len(), MAX_NOTE_BYTES));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(len: usize) -> IdemKey {
        IdemKey::new("k".repeat(len)).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn each_limit_admits_its_size_and_refuses_one_byte_more() {
        let args = "a".repeat(MAX_WRITE_ARGS_BYTES);
        assert!(check_write_size(&args, &key(1)).is_ok());
        let over = format!("{args}a");
        let e = check_write_size(&over, &key(1)).unwrap_err();
        assert_eq!(e.code(), "BD_CACHE.BAD_ARGS");
        assert!(e.detail().starts_with("args is 262145 bytes"), "{e}");

        assert!(check_write_size("{}", &key(MAX_IDEM_KEY_BYTES)).is_ok());
        let e = check_write_size("{}", &key(MAX_IDEM_KEY_BYTES + 1)).unwrap_err();
        assert_eq!(e.code(), "BD_CACHE.BAD_ARGS");
        assert!(e.detail().starts_with("idem_key is 1025 bytes"), "{e}");

        assert!(check_note_size(&"n".repeat(MAX_NOTE_BYTES)).is_ok());
        let e = check_note_size(&"n".repeat(MAX_NOTE_BYTES + 1)).unwrap_err();
        assert_eq!(e.code(), "BD_CACHE.BAD_ARGS");
        assert!(e.detail().contains("16384-byte limit"), "{e}");
    }
}
