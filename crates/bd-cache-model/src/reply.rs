//! Reply and report shapes of the cache verbs.
//!
//! Provisional: the exact field set is owned by the later `bdc` sprint
//! (REQ-BDC-0806, deferred). Rows are objects, never tuples. The goldens in
//! `tests/goldens.rs` pin these Rust shapes so wave-2 sprints agree; they are
//! not claimed equal to the Python replies.

use std::collections::BTreeMap;

use sc_beads::{Actor, Bead, BeadId, Timestamp};
use serde::Serialize;

use crate::journal::{JournalEntry, JournalState, Seq};
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
    pub scope_last_ok_at: Option<Timestamp>,
    /// Pending entries replayed into the view.
    pub pending: u32,
    /// The outage, while one holds.
    pub outage: Option<Outage>,
    /// The bead's scope: its phase when that scope is known, else the first
    /// known scope that holds it; `None` for a row outside every scope.
    ///
    /// Not serialized: the 0.1 Python `get` dict keeps its key set (REQ-BDC-0910).
    #[serde(skip)]
    pub scope: Option<ScopeId>,
}

/// `list`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListReply {
    /// The scope listed.
    pub scope: ScopeId,
    /// The scope's `last_ok_at`.
    pub last_ok_at: Option<Timestamp>,
    /// The rows, by id.
    pub rows: Vec<RowView>,
    /// The outage, while one holds.
    pub outage: Option<Outage>,
}

/// What a `ready` answer is based on (ADR-BDC-0002): every basis but
/// `Unknown` says when bd's answer was fetched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReadyBasis {
    /// bd answered within the caller's limit.
    Fresh {
        /// When bd's answer was fetched: the instant taken before the first
        /// `list_blocked` / `list_ready` call of this answer (a-1 amendment 2).
        fetched_at: Timestamp,
    },
    /// The stored base.
    Cached {
        /// When the base was fetched.
        fetched_at: Timestamp,
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
    /// The scope's last successful sync, as on every read reply
    /// (ADR-BDC-0005; a-1 amendment 2).
    pub last_ok_at: Option<Timestamp>,
    /// The basis.
    pub basis: ReadyBasis,
    /// True when journaled writes adjusted the base.
    pub locally_adjusted: bool,
    /// Ready beads.
    pub ready: Vec<RowView>,
    /// Why a requested fresh answer fell back to the cached one.
    pub read_fallback: Option<String>,
    /// The fallback's cause as data: a cut, bd not reached
    /// ([`CacheError::outage`](crate::CacheError::outage), REQ-BDC-0729), or
    /// an answered failure; `None` with no fallback. A front end classifies
    /// the fallback by this, never by `read_fallback`'s wording. Not part of
    /// the 0.1 dict.
    #[serde(skip)]
    pub read_fallback_cause: Option<FallbackCause>,
    /// Ready per bd but no cached row yet; omitted from `ready`.
    pub missing_rows: Vec<BeadId>,
}

/// Why a requested fresh `ready` fell back to the cached answer
/// ([`ReadyReply::read_fallback_cause`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FallbackCause {
    /// The caller's `fresh_within` ran out before bd answered (a cut).
    Cut,
    /// bd could not be reached (an outage error).
    Unreachable,
    /// bd answered with a failure.
    Answered,
}

/// `write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WriteReply {
    /// The entry.
    pub seq: Seq,
    /// True when the idempotency key already journaled this write.
    pub duplicate: bool,
}

/// `write_returning`: the journaled entry and the bead's views around it (REQ-BDC-0809).
///
/// Everything a front end needs to answer a journaled write (contract §5.3,
/// `issues update` / `issues close` / `dependencies add`) without a second
/// call. Not serialized: `bd-cache-api` maps it into its own data model.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteOutcome {
    /// The entry as stored after this call: the new entry, or on a duplicate
    /// the entry the key already journals.
    pub entry: JournalEntry,
    /// The bead's view before the write. On a duplicate, the current view.
    pub before: RowView,
    /// The bead's view after the write, as a `get` right after it answers
    /// (contract §5.3 `issue`). On a duplicate, the current view.
    pub after: RowView,
    /// True when the idempotency key already journaled this write.
    pub duplicate: bool,
    /// Whether the write changes the view (for a close, `!already_closed`),
    /// stored with the entry; on a duplicate, the stored value (lead ruling S68).
    ///
    /// `Some(stored)` when the entry stores it (every entry this core
    /// journals); for an entry stored without it (by the v3 Python writer, or before
    /// the column existed), `Some(predicted)` while it is pending, and `None`
    /// (unknown) in any other state, never a guess.
    pub changed: Option<bool>,
}

/// Journal entry counts by state (contract §6, `cache status` `journal`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
pub struct StateCounts {
    /// Entries waiting for the drain.
    pub pending: u64,
    /// Entries landed in bd.
    pub applied: u64,
    /// Entries sent to the phase lead.
    pub escalated: u64,
    /// Entries settled by hand.
    pub resolved: u64,
}

impl StateCounts {
    /// The count of one state.
    #[must_use]
    pub fn get(&self, state: JournalState) -> u64 {
        match state {
            JournalState::Pending => self.pending,
            JournalState::Applied => self.applied,
            JournalState::Escalated => self.escalated,
            JournalState::Resolved => self.resolved,
        }
    }

    /// Adds `n` entries in `state`.
    pub fn add(&mut self, state: JournalState, n: u64) {
        let slot = match state {
            JournalState::Pending => &mut self.pending,
            JournalState::Applied => &mut self.applied,
            JournalState::Escalated => &mut self.escalated,
            JournalState::Resolved => &mut self.resolved,
        };
        *slot = slot.saturating_add(n);
    }

    /// Entries in every state.
    #[must_use]
    pub fn sum(&self) -> u64 {
        self.pending
            .saturating_add(self.applied)
            .saturating_add(self.escalated)
            .saturating_add(self.resolved)
    }
}

/// `journal_counts`: the journal counted by state, in total and per scope.
///
/// Every entry is counted once in `total` and once in either its scope's
/// bucket (`by_scope`) or `unscoped`, so `total` is the sum of the buckets.
/// A scope with no entry has no bucket.
///
/// # Examples
///
/// ```
/// use bd_cache_model::{JournalCounts, JournalState, ScopeId};
/// let x = ScopeId::new("x").unwrap();
/// let mut c = JournalCounts::default();
/// c.add(Some(&x), JournalState::Pending, 2);
/// c.add(None, JournalState::Escalated, 1);
/// assert_eq!(c.total.pending, 2);
/// assert_eq!(c.total.sum(), 3);
/// assert_eq!(c.by_scope[&x].pending, 2);
/// assert_eq!(c.unscoped.escalated, 1);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct JournalCounts {
    /// Every entry.
    pub total: StateCounts,
    /// The entries of each scope (the entry's phase at write time).
    pub by_scope: BTreeMap<ScopeId, StateCounts>,
    /// The entries journaled with no scope.
    pub unscoped: StateCounts,
}

impl JournalCounts {
    /// Counts `n` entries of `scope` (`None` = unscoped) in `state`.
    pub fn add(&mut self, scope: Option<&ScopeId>, state: JournalState, n: u64) {
        self.total.add(state, n);
        match scope {
            Some(s) => self.by_scope.entry(s.clone()).or_default().add(state, n),
            None => self.unscoped.add(state, n),
        }
    }
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
    /// The entry's scope (`phase` in the 0.1 wire form).
    pub phase: Option<ScopeId>,
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
