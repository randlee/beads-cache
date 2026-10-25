//! The store port ([`CacheStore`]) and the cross-process drain lock ([`DrainLock`]).

use sc_beads::{Actor, Bead, BeadId, BlockedRow, Edge, Fingerprint, Timestamp};

use crate::error::CacheError;
use crate::journal::JournalState;
use crate::journal::{IdemKey, JournalEntry, NewEntry, Seq};
use crate::reply::{JournalCounts, PruneReport};
use crate::scope::{Scope, ScopeId};
use crate::state::{
    BaseEntry, FingerprintOutcome, Freshness, OpenWork, Outage, ReadyBase, StoreStatus, SyncState,
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
/// - `upsert_rows(Some(s), ..)` sets membership of `s` from the scope's
///   filter (added when [`Scope::holds`] the row, removed when it does not)
///   and records `s`'s name as the row's v3 `phase` while `s` holds it;
///   `upsert_rows(None, ..)` never changes existing membership (a new row
///   stored with `None` has none: an out-of-scope row).
/// - `ingest_scope(s, rows, blocked, ready, at, revision)` does in one
///   transaction what a passing sync of `s` stores: the rows as
///   `upsert_rows(Some(s), rows, false)` (each stored row's `revision` set to
///   `revision`), the ready base as `replace_ready_base(s, blocked, ready,
///   at)`, and `s`'s sync state with `last_started_at = last_ok_at = at`
///   and no error or failure. When any step fails nothing is stored.
/// - `record_sync` stores every field of the state, `failing_since` and
///   `notified_since` included, and `sync_state` reads them back.
/// - `announce_scope(s, since)` sets `notified_since = since` and answers
///   `true` only while `failing_since` equals `since` (compare-and-set);
///   otherwise it changes nothing and answers `false`.
/// - unforced upserts skip a row whose stored `updated_at` is a later
///   instant (`skipped_newer`); `updated_at` is compared as an RFC 3339
///   instant at second precision, never as text (`12:00:00+02:00` equals
///   `10:00:00Z`); when either value does not parse, the stored row is not
///   newer and the incoming row is stored; forced upserts always store.
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
///   `insert_entry` stores the entry and its `changed` in one transaction,
///   so no reader ever sees one without the other (REQ-BDC-0809);
///   `escalate` does not change `attempts`; `resolve` on an entry that is
///   not escalated, or on an absent seq, is `NOT_ESCALATED`, else it stores
///   `note` as [`resolve_note`](crate::resolve_note) (`resolved by <actor>:
///   <note>`) and leaves `last_error` as it was.
/// - `record_attempt`, `escalate` and `apply_with_readback` set
///   `last_error_code` to the `CODE` of the text they store
///   ([`error_code_prefix`](crate::error_code_prefix)), `None` when it has
///   none.
/// - `journal_counts` counts every entry once in `total` and once in its
///   scope's bucket or `unscoped`, from one snapshot.
/// - `prune_scope` is `PRUNE_BLOCKED` while the scope has a pending or
///   escalated entry; otherwise it deletes the scope's beads that no other
///   scope holds (with their edges), the scope's journal entries, sync state
///   and ready base, and sweeps out-of-scope rows with no open entry. The
///   scope stays registered.
/// - `status().stale` lists exactly the rows `stale_rows` returns: a row
///   whose read-back failed is listed until a store of it clears the mark
///   (a-1 amendment 1).
/// - an unknown `seq` is `NOT_FOUND` for every other seq-taking method
///   (`apply_with_readback`, `record_attempt`, `escalate`, `mark_announced`);
///   `entry_by_seq` returns `None`.
///
/// Timestamps are [`Timestamp`]s supplied by the caller (`at`), so a store
/// reads no clock.
///
/// # Errors
///
/// Every method returns a [`CacheError`]: the codes named above, or the
/// implementation's own storage failures, which it reports as
/// `STORE_FAILED` ([`CacheError::StoreFailed`], built with
/// [`CacheError::store_failed`]): the step and file in `detail`, the
/// underlying error (SQLite, I/O, a stored value that does not decode) kept
/// as the source. Other codes keep their meanings: a missing or invalid
/// metadata file is still `NO_DATABASE` (a-1 amendment 1).
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
        s: Option<&Scope>,
        rows: &[Bead],
        force: bool,
    ) -> Result<UpsertReport, CacheError>;
    /// Stores a consumer sync's result for `s` in one transaction (see the
    /// rules above; REQ-BDC-0008, 0502).
    fn ingest_scope(
        &mut self,
        s: &Scope,
        rows: &[Bead],
        blocked: &[BlockedRow],
        ready: &[BeadId],
        at: &Timestamp,
        revision: Option<&str>,
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
        at: &Timestamp,
    ) -> Result<(), CacheError>;
    /// The scope's ready base; `None` = never fetched.
    fn ready_base(&self, s: &ScopeId) -> Result<Option<ReadyBase>, CacheError>;
    /// One bead's entry in the scope's ready base; `None` = the scope never stored a base.
    ///
    /// Equal to [`BaseEntry::of`] applied to [`CacheStore::ready_base`]. The
    /// provided body loads the whole base; an implementation that can answer
    /// from an index overrides it (and reads head and entry in one snapshot), so a cached `get` costs the same whatever
    /// the size of the scope (NFR-BDC-0001).
    fn base_entry(&self, s: &ScopeId, id: &BeadId) -> Result<Option<BaseEntry>, CacheError> {
        Ok(self.ready_base(s)?.map(|b| BaseEntry::of(&b, id)))
    }
    /// Whether any journal entry is pending, or was applied at or after `at`.
    ///
    /// Equal to "`pending_entries` or `applied_since(at)` is not empty". The
    /// provided body loads both lists; an implementation that can ask the
    /// question of an index overrides it, so a cached `get` does not read the
    /// journal's history (NFR-BDC-0001).
    ///
    /// Though it takes `&self`, an implementation may write to date journal
    /// entries another writer left without an applied time, so later calls
    /// stay index lookups. Such a write must not wait for the write lock
    /// (it is skipped when the lock is held elsewhere), and the answer must
    /// be the same whether or not it runs (ADR-STORE-0004, lead rulings S25/S26).
    fn has_entries_since(&self, at: &Timestamp) -> Result<bool, CacheError> {
        Ok(!self.pending_entries()?.is_empty() || !self.applied_since(at)?.is_empty())
    }
    /// Marks the scope's ready base stale.
    fn mark_ready_base_stale(&mut self, s: &ScopeId) -> Result<(), CacheError>;

    /// The entry holding a key, in any state.
    fn entry_by_key(&self, k: &IdemKey) -> Result<Option<JournalEntry>, CacheError>;
    /// The entry with a seq, in any state.
    fn entry_by_seq(&self, seq: Seq) -> Result<Option<JournalEntry>, CacheError>;
    /// Journals a write as `Pending`; `at` is its `created_at`.
    fn insert_entry(&mut self, e: NewEntry, at: &Timestamp) -> Result<Seq, CacheError>;
    /// Every pending entry, by seq.
    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError>;
    /// A bead's pending entries, by seq.
    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError>;
    /// Entries applied at or after `at`, by seq.
    ///
    /// As [`CacheStore::has_entries_since`], an implementation may write to
    /// date entries, must not wait for the write lock to do so, and returns
    /// the same entries either way.
    fn applied_since(&self, at: &Timestamp) -> Result<Vec<JournalEntry>, CacheError>;
    /// Marks an entry applied, storing the read-back row or marking the bead stale; one transaction.
    fn apply_with_readback(
        &mut self,
        seq: Seq,
        row: Option<&Bead>,
        err: Option<&str>,
        at: &Timestamp,
    ) -> Result<(), CacheError>;
    /// Counts one failed attempt with its error; returns the new count.
    fn record_attempt(&mut self, seq: Seq, err: &str, at: &Timestamp) -> Result<u32, CacheError>;
    /// Escalates an entry with its reason.
    fn escalate(&mut self, seq: Seq, reason: &str, at: &Timestamp) -> Result<(), CacheError>;
    /// Resolves an escalated entry; `NOT_ESCALATED` otherwise.
    fn resolve(
        &mut self,
        seq: Seq,
        note: &str,
        actor: &Actor,
        at: &Timestamp,
    ) -> Result<(), CacheError>;
    /// Escalated entries not yet announced, by seq.
    fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError>;
    /// Records that an escalation was announced (handled by a post-sync hook).
    fn mark_announced(&mut self, seq: Seq, at: &Timestamp) -> Result<(), CacheError>;
    /// Applies the rebuild rule to a fresh fingerprint (REQ-BDC-0105).
    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError>;
    /// A scope's sync state (all `None` before its first sync).
    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError>;
    /// Stores a scope's sync state.
    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError>;
    /// Marks the scope's current run of failures handled when it began at
    /// `since` (compare-and-set; see the rules above).
    fn announce_scope(&mut self, s: &ScopeId, since: &Timestamp) -> Result<bool, CacheError>;
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
    /// The journal's open work: pending, and escalated but not announced.
    fn open_work(&self) -> Result<OpenWork, CacheError>;
    /// Status: meta, journal counts, scopes, escalations, outage, and the
    /// stale rows (`stale`, the same set as [`CacheStore::stale_rows`]).
    ///
    /// One consistent snapshot: every field is read at the same point (one
    /// read transaction for SQLite, one lock for `FakeStore`).
    fn status(&self) -> Result<StoreStatus, CacheError>;
    /// The journal counted by state, in total, per scope and unscoped
    /// (contract §6, `cache status` `journal`).
    ///
    /// One consistent snapshot, and for SQLite one grouped query, never a
    /// scan of the journal in Rust. The provided body derives the counts from
    /// [`CacheStore::status`]'s `scope:state` keys, where a missing scope is
    /// `None`, so it counts the entries of a scope named `None` as unscoped;
    /// every store of this workspace overrides it.
    ///
    /// Known limitation of the provided body (QA on #75): `None` is a valid
    /// scope name, and v3's `status` keys cannot tell `None:pending` (no
    /// scope) from the scope `None`, so that scope's entries land in
    /// `unscoped` and it gets no `by_scope` bucket; `total` is still right.
    /// The overrides in `FakeStore` and `SqliteStore` read the scope column itself
    /// and count such a scope in its own bucket.
    fn journal_counts(&self) -> Result<JournalCounts, CacheError> {
        let mut counts = JournalCounts::default();
        for (key, n) in self.status()?.journal {
            let Some((name, state)) = key.rsplit_once(':') else {
                continue;
            };
            let Some(state) = JournalState::parse(state) else {
                continue;
            };
            let scope = if name == "None" {
                None
            } else {
                ScopeId::new(name).ok()
            };
            counts.add(scope.as_ref(), state, n);
        }
        Ok(counts)
    }
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
