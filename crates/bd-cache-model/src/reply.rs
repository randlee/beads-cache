//! Reply and report shapes of the cache verbs.
//!
//! Provisional: the exact field set is owned by the later `bdc` sprint
//! (REQ-BDC-0806, deferred). Rows are objects, never tuples. The goldens in
//! `tests/goldens.rs` pin these Rust shapes so wave-2 sprints agree; they are
//! not claimed equal to the Python replies.

use sc_beads::{Actor, Bead, BeadId};
use serde::Serialize;

use crate::journal::Seq;
use crate::scope::ScopeId;
use crate::state::{Outage, StoreStatus};

/// A bead as a read returns it: the view, its freshness and its blocked state.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RowView {
    /// The view (stored row with pending entries replayed).
    pub bead: Bead,
    /// True when the stored row is stale (a read-back failed).
    pub stale: bool,
    /// Blockers: `None` = unknown, `Some([])` = not blocked.
    pub blocked: Option<Vec<BeadId>>,
}

/// `get`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GetReply {
    /// The row.
    pub row: RowView,
    /// The bead's scope's `last_ok_at`, when it has one.
    pub scope_last_ok_at: Option<String>,
    /// Pending entries replayed into the view.
    pub pending: u32,
    /// The outage, while one holds.
    pub outage: Option<Outage>,
}

/// `list`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListReply {
    /// The scope listed.
    pub scope: ScopeId,
    /// The scope's `last_ok_at`.
    pub last_ok_at: Option<String>,
    /// The rows, by id.
    pub rows: Vec<RowView>,
    /// The outage, while one holds.
    pub outage: Option<Outage>,
}

/// What a `ready` answer is based on (ADR-BDC-0002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReadyBasis {
    /// bd answered within the caller's limit.
    Fresh,
    /// The stored base.
    Cached {
        /// When the base was fetched.
        fetched_at: String,
        /// True when a later refresh failed.
        stale: bool,
    },
    /// No base yet (cold cache).
    Unknown,
}

/// `ready`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadyReply {
    /// The scope.
    pub scope: ScopeId,
    /// The basis.
    pub basis: ReadyBasis,
    /// True when journaled writes adjusted the base.
    pub locally_adjusted: bool,
    /// Ready beads.
    pub ready: Vec<RowView>,
    /// Why a requested fresh answer fell back to the cached one.
    pub read_fallback: Option<String>,
    /// Ready per bd but no cached row yet; omitted from `ready`.
    pub missing_rows: Vec<BeadId>,
}

/// `write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WriteReply {
    /// The entry.
    pub seq: Seq,
    /// True when the idempotency key already journaled this write.
    pub duplicate: bool,
}

/// One escalated entry in a drain report (REQ-BDC-0406).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EscalatedItem {
    /// The entry.
    pub seq: Seq,
    /// The bead.
    pub bead: BeadId,
    /// The op name.
    pub op: String,
    /// Who wrote it.
    pub actor: Actor,
    /// The entry's phase.
    pub phase: Option<String>,
    /// Why it escalated.
    pub reason: String,
}

/// `drain` (REQ-BDC-0406).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DrainReport {
    /// The drain lock was held elsewhere; nothing ran.
    pub busy: bool,
    /// Entries applied.
    pub applied: Vec<Seq>,
    /// Entries that failed and stay pending.
    pub retrying: Vec<Seq>,
    /// Entries escalated.
    pub escalated: Vec<EscalatedItem>,
    /// Entries not started by the start gate (or stopped by an outage).
    pub deferred: Vec<Seq>,
    /// Entries whose call the hard deadline cut.
    pub deferred_in_flight: Vec<Seq>,
    /// The outage, when the backend was unreachable.
    pub outage: Option<Outage>,
}

/// `sync`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncReport {
    /// The scope.
    pub scope: ScopeId,
    /// The backend check rebuilt the cache.
    pub rebuilt: bool,
    /// The live schema equals the pinned one.
    pub model_current: bool,
    /// Rows stored.
    pub stored: u32,
    /// Blocked rows in the refreshed base.
    pub blocked: u32,
    /// The sync did not run (deadline).
    pub deferred: bool,
    /// The outage, when the backend was unreachable.
    pub outage: Option<Outage>,
}

/// `prune`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PruneReport {
    /// The scope.
    pub scope: ScopeId,
    /// Beads deleted (held by no other scope).
    pub beads_deleted: u32,
    /// Out-of-scope rows swept (no scope, no open entry).
    pub out_of_scope_swept: u32,
}

/// `status`: the store's status, which carries the outage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusReply {
    /// The store's status, flattened into the reply.
    #[serde(flatten)]
    pub store: StoreStatus,
}
