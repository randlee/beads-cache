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
    pub(crate) id: BeadId,
    /// How long a miss may wait for bd; `0` answers from the cache only
    /// (`NOT_CACHED` on a miss); absent takes the configured miss timeout.
    pub(crate) miss_timeout_ms: Option<u64>,
}

impl IssuesGet {
    /// The bead.
    #[must_use]
    pub fn id(&self) -> &BeadId {
        &self.id
    }

    /// How long a miss may wait for bd; `0` answers from the cache only
    /// (`NOT_CACHED` on a miss); absent takes the configured miss timeout.
    #[must_use]
    pub fn miss_timeout_ms(&self) -> Option<u64> {
        self.miss_timeout_ms
    }
}

/// `issues list` (`/v0` listIssues), over one known scope.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct IssuesList {
    /// Statuses to keep; set, it replaces the default exclusions.
    pub(crate) status: Vec<String>,
    /// Labels a row must all carry; the first `phase-<x>` one selects the
    /// scope when `scope` is absent.
    pub(crate) label: Vec<String>,
    /// Labels a row must carry at least one of.
    pub(crate) label_any: Vec<String>,
    /// Labels a row must carry none of.
    pub(crate) exclude_label: Vec<String>,
    /// The assignee a row must have.
    pub(crate) assignee: Option<String>,
    /// Lifts the default exclusions (closed rows, templates, `gate` rows).
    pub(crate) all: bool,
    /// The order.
    #[schemars(with = "Option<String>")]
    pub(crate) sort: ListSort,
    /// The page size; absent or 0 is no limit inside a scope.
    pub(crate) limit: Option<u64>,
    /// The `next_cursor` of the previous page, decoded.
    #[schemars(with = "Option<String>")]
    pub(crate) cursor: Option<Cursor>,
    /// The scope (`phase-<x>` or `<x>`).
    #[schemars(with = "Option<String>")]
    pub(crate) scope: Option<ScopeId>,
}

impl IssuesList {
    /// Statuses to keep; set, it replaces the default exclusions.
    #[must_use]
    pub fn status(&self) -> &[String] {
        &self.status
    }

    /// Labels a row must all carry; the first `phase-<x>` one selects the
    /// scope when `scope` is absent.
    #[must_use]
    pub fn label(&self) -> &[String] {
        &self.label
    }

    /// Labels a row must carry at least one of.
    #[must_use]
    pub fn label_any(&self) -> &[String] {
        &self.label_any
    }

    /// Labels a row must carry none of.
    #[must_use]
    pub fn exclude_label(&self) -> &[String] {
        &self.exclude_label
    }

    /// The assignee a row must have.
    #[must_use]
    pub fn assignee(&self) -> Option<&str> {
        self.assignee.as_deref()
    }

    /// Lifts the default exclusions (closed rows, templates, `gate` rows).
    #[must_use]
    pub fn all(&self) -> bool {
        self.all
    }

    /// The order.
    #[must_use]
    pub fn sort(&self) -> ListSort {
        self.sort
    }

    /// The page size; absent or 0 is no limit inside a scope.
    #[must_use]
    pub fn limit(&self) -> Option<u64> {
        self.limit
    }

    /// The `next_cursor` of the previous page, decoded.
    #[must_use]
    pub fn cursor(&self) -> Option<&Cursor> {
        self.cursor.as_ref()
    }

    /// The scope (`phase-<x>` or `<x>`).
    #[must_use]
    pub fn scope(&self) -> Option<&ScopeId> {
        self.scope.as_ref()
    }
}

/// `ready [--fresh-within-ms N]` (`/v0` listReadyWork; membership follows the bd CLI).
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct Ready {
    /// Labels a row must all carry; the first `phase-<x>` one selects the scope.
    pub(crate) label: Vec<String>,
    /// Labels a row must carry at least one of.
    pub(crate) label_any: Vec<String>,
    /// Labels a row must carry none of.
    pub(crate) exclude_label: Vec<String>,
    /// The assignee a row must have.
    pub(crate) assignee: Option<String>,
    /// Keep only unassigned rows.
    pub(crate) unassigned: bool,
    /// The priority a row must have.
    pub(crate) priority: Option<i64>,
    /// At most this many rows; absent or 0 is no limit inside a scope.
    pub(crate) limit: Option<u64>,
    /// The scope (`phase-<x>` or `<x>`).
    #[schemars(with = "Option<String>")]
    pub(crate) scope: Option<ScopeId>,
    /// Ask bd for a fresh answer within this many milliseconds.
    pub(crate) fresh_within_ms: Option<u64>,
}

impl Ready {
    /// Labels a row must all carry; the first `phase-<x>` one selects the scope.
    #[must_use]
    pub fn label(&self) -> &[String] {
        &self.label
    }

    /// Labels a row must carry at least one of.
    #[must_use]
    pub fn label_any(&self) -> &[String] {
        &self.label_any
    }

    /// Labels a row must carry none of.
    #[must_use]
    pub fn exclude_label(&self) -> &[String] {
        &self.exclude_label
    }

    /// The assignee a row must have.
    #[must_use]
    pub fn assignee(&self) -> Option<&str> {
        self.assignee.as_deref()
    }

    /// Keep only unassigned rows.
    #[must_use]
    pub fn unassigned(&self) -> bool {
        self.unassigned
    }

    /// The priority a row must have.
    #[must_use]
    pub fn priority(&self) -> Option<i64> {
        self.priority
    }

    /// At most this many rows; absent or 0 is no limit inside a scope.
    #[must_use]
    pub fn limit(&self) -> Option<u64> {
        self.limit
    }

    /// The scope (`phase-<x>` or `<x>`).
    #[must_use]
    pub fn scope(&self) -> Option<&ScopeId> {
        self.scope.as_ref()
    }

    /// Ask bd for a fresh answer within this many milliseconds.
    #[must_use]
    pub fn fresh_within_ms(&self) -> Option<u64> {
        self.fresh_within_ms
    }
}

/// `dependencies list --issue-id ID… [--type T…]` (`/v0` listDependencies).
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct DependenciesList {
    /// The beads, in the order their edges are listed (nonempty).
    #[schemars(with = "Vec<String>")]
    pub(crate) issue_id: Vec<BeadId>,
    /// The edge types to keep; empty keeps every type (`type` on the wire).
    #[schemars(with = "Vec<String>")]
    pub(crate) types: Vec<DepType>,
}

impl DependenciesList {
    /// The beads, in the order their edges are listed (nonempty).
    #[must_use]
    pub fn issue_id(&self) -> &[BeadId] {
        &self.issue_id
    }

    /// The edge types to keep; empty keeps every type (`type` on the wire).
    #[must_use]
    pub fn types(&self) -> &[DepType] {
        &self.types
    }
}

/// The served `/v0` `metadata` patch: `{set?, unset?}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct MetadataPatch {
    /// Keys to set.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub(crate) set: Map<String, Value>,
    /// Keys to remove.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) unset: Vec<String>,
}

impl MetadataPatch {
    /// Keys to set.
    #[must_use]
    pub fn set(&self) -> &Map<String, Value> {
        &self.set
    }

    /// Keys to remove.
    #[must_use]
    pub fn unset(&self) -> &[String] {
        &self.unset
    }
}

/// The served `/v0` update patch (`IssuePatchBody`, the mapped members only).
///
/// Serializes with only the members it requests, so a decoded patch and the
/// `Entry.args` it is journaled as compare equal (contract §6 `Entry`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct IssuePatch {
    /// New status (never `closed`: that is `issues close`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    /// New assignee; `""` unassigns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) assignee: Option<String>,
    /// Labels to add.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) add_labels: Vec<String>,
    /// Labels to remove.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) remove_labels: Vec<String>,
    /// Text appended to the notes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) append_notes: Option<String>,
    /// Metadata keys to set and unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) metadata: Option<MetadataPatch>,
}

impl IssuePatch {
    /// New status (never `closed`: that is `issues close`).
    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// New assignee; `""` unassigns.
    #[must_use]
    pub fn assignee(&self) -> Option<&str> {
        self.assignee.as_deref()
    }

    /// Labels to add.
    #[must_use]
    pub fn add_labels(&self) -> &[String] {
        &self.add_labels
    }

    /// Labels to remove.
    #[must_use]
    pub fn remove_labels(&self) -> &[String] {
        &self.remove_labels
    }

    /// Text appended to the notes.
    #[must_use]
    pub fn append_notes(&self) -> Option<&str> {
        self.append_notes.as_deref()
    }

    /// Metadata keys to set and unset.
    #[must_use]
    pub fn metadata(&self) -> Option<&MetadataPatch> {
        self.metadata.as_ref()
    }
}

/// `issues update <id>` (`/v0` updateIssue), journaled.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct IssuesUpdate {
    /// The bead.
    #[schemars(with = "String")]
    pub(crate) id: BeadId,
    /// Who writes.
    #[schemars(with = "String")]
    pub(crate) actor: Actor,
    /// The caller's key; required, never derived (REQ-BDC-0004).
    #[schemars(with = "String")]
    pub(crate) idempotency_key: IdemKey,
    /// The change; nonempty, validated as the journal will parse it.
    pub(crate) patch: IssuePatch,
    /// Guard: the status the bead must have.
    pub(crate) expected_status: Option<String>,
    /// Guard: the assignee the bead must have (`""` = unassigned).
    pub(crate) expected_assignee: Option<String>,
}

impl IssuesUpdate {
    /// The bead.
    #[must_use]
    pub fn id(&self) -> &BeadId {
        &self.id
    }

    /// Who writes.
    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The caller's key; required, never derived (REQ-BDC-0004).
    #[must_use]
    pub fn idempotency_key(&self) -> &IdemKey {
        &self.idempotency_key
    }

    /// The change; nonempty, validated as the journal will parse it.
    #[must_use]
    pub fn patch(&self) -> &IssuePatch {
        &self.patch
    }

    /// Guard: the status the bead must have.
    #[must_use]
    pub fn expected_status(&self) -> Option<&str> {
        self.expected_status.as_deref()
    }

    /// Guard: the assignee the bead must have (`""` = unassigned).
    #[must_use]
    pub fn expected_assignee(&self) -> Option<&str> {
        self.expected_assignee.as_deref()
    }
}

/// `issues close <id>` (`/v0` closeIssue), journaled.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct IssuesClose {
    /// The bead.
    #[schemars(with = "String")]
    pub(crate) id: BeadId,
    /// Who writes.
    #[schemars(with = "String")]
    pub(crate) actor: Actor,
    /// The caller's key; required, never derived (REQ-BDC-0004).
    #[schemars(with = "String")]
    pub(crate) idempotency_key: IdemKey,
    /// The close reason.
    pub(crate) reason: Option<String>,
    /// Guard (cache extension): the status the bead must have.
    pub(crate) expected_status: Option<String>,
    /// Guard (cache extension): the assignee the bead must have.
    pub(crate) expected_assignee: Option<String>,
}

impl IssuesClose {
    /// The bead.
    #[must_use]
    pub fn id(&self) -> &BeadId {
        &self.id
    }

    /// Who writes.
    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The caller's key; required, never derived (REQ-BDC-0004).
    #[must_use]
    pub fn idempotency_key(&self) -> &IdemKey {
        &self.idempotency_key
    }

    /// The close reason.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    /// Guard (cache extension): the status the bead must have.
    #[must_use]
    pub fn expected_status(&self) -> Option<&str> {
        self.expected_status.as_deref()
    }

    /// Guard (cache extension): the assignee the bead must have.
    #[must_use]
    pub fn expected_assignee(&self) -> Option<&str> {
        self.expected_assignee.as_deref()
    }
}

/// `dependencies add <issue_id> <depends_on_id> --type T` (`/v0` addDependencies), journaled.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct DependenciesAdd {
    /// Who writes.
    #[schemars(with = "String")]
    pub(crate) actor: Actor,
    /// The caller's key; required, never derived (REQ-BDC-0004).
    #[schemars(with = "String")]
    pub(crate) idempotency_key: IdemKey,
    /// The one edge (`edges: [edge]` on the wire; exactly one).
    #[schemars(with = "Vec<Value>")]
    pub(crate) edge: Edge,
}

impl DependenciesAdd {
    /// Who writes.
    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The caller's key; required, never derived (REQ-BDC-0004).
    #[must_use]
    pub fn idempotency_key(&self) -> &IdemKey {
        &self.idempotency_key
    }

    /// The one edge (`edges: [edge]` on the wire; exactly one).
    #[must_use]
    pub fn edge(&self) -> &Edge {
        &self.edge
    }
}

/// `context` (`/v0` getContext): no members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[non_exhaustive]
pub struct Context;
