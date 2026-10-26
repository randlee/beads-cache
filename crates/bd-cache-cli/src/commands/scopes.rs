//! `cache scopes add|set|remove|list|ingest|announce` (ADR-BDC-0013, ADR-BDC-0015,
//! REQ-BDC-0721, 0726, 0728): their argv rows, and the bounded stdin of `ingest`.
//!
//! They are `bd-cache-api` operations, run like the other management verbs.
//! `cache scopes` stays the read verb and is `scopes list`'s alias.
//! `scopes ingest` reads its snapshot from stdin through a limit of
//! [`MAX_INGEST_BYTES`] + 1 bytes, so an input over the limit is refused
//! (`INGEST_INVALID`, `too_large`) before it is held whole; the rows are then
//! counted as they decode (`bd-cache-model`'s `IngestSnapshot::from_json`).

use std::io::Read;

use bd_cache_api::{ApiError, Command};
use bd_cache_model::{CacheError, IngestReason, MAX_INGEST_BYTES, ScopeId};
use serde_json::Value;

use crate::args::{ArgSpec, CommandSpec, Shape, Target, Ty};

use Shape::{Many, One};
use Ty::Str;

const SCOPE: ArgSpec = ArgSpec::positional("scope", One, Str, true, "scope", "The scope's name, as written.");

/// The filter arguments of `add` and `set` (ADR-BDC-0013).
const FILTER: [ArgSpec; 6] = [
    ArgSpec::switch("all", "all", "Every bead (alone, without a criterion)."),
    ArgSpec::flag("labels-all", "LABEL", Many, Str, false, "labels_all", "A label a member must carry (repeat: all of them)."),
    ArgSpec::flag("labels-any", "LABEL", Many, Str, false, "labels_any", "Labels of which a member carries one (repeat)."),
    ArgSpec::flag("status", "STATUS", Many, Str, false, "status", "A status a member may have (repeat)."),
    ArgSpec::flag("type", "TYPE", One, Str, false, "type", "The issue type."),
    ArgSpec::flag("assignee", "WHO", One, Str, false, "assignee", "The assignee."),
];

const ADD_ARGS: [ArgSpec; 7] = [SCOPE, FILTER[0], FILTER[1], FILTER[2], FILTER[3], FILTER[4], FILTER[5]];

/// The `cache scopes` subcommands.
pub(crate) const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        path: "cache scopes add",
        about: "Register a scope: a name and a bd filter (all, or labels, status, type, assignee).",
        target: Target::Api(Command::ScopesAdd),
        args: &ADD_ARGS,
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes set",
        about: "Change a registered scope's filter; its memberships start again.",
        target: Target::Api(Command::ScopesSet),
        args: &ADD_ARGS,
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes remove",
        about: "cache evict, then remove the scope from the registry (PRUNE_BLOCKED as evict).",
        target: Target::Api(Command::ScopesRemove),
        args: &[SCOPE],
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes list",
        about: "Every scope with its filter and sync state; the same reply as cache scopes.",
        target: Target::Api(Command::ScopesList),
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes ingest",
        about: "Store a snapshot of a registered scope from stdin: {\"rows\": [...], \"blocked\": [...]} or a rows array.",
        target: Target::Api(Command::ScopesIngest),
        args: &[
            SCOPE,
            ArgSpec::flag("revision", "R", One, Str, false, "revision", "An opaque revision stored with the snapshot."),
            ArgSpec::switch("allow-empty", "allow_empty", "Accept a snapshot with no row."),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes announce",
        about: "Mark the scope's current failure notified when --since is its failing_since.",
        target: Target::Api(Command::ScopesAnnounce),
        args: &[
            SCOPE,
            ArgSpec::flag("since", "TOKEN", One, Str, true, "since", "The failing_since token the hook was given."),
        ],
        one_of: None,
    },
];

/// Adds the snapshot read from `input` (stdin) to the `scopes ingest` request.
///
/// # Errors
///
/// `INGEST_INVALID` (`too_large`) for more than [`MAX_INGEST_BYTES`] of
/// input, before it is held whole; `BAD_ARGS` for input that cannot be read
/// or is not JSON (`row_invalid` is decided by the decoder).
pub(crate) fn merge_stdin(request: &mut Value, input: impl Read) -> Result<(), ApiError> {
    let limit = u64::try_from(MAX_INGEST_BYTES).unwrap_or(u64::MAX).saturating_add(1);
    let mut buf = Vec::new();
    input
        .take(limit)
        .read_to_end(&mut buf)
        .map_err(|e| ApiError::bad_args(format!("scopes ingest: cannot read stdin: {e}")))?;
    if buf.len() > MAX_INGEST_BYTES {
        let scope = request
            .get("scope")
            .and_then(Value::as_str)
            .and_then(|s| ScopeId::new(s).ok())
            .ok_or_else(|| ApiError::bad_member("scope", "scope is required"))?;
        return Err(ApiError::from(&CacheError::IngestInvalid {
            detail: format!("stdin holds more than {MAX_INGEST_BYTES} bytes; one ingest takes at most that"),
            scope,
            reason: IngestReason::TooLarge,
            index: None,
        }));
    }
    let body: Value = serde_json::from_slice(&buf)
        .map_err(|e| ApiError::bad_args(format!("scopes ingest: stdin is not JSON: {e}")))?;
    let Value::Object(req) = request else {
        return Err(ApiError::bad_args("the request must be a JSON object"));
    };
    match body {
        rows @ Value::Array(_) => {
            req.insert("rows".to_owned(), rows);
        }
        Value::Object(snapshot) => {
            for (k, v) in snapshot {
                req.insert(k, v);
            }
        }
        _ => return Err(ApiError::bad_args("scopes ingest: stdin must be a rows array or an object")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, reason = "test code: JSON indexing")]

    use super::*;
    use serde_json::json;

    /// Over 64 MiB of stdin is refused as `too_large` without holding it all
    /// (the read stops at the limit + 1); planted negative: a small body is merged.
    #[test]
    fn stdin_over_the_limit_is_refused_before_it_is_held_whole() {
        let huge = std::io::repeat(b' ').take(u64::try_from(MAX_INGEST_BYTES).unwrap_or(0) * 2);
        let mut req = json!({"scope": "mine"});
        let e = merge_stdin(&mut req, huge).err().map(|e| (e.code().to_owned(), e.details()["reason"].clone()));
        assert_eq!(e, Some(("BD_CACHE.INGEST_INVALID".to_owned(), json!("too_large"))));
        let mut req = json!({"scope": "mine"});
        assert!(merge_stdin(&mut req, &b"{\"rows\": [], \"blocked\": []}"[..]).is_ok());
        assert_eq!(req["rows"], json!([]));
    }
}
