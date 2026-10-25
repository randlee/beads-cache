//! Surface (a) requests: the beads commands, in `/v0` names (contract §5.3).

use bd_cache_model::{IdemKey, ScopeId};
use sc_beads::{Actor, BeadId, DepType, Edge};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::page::{Cursor, ListSort};

/// `issues get <id> [--miss-timeout-ms N]` (`/v0` getIssue).
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct IssuesGet {
    /// The bead.
    #[schemars(with = "String")]
    pub id: BeadId,
    /// How long a miss may wait for bd; `0` answers from the cache only
    /// (`NOT_CACHED` on a miss); absent takes the configured miss timeout.
    pub miss_timeout_ms: Option<u64>,
}

/// `issues list` (`/v0` listIssues), over one known scope.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct IssuesList {
    /// Statuses to keep; set, it replaces the default exclusions.
    pub status: Vec<String>,
    /// Labels a row must all carry; the first `phase-<x>` one selects the
    /// scope when `scope` is absent.
    pub label: Vec<String>,
    /// Labels a row must carry at least one of.
    pub label_any: Vec<String>,
    /// Labels a row must carry none of.
    pub exclude_label: Vec<String>,
    /// The assignee a row must have.
    pub assignee: Option<String>,
    /// Lifts the default exclusions (closed rows, templates, `gate` rows).
    pub all: bool,
    /// The order.
    #[schemars(with = "Option<String>")]
    pub sort: ListSort,
    /// The page size; absent or 0 is no limit inside a scope.
    pub limit: Option<u64>,
    /// The `next_cursor` of the previous page, decoded.
    #[schemars(with = "Option<String>")]
    pub cursor: Option<Cursor>,
    /// The scope (`phase-<x>` or `<x>`).
    #[schemars(with = "Option<String>")]
    pub scope: Option<ScopeId>,
}

/// `ready [--fresh-within-ms N]` (`/v0` listReadyWork; membership follows the bd CLI).
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct Ready {
    /// Labels a row must all carry; the first `phase-<x>` one selects the scope.
    pub label: Vec<String>,
    /// Labels a row must carry at least one of.
    pub label_any: Vec<String>,
    /// Labels a row must carry none of.
    pub exclude_label: Vec<String>,
    /// The assignee a row must have.
    pub assignee: Option<String>,
    /// Keep only unassigned rows.
    pub unassigned: bool,
    /// The priority a row must have.
    pub priority: Option<i64>,
    /// At most this many rows; absent or 0 is no limit inside a scope.
    pub limit: Option<u64>,
    /// The scope (`phase-<x>` or `<x>`).
    #[schemars(with = "Option<String>")]
    pub scope: Option<ScopeId>,
    /// Ask bd for a fresh answer within this many milliseconds.
    pub fresh_within_ms: Option<u64>,
}

/// `dependencies list --issue-id ID… [--type T…]` (`/v0` listDependencies).
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct DependenciesList {
    /// The beads, in the order their edges are listed (nonempty).
    #[schemars(with = "Vec<String>")]
    pub issue_id: Vec<BeadId>,
    /// The edge types to keep; empty keeps every type (`type` on the wire).
    #[schemars(with = "Vec<String>")]
    pub types: Vec<DepType>,
}

/// The served `/v0` `metadata` patch: `{set?, unset?}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct MetadataPatch {
    /// Keys to set.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub set: Map<String, Value>,
    /// Keys to remove.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unset: Vec<String>,
}

/// The served `/v0` update patch (`IssuePatchBody`, the mapped members only).
///
/// Serializes with only the members it requests, so a decoded patch and the
/// `Entry.args` it is journaled as compare equal (contract §6 `Entry`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct IssuePatch {
    /// New status (never `closed`: that is `issues close`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// New assignee; `""` unassigns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    /// Labels to add.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add_labels: Vec<String>,
    /// Labels to remove.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove_labels: Vec<String>,
    /// Text appended to the notes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append_notes: Option<String>,
    /// Metadata keys to set and unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MetadataPatch>,
}

/// `issues update <id>` (`/v0` updateIssue), journaled.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct IssuesUpdate {
    /// The bead.
    #[schemars(with = "String")]
    pub id: BeadId,
    /// Who writes.
    #[schemars(with = "String")]
    pub actor: Actor,
    /// The caller's key; required, never derived (REQ-BDC-0004).
    #[schemars(with = "String")]
    pub idempotency_key: IdemKey,
    /// The change; nonempty, validated as the journal will parse it.
    pub patch: IssuePatch,
    /// Guard: the status the bead must have.
    pub expected_status: Option<String>,
    /// Guard: the assignee the bead must have (`""` = unassigned).
    pub expected_assignee: Option<String>,
}

/// `issues close <id>` (`/v0` closeIssue), journaled.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct IssuesClose {
    /// The bead.
    #[schemars(with = "String")]
    pub id: BeadId,
    /// Who writes.
    #[schemars(with = "String")]
    pub actor: Actor,
    /// The caller's key; required, never derived (REQ-BDC-0004).
    #[schemars(with = "String")]
    pub idempotency_key: IdemKey,
    /// The close reason.
    pub reason: Option<String>,
    /// Guard (cache extension): the status the bead must have.
    pub expected_status: Option<String>,
    /// Guard (cache extension): the assignee the bead must have.
    pub expected_assignee: Option<String>,
}

/// `dependencies add <issue_id> <depends_on_id> --type T` (`/v0` addDependencies), journaled.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct DependenciesAdd {
    /// Who writes.
    #[schemars(with = "String")]
    pub actor: Actor,
    /// The caller's key; required, never derived (REQ-BDC-0004).
    #[schemars(with = "String")]
    pub idempotency_key: IdemKey,
    /// The one edge (`edges: [edge]` on the wire; exactly one).
    #[schemars(with = "Vec<Value>")]
    pub edge: Edge,
}

/// `context` (`/v0` getContext): no members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct Context;
