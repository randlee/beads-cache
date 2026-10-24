//! The store port ([`CacheStore`]) and the cross-process drain lock ([`DrainLock`]).

use sc_beads::{Actor, Bead, BeadId, BlockedRow, Edge, Fingerprint};

use crate::error::CacheError;
use crate::journal::{IdemKey, JournalEntry, NewEntry, Seq};
use crate::reply::PruneReport;
use crate::scope::ScopeId;
use crate::state::{
    FingerprintOutcome, Freshness, OpenWork, Outage, ReadyBase, StoreStatus, SyncState,
    UpsertReport,
};

/// The cache's store; a-4 implements it over SQLite, `testing::FakeStore` in memory.
///
/// Rules (checked by the contract suite, `testing::contract`):
///
/// - `get_row(upsert(b)) == normalize(b)`: `dependencies` rebuilt from the
///   edges table, counts absent ([`normalize`](crate::normalize)).
/// - upsert writes the bead's outgoing edges from `Bead.dependencies`,
///   replacing the old ones.
/// - any successful store of a bead clears `stale`.
/// - `upsert_rows(Some(s), ..)` sets membership of `s` from the row's labels
///   (added when the row carries `s`'s label, removed when it does not);
///   `upsert_rows(None, ..)` never changes existing membership (a new row
///   stored with `None` has none: an out-of-scope row).
/// - unforced upserts skip a row whose stored `updated_at` is newer
///   (`skipped_newer`); forced upserts always store.
/// - `status_changed` lists existing rows whose status text changed, forced
///   or not; new rows excluded.
/// - `apply_with_readback(seq, None, Some(err))` marks the entry applied with
///   `err` and sets stale on the bead; with `Some(row)` it stores the row
///   forced (membership unchanged) and marks the entry applied, in one
///   transaction. Both set `applied_at`.
/// - `record_fingerprint` `Rebuilt` deletes beads, edges, memberships,
///   `sync_state` rows and every ready base; keeps the journal (and the
///   scope registry, alerts and outage). The first fingerprint is
///   `Recorded` and drops nothing.
/// - `ensure_scope` persists; `scopes()` returns configured and on-demand
///   scopes across restarts.
/// - `replace_ready_base` replaces the whole base for the scope and clears
///   its stale mark; its `at` is the instant taken before the sync's first
///   `list_blocked` / `list_ready` call. `ready_base` is `None` only when the
///   scope never stored one; `mark_ready_base_stale` on a missing base is a
///   no-op; `prune_scope` deletes it.
/// - `applied_since(at)` returns entries applied at or after `at`, in seq order.
/// - journal reads (`pending_entries`, `pending_for`, `unannounced_escalations`)
///   are in seq order; `insert_entry` on a used key is `IDEM_CONFLICT`;
///   `escalate` does not change `attempts`; `resolve` on an entry that is
///   not escalated is `NOT_ESCALATED`, else it appends
///   `" | resolved by <actor>: <note>"` to `last_error`.
/// - `prune_scope` is `PRUNE_BLOCKED` while the scope has a pending or
///   escalated entry; otherwise it deletes the scope's beads that no other
///   scope holds (with their edges), the scope's journal entries, sync state
///   and ready base, and sweeps out-of-scope rows with no open entry. The
///   scope stays registered.
/// - an unknown `seq` is `NOT_FOUND`.
///
/// Timestamps are RFC 3339 UTC strings supplied by the caller (`at`), so a
/// store reads no clock.
///
/// # Errors
///
/// Every method returns a [`CacheError`]: the codes named above, or the
/// implementation's own storage failures.
#[expect(
    clippy::missing_errors_doc,
    reason = "error rules are documented once on the trait"
)]
pub trait CacheStore: Send {
    /// Registers a scope (configured, or on demand by a sync); idempotent and persistent.
    fn ensure_scope(&mut self, s: &ScopeId) -> Result<(), CacheError>;
    /// Every registered scope.
    fn scopes(&self) -> Result<Vec<ScopeId>, CacheError>;
    /// The stored row and its freshness.
    fn get_row(&self, id: &BeadId) -> Result<Option<(Bead, Freshness)>, CacheError>;
    /// Rows the scope holds, by id.
    fn rows_in_scope(&self, s: &ScopeId) -> Result<Vec<(Bead, Freshness)>, CacheError>;
    /// Stores validated rows (see the rules above).
    fn upsert_rows(
        &mut self,
        s: Option<&ScopeId>,
        rows: &[Bead],
        force: bool,
    ) -> Result<UpsertReport, CacheError>;
    /// Ids of stale rows.
    fn stale_rows(&self) -> Result<Vec<BeadId>, CacheError>;
    /// Incoming edges of a bead, with kind (the edges index, REQ-BDC-0109).
    fn dependents_of(&self, id: &BeadId) -> Result<Vec<Edge>, CacheError>;
    /// Replaces the scope's whole ready base.
    fn replace_ready_base(
        &mut self,
        s: &ScopeId,
        blocked: &[BlockedRow],
        ready: &[BeadId],
        at: &str,
    ) -> Result<(), CacheError>;
    /// The scope's ready base; `None` = never fetched.
    fn ready_base(&self, s: &ScopeId) -> Result<Option<ReadyBase>, CacheError>;
    /// Marks the scope's ready base stale.
    fn mark_ready_base_stale(&mut self, s: &ScopeId) -> Result<(), CacheError>;

    /// The entry holding a key, in any state.
    fn entry_by_key(&self, k: &IdemKey) -> Result<Option<JournalEntry>, CacheError>;
    /// The entry with a seq, in any state.
    fn entry_by_seq(&self, seq: Seq) -> Result<Option<JournalEntry>, CacheError>;
    /// Journals a write as `Pending`; `at` is its `created_at`.
    fn insert_entry(&mut self, e: NewEntry, at: &str) -> Result<Seq, CacheError>;
    /// Every pending entry, by seq.
    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError>;
    /// A bead's pending entries, by seq.
    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError>;
    /// Entries applied at or after `at`, by seq.
    fn applied_since(&self, at: &str) -> Result<Vec<JournalEntry>, CacheError>;
    /// Marks an entry applied, storing the read-back row or marking the bead stale; one transaction.
    fn apply_with_readback(
        &mut self,
        seq: Seq,
        row: Option<&Bead>,
        err: Option<&str>,
        at: &str,
    ) -> Result<(), CacheError>;
    /// Counts one failed attempt with its error; returns the new count.
    fn record_attempt(&mut self, seq: Seq, err: &str, at: &str) -> Result<u32, CacheError>;
    /// Escalates an entry with its reason.
    fn escalate(&mut self, seq: Seq, reason: &str, at: &str) -> Result<(), CacheError>;
    /// Resolves an escalated entry; `NOT_ESCALATED` otherwise.
    fn resolve(&mut self, seq: Seq, note: &str, actor: &Actor, at: &str) -> Result<(), CacheError>;
    /// Escalated entries not yet announced, by seq.
    fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError>;
    /// Records that an escalation was announced.
    fn mark_announced(&mut self, seq: Seq, at: &str) -> Result<(), CacheError>;
    /// Applies the rebuild rule to a fresh fingerprint (REQ-BDC-0105).
    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError>;
    /// A scope's sync state (all `None` before its first sync).
    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError>;
    /// Stores a scope's sync state.
    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError>;
    /// The scope's last announced sync-failure cause (stable cause key).
    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError>;
    /// Sets (`Some`) or clears (`None`) the scope's sync alert.
    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError>;
    /// The outage, while one holds.
    fn outage(&self) -> Result<Option<Outage>, CacheError>;
    /// Records (`Some`) or clears (`None`) the outage.
    fn set_outage(&mut self, o: Option<&Outage>) -> Result<(), CacheError>;
    /// Prunes a scope (see the rules above).
    fn prune_scope(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError>;
    /// Open work for the stop rule.
    fn open_work(&self) -> Result<OpenWork, CacheError>;
    /// Status: meta, journal counts, scopes, escalations, outage.
    fn status(&self) -> Result<StoreStatus, CacheError>;
}

/// Cross-process single drain writer (a-4: `flock` on `<cache file>.drain.lock`).
pub trait DrainLock: Send + Sync {
    /// Takes the lock without blocking; `None` when another holder has it.
    ///
    /// # Errors
    ///
    /// The implementation's own failure to open or lock its lock file.
    fn try_acquire(&self) -> Result<Option<Box<dyn DrainGuard>>, CacheError>;
}

/// Holds the drain lock; dropping it releases the lock.
pub trait DrainGuard: Send {}
