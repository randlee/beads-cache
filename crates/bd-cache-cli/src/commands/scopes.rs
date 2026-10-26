//! `cache scopes add|set|remove|list|ingest|announce` (ADR-BDC-0013, ADR-BDC-0015,
//! REQ-BDC-0721, 0726, 0728): their argv rows, and the streamed stdin of `ingest`.
//!
//! They are `bd-cache-api` operations, run like the other management verbs.
//! `cache scopes` stays the read verb and is `scopes list`'s alias.
//! `scopes ingest` decodes its snapshot from stdin while it reads it
//! (`IngestSnapshot::from_reader`): the bytes are counted against
//! `MAX_INGEST_BYTES` and the rows against `MAX_INGEST_ROWS` as they
//! decode, so an input over either is refused (`INGEST_INVALID`) before it
//! is held whole, and nothing is encoded again. stdin holds a rows array or
//! `{"rows": [...], "blocked": [...]}` and nothing else: the scope, the
//! revision and `--allow-empty` come from the command line only (`BAD_ARGS`
//! for any other member).

use std::io::Read;

use bd_cache_api::{ApiError, Command, Request};
use bd_cache_model::IngestSnapshot;

use crate::args::{ArgSpec, CommandSpec, Shape, Ty};

use Shape::{Many, One};
use Ty::Str;

const SCOPE: ArgSpec = ArgSpec::positional(
    "scope",
    One,
    Str,
    true,
    "scope",
    "The scope's name, as written.",
);

/// The filter arguments of `add` and `set` (ADR-BDC-0013).
const FILTER: [ArgSpec; 6] = [
    ArgSpec::switch("all", "all", "Every bead (alone, without a criterion)."),
    ArgSpec::flag(
        "labels-all",
        "LABEL",
        Many,
        Str,
        false,
        "labels_all",
        "A label a member must carry (repeat: all of them).",
    ),
    ArgSpec::flag(
        "labels-any",
        "LABEL",
        Many,
        Str,
        false,
        "labels_any",
        "Labels of which a member carries one (repeat).",
    ),
    ArgSpec::flag(
        "status",
        "STATUS",
        Many,
        Str,
        false,
        "status",
        "A status a member may have (repeat).",
    ),
    ArgSpec::flag("type", "TYPE", One, Str, false, "type", "The issue type."),
    ArgSpec::flag(
        "assignee",
        "WHO",
        One,
        Str,
        false,
        "assignee",
        "The assignee.",
    ),
];

const ADD_ARGS: [ArgSpec; 7] = [
    SCOPE, FILTER[0], FILTER[1], FILTER[2], FILTER[3], FILTER[4], FILTER[5],
];

/// The `cache scopes` subcommands.
pub(crate) const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        path: "cache scopes add",
        about: "Register a scope: a name and a bd filter (all, or labels, status, type, assignee).",
        command: Command::ScopesAdd,
        args: &ADD_ARGS,
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes set",
        about: "Change a registered scope's filter; its memberships start again.",
        command: Command::ScopesSet,
        args: &ADD_ARGS,
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes remove",
        about: "cache evict, then remove the scope from the registry (PRUNE_BLOCKED as evict).",
        command: Command::ScopesRemove,
        args: &[SCOPE],
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes list",
        about: "Every scope with its filter and sync state; the same reply as cache scopes.",
        command: Command::ScopesList,
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes ingest",
        about: "Store a snapshot of a registered scope from stdin: {\"rows\": [...], \"blocked\": [...]} or a rows array.",
        command: Command::ScopesIngest,
        args: &[
            SCOPE,
            ArgSpec::flag(
                "revision",
                "R",
                One,
                Str,
                false,
                "revision",
                "An opaque revision stored with the snapshot.",
            ),
            ArgSpec::switch(
                "allow-empty",
                "allow_empty",
                "Accept a snapshot with no row.",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes announce",
        about: "Mark the scope's current failure notified when --since is its failing_since.",
        command: Command::ScopesAnnounce,
        args: &[
            SCOPE,
            ArgSpec::flag(
                "since",
                "TOKEN",
                One,
                Str,
                true,
                "since",
                "The failing_since token the hook was given.",
            ),
        ],
        one_of: None,
    },
];

/// `req` with the snapshot decoded from `input` (stdin) as it is read, when
/// it is `scopes ingest`; any other request as it is.
///
/// # Errors
///
/// `INGEST_INVALID` (`too_large`, `too_many_rows`, `row_invalid`,
/// `membership`) and `BAD_ARGS` (a member other than `rows` and `blocked`, a
/// read that fails), as `IngestSnapshot::from_reader` gives them.
pub(crate) fn with_stdin_snapshot(req: Request, input: impl Read) -> Result<Request, ApiError> {
    let Request::ScopesIngest(ingest) = req else {
        return Ok(req);
    };
    let snapshot =
        IngestSnapshot::from_reader(ingest.scope(), input).map_err(|e| ApiError::from(&e))?;
    Ok(Request::ScopesIngest(Box::new(
        (*ingest).with_snapshot(snapshot),
    )))
}

#[cfg(test)]
mod tests {
    use bd_cache_model::MAX_INGEST_BYTES;

    use super::*;
    use serde_json::json;

    #[expect(clippy::panic, reason = "test code: a failed decode is a test failure")]
    fn ingest() -> Request {
        bd_cache_api::decode(Command::ScopesIngest, &json!({"scope": "mine"}))
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn code(r: Result<Request, ApiError>) -> Option<(String, serde_json::Value)> {
        r.err().map(|e| {
            (
                e.code().to_owned(),
                e.details().get("reason").cloned().unwrap_or_default(),
            )
        })
    }

    /// Over 64 MiB of stdin is refused as `too_large` without holding it all
    /// (the read stops at the limit); a snapshot object that names another
    /// scope is `BAD_ARGS`, never ingested there (B8Q1-031). Planted
    /// negative: a small body is decoded into the request.
    #[test]
    #[expect(clippy::panic, reason = "test code: a failed decode is a test failure")]
    fn stdin_is_streamed_and_takes_only_rows_and_blocked() {
        let limit = u64::try_from(MAX_INGEST_BYTES).unwrap_or(0);
        let huge = std::io::repeat(b' ').take(limit * 2);
        assert_eq!(
            code(with_stdin_snapshot(ingest(), huge)),
            Some(("BD_CACHE.INGEST_INVALID".to_owned(), json!("too_large")))
        );
        let other = &br#"{"scope": "other", "rows": []}"#[..];
        assert_eq!(
            code(with_stdin_snapshot(ingest(), other)).map(|(c, _)| c),
            Some("BD_CACHE.BAD_ARGS".to_owned())
        );
        let ok = with_stdin_snapshot(ingest(), &b"{\"rows\": [], \"blocked\": []}"[..]);
        match ok {
            Ok(Request::ScopesIngest(r)) => {
                assert_eq!(r.scope().name(), "mine");
                assert!(r.snapshot().is_empty());
            }
            other => panic!("{other:?}"),
        }
    }
}
