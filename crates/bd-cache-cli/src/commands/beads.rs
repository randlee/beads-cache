//! Surface (a), the beads commands (contract §5): their argv rows, and the
//! `/v0` operations the cache does not serve (§5.4).
//!
//! Each row names the `/v0` request member every argument fills; the request
//! goes to `bd-cache-api`'s decoder unchanged, so a flag cannot mean anything
//! the decoder does not check.

use bd_cache_api::{ApiError, Command, Unsupported};

use crate::args::{ArgSpec, CommandSpec, Shape, Target, Ty};

use Shape::{List, Many, One};
use Ty::{Int, Json, Str};

const LABELS: [ArgSpec; 3] = [
    ArgSpec::flag(
        "label",
        "L",
        Many,
        Str,
        false,
        "label",
        "A label every row carries; the first phase-<x> one selects the scope.",
    ),
    ArgSpec::flag(
        "label-any",
        "L",
        Many,
        Str,
        false,
        "label_any",
        "A row carries at least one of these labels.",
    ),
    ArgSpec::flag(
        "exclude-label",
        "L",
        Many,
        Str,
        false,
        "exclude_label",
        "A row carries none of these labels.",
    ),
];

const SCOPE: ArgSpec = ArgSpec::flag(
    "scope",
    "S",
    One,
    Str,
    false,
    "scope",
    "The scope (phase-<x> or <x>); a list-like read needs one.",
);

const ACTOR: ArgSpec = ArgSpec::flag("actor", "A", One, Str, true, "actor", "Who writes.");

const IDEMPOTENCY_KEY: ArgSpec = ArgSpec::flag(
    "idempotency-key",
    "K",
    One,
    Str,
    true,
    "idempotency_key",
    "Names this one write; reuse it only to retry it (never derived).",
);

const EXPECTED: [ArgSpec; 2] = [
    ArgSpec::flag(
        "expected-status",
        "S",
        One,
        Str,
        false,
        "expected_status",
        "Guard: the bead's status, checked when journaled and again at flush.",
    ),
    ArgSpec::flag(
        "expected-assignee",
        "A",
        One,
        Str,
        false,
        "expected_assignee",
        "Guard: the bead's assignee, checked when journaled and again at flush.",
    ),
];

const ISSUE_ID: ArgSpec = ArgSpec::positional("id", One, Str, true, "id", "The bead.");

/// The surface (a) commands, in contract order (§5.2).
pub(crate) const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        path: "issues get",
        about: "One bead: its row from the cache, with x_cache (getIssue).",
        target: Target::Api(Command::IssuesGet),
        args: &[
            ISSUE_ID,
            ArgSpec::flag(
                "miss-timeout-ms",
                "N",
                One,
                Int,
                false,
                "miss_timeout_ms",
                "How long a miss may wait for bd; 0 answers from the cache only.",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "issues list",
        about: "The rows of one scope, filtered, ordered and paged (listIssues).",
        target: Target::Api(Command::IssuesList),
        args: &[
            ArgSpec::flag(
                "status",
                "S",
                Many,
                Str,
                false,
                "status",
                "A status to keep; replaces the default exclusions.",
            ),
            LABELS[0],
            LABELS[1],
            LABELS[2],
            ArgSpec::flag(
                "assignee",
                "A",
                One,
                Str,
                false,
                "assignee",
                "The assignee a row has.",
            ),
            ArgSpec::switch(
                "all",
                "all",
                "Lift the default exclusions (closed rows, templates, gate rows).",
            ),
            ArgSpec::flag(
                "sort",
                "created|priority",
                One,
                Str,
                false,
                "sort",
                "The order (default created).",
            ),
            ArgSpec::flag(
                "limit",
                "N",
                One,
                Int,
                false,
                "limit",
                "The page size; absent or 0 is no limit inside a scope.",
            ),
            ArgSpec::flag(
                "cursor",
                "C",
                One,
                Str,
                false,
                "cursor",
                "The next_cursor of the previous page.",
            ),
            SCOPE,
        ],
        one_of: None,
    },
    CommandSpec {
        path: "issues update",
        about: "Journal an update of one bead (updateIssue).",
        target: Target::Api(Command::IssuesUpdate),
        args: &[
            ISSUE_ID,
            ACTOR,
            IDEMPOTENCY_KEY,
            ArgSpec::flag(
                "patch",
                "JSON",
                One,
                Json,
                false,
                "patch",
                "The whole /v0 patch as JSON; the patch flags below add to it.",
            ),
            ArgSpec::flag(
                "status",
                "S",
                One,
                Str,
                false,
                "patch.status",
                "The new status (closed is refused: use issues close).",
            ),
            ArgSpec::flag(
                "assignee",
                "A",
                One,
                Str,
                false,
                "patch.assignee",
                "The new assignee; an empty value unassigns.",
            ),
            ArgSpec::flag(
                "add-label",
                "L",
                Many,
                Str,
                false,
                "patch.add_labels",
                "A label to add.",
            ),
            ArgSpec::flag(
                "remove-label",
                "L",
                Many,
                Str,
                false,
                "patch.remove_labels",
                "A label to remove.",
            ),
            ArgSpec::flag(
                "append-notes",
                "T",
                One,
                Str,
                false,
                "patch.append_notes",
                "Text appended to the notes.",
            ),
            ArgSpec::flag(
                "set-metadata",
                "JSON",
                One,
                Json,
                false,
                "patch.metadata.set",
                "Metadata keys to set, as a JSON object.",
            ),
            ArgSpec::flag(
                "unset-metadata",
                "KEY",
                Many,
                Str,
                false,
                "patch.metadata.unset",
                "A metadata key to remove.",
            ),
            EXPECTED[0],
            EXPECTED[1],
        ],
        one_of: None,
    },
    CommandSpec {
        path: "issues close",
        about: "Journal the close of one bead (closeIssue).",
        target: Target::Api(Command::IssuesClose),
        args: &[
            ISSUE_ID,
            ACTOR,
            IDEMPOTENCY_KEY,
            ArgSpec::flag(
                "reason",
                "T",
                One,
                Str,
                false,
                "reason",
                "The close reason.",
            ),
            EXPECTED[0],
            EXPECTED[1],
        ],
        one_of: None,
    },
    CommandSpec {
        path: "ready",
        about: "The ready beads of one scope, as the bd CLI counts them (listReadyWork).",
        target: Target::Api(Command::Ready),
        args: &[
            LABELS[0],
            LABELS[1],
            LABELS[2],
            ArgSpec::flag(
                "assignee",
                "A",
                One,
                Str,
                false,
                "assignee",
                "The assignee a row has.",
            ),
            ArgSpec::switch("unassigned", "unassigned", "Only unassigned rows."),
            ArgSpec::flag(
                "priority",
                "N",
                One,
                Int,
                false,
                "priority",
                "The priority a row has.",
            ),
            ArgSpec::flag(
                "limit",
                "N",
                One,
                Int,
                false,
                "limit",
                "Caps the list; absent is no limit inside a scope.",
            ),
            ArgSpec::flag(
                "sort",
                "priority",
                One,
                Str,
                false,
                "sort",
                "The order; only priority is served.",
            ),
            SCOPE,
            ArgSpec::flag(
                "fresh-within-ms",
                "N",
                One,
                Int,
                false,
                "fresh_within_ms",
                "Ask bd for a fresh answer, waiting at most N ms; the cached one otherwise.",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "dependencies list",
        about: "The cached dependency edges of some beads (listDependencies).",
        target: Target::Api(Command::DependenciesList),
        args: &[
            ArgSpec::flag(
                "issue-id",
                "ID",
                List,
                Str,
                true,
                "issue_id",
                "The beads whose edges to list, in this order.",
            ),
            ArgSpec::flag(
                "type",
                "T",
                List,
                Str,
                false,
                "type",
                "Only edges of these types.",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "dependencies add",
        about: "Journal one dependency edge (addDependencies).",
        target: Target::Api(Command::DependenciesAdd),
        args: &[
            ArgSpec::positional(
                "issue_id",
                One,
                Str,
                true,
                "edges.0.issue_id",
                "The depending bead.",
            ),
            ArgSpec::positional(
                "depends_on_id",
                One,
                Str,
                true,
                "edges.0.depends_on_id",
                "The bead depended on.",
            ),
            ArgSpec::flag(
                "type",
                "T",
                One,
                Str,
                true,
                "edges.0.type",
                "The dependency type (blocks, related, ...).",
            ),
            ACTOR,
            IDEMPOTENCY_KEY,
        ],
        one_of: None,
    },
    CommandSpec {
        path: "context",
        about: "The project, the cache file and the served capabilities (getContext).",
        target: Target::Api(Command::Context),
        args: &[],
        one_of: None,
    },
];

/// The `/v0` operations the cache does not serve, by the command path a
/// client would type, with the operation's name (contract §5.4, §5.5).
///
/// Each answers `UNSUPPORTED` (`details.unsupported: "operation"`), exit 3,
/// never `USAGE`: the command is well formed, the cache does not serve it.
pub(crate) const UNSERVED: &[(&str, &str)] = &[
    ("issues create", "createIssue"),
    ("issues batch-create", "batchCreate"),
    ("issues delete", "delete"),
    ("issues sweep", "sweep"),
    ("issues claim", "claim"),
    ("issues release", "release"),
    ("issues claim-next", "claimNext"),
    ("issues reopen", "reopen"),
    ("issues cas-metadata", "casMetadata"),
    ("issues batch-apply", "batchApply"),
    ("issues batch-close", "batchClose"),
    ("issues query", "issues:query"),
    ("issues count", "issues:count"),
    ("issues batch-get", "issues:batchGet"),
    ("ready count", "ready:count"),
    ("dependencies remove", "dependencies:remove"),
    ("dependencies tree", "dependencies:tree"),
    ("dependencies cycles", "dependencies:cycles"),
    ("dependencies count", "dependencies:count"),
    ("dependencies blocking", "dependencies:blocking"),
    ("blocked", "listBlockedIssues"),
    ("stats", "stats"),
    ("related", "related"),
    ("comments", "comments"),
    ("config", "config"),
    ("memories", "memories"),
    ("events", "events"),
    ("healthz", "healthz"),
];

/// `UNSUPPORTED` for leading command `words` that name an operation the cache
/// does not serve; `None` for any other words.
pub(crate) fn unserved(words: &[&str]) -> Option<ApiError> {
    UNSERVED.iter().find_map(|(path, operation)| {
        let n = path.split(' ').count();
        let typed = words.get(..n)?.join(" ");
        (typed == *path).then(|| {
            ApiError::unsupported(Unsupported::Operation, operation).with_detail("command", *path)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unserved_operation_is_unsupported_and_a_served_one_is_not() {
        let e = unserved(&["issues", "create", "x"]).map(|e| e.details().clone());
        assert_eq!(
            e.as_ref()
                .and_then(|d| d.get("name"))
                .and_then(|v| v.as_str()),
            Some("createIssue")
        );
        assert!(unserved(&["issues", "get", "create"]).is_none());
        assert!(unserved(&["ready"]).is_none());
        assert!(unserved(&["ready", "count"]).is_some());
    }
}
