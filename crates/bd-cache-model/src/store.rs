//! The store port ([`CacheStore`]) and the cross-process drain lock ([`DrainLock`]).

use std::collections::BTreeSet;
use std::time::Duration;

use sc_beads::{Actor, Bead, BeadId, BlockedRow, Edge, Fingerprint, Timestamp};

use crate::error::CacheError;
use crate::ingest::IngestSnapshot;
use crate::journal::JournalState;
use crate::journal::{IdemKey, JournalEntry, NewEntry, Seq};
use crate::reply::{JournalCounts, PruneReport};
use crate::scope::{Scope, ScopeId};
use crate::state::{
    BaseEntry, EscalationPage, FailingSince, FingerprintOutcome, Freshness, HookState, LeaseHolder,
    LeaseTake, OpenWork, Outage, PendingSummary, ReadyBase, RunId, StoreStatus, SyncOutcome,
    SyncRun, SyncRunEnd, SyncState, UpsertReport,
};

/// The cache's store; a-4 implements it over SQLite, `testing::FakeStore` in memory.
///
/// An open extension point: `bd-cache-store`'s `SqliteStore` and the
/// in-memory `FakeStore` implement it, and a test wrapper may too. It is not
/// sealed, so adding a required method is a breaking change for every
/// implementor and bumps this crate's version; a method that can be answered
/// from the others gets a provided body instead (`members`, `scopes_of`,
/// `base_entry`, `has_entries_since`, `journal_counts`).
///
/// Rules (checked by the contract suite, `testing::contract`):
///
/// - `get_row(upsert(b)) == normalize(b)`: `dependencies` rebuilt from the
///   edges table, counts absent ([`normalize`](crate::normalize)).
/// - upsert writes the bead's outgoing edges from `Bead.dependencies`,
///   replacing the old ones.
/// - any successful store of a bead clears `stale`.
/// - `upsert_rows(Some(s), ..)` sets membership of `s` from [`Scope::holds`]
///   (added when the scope holds the row: its label for a phase-a scope, its
///   filter with the status criterion for a filter scope; removed when it
///   does not); `upsert_rows(None, ..)` never changes existing membership (a
///   new row stored with `None` has none: an out-of-scope row).
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
/// - The registry (ADR-BDC-0013): `ensure_scope` registers a scope that is
///   absent and returns the stored one (an existing scope keeps its filter);
///   `put_scope` is add-or-set: it creates or updates; `scope_add` refuses an
///   existing name (`BAD_ARGS`), `scope_set` an absent one (`SCOPE_UNKNOWN`).
///   The four share one registration rule, and a registration that would
///   make the registry hold more than [`MAX_REGISTERED_SCOPES`](crate::MAX_REGISTERED_SCOPES)
///   names is `BAD_ARGS` ([`CacheError::registry_full`]), nothing written. A changed
///   filter (`put_scope`, `scope_set`) drops the scope's memberships, sync
///   state and ready base, so its next sync starts from nothing; the rows stay.
///   `scope_remove` is `prune_scope` (and its refusal) followed by removing
///   the scope from the registry, in one transaction. `scopes()` returns
///   every registered scope, by name, across restarts.
/// - `ingest_scope` (REQ-BDC-0726) stores a checked snapshot in one
///   transaction: the scope must be registered (`SCOPE_UNKNOWN` otherwise,
///   read in that transaction: an ingest never registers a scope), its
///   registered filter decides the status criterion; rows unforced (the
///   newer-row guard), the scope's membership replaced by the snapshot's
///   rows the scope's status criterion admits (other scopes' memberships
///   kept), the ready base replaced by `blocked` and `ready` restricted to
///   those members, the sync state an ok result at `at` (`failing_since`
///   cleared), and `revision` stored as given. The limits are the
///   snapshot's ([`IngestSnapshot`] checks both): the store trusts it.
/// - `record_sync_result` applies one sync result to the scope's sync state
///   in one write transaction (REQ-BDC-0728): an ok result sets
///   `last_started_at` and `last_ok_at` to `started` and clears
///   `last_error`, `failing_since` and `notified_since`; a failure sets
///   `last_started_at` and `last_error` and sets `failing_since` to `started`
///   only when it is unset; a deferred sync sets `last_started_at` only. It
///   reads the stored state inside that transaction, so a `scope_announce`
///   or an ingest by another process is never overwritten by a stale copy.
///   `record_sync` never writes `notified_since`: `scope_announce` sets it.
/// - `scopes_of(id)` is every registered scope whose stored membership holds
///   the bead, by name: the scopes `members` would list it in.
/// - `scope_announce(s, since)` marks the current failure only when `since`
///   equals the stored `failing_since`, else `PRECONDITION` (REQ-BDC-0728);
///   `mark_offered` sets `first_offered_at` of entries that have none, never
///   changing a set one.
/// - `lease_take` (REQ-BDC-0722) takes the one lease under one write
///   transaction: free, it is `Taken`; held with `expires_at` after `now`, it
///   is `Held` and nothing changes; expired, it is `TookOver`. `lease_release`
///   frees it only for its own `run_id`.
/// - `hook_failed` counts one failure (keeping the first failure's time),
///   `hook_succeeded` resets the count (REQ-BDC-0727).
/// - `replace_ready_base` replaces the whole base for the scope and clears
///   its stale mark; its `at` is the instant taken before the sync's first
///   `list_blocked` / `list_ready` call. `ready_base` is `None` only when the
///   scope never stored one; `mark_ready_base_stale` on a missing base is a
///   no-op; `prune_scope` deletes it.
/// - `applied_since(at)` returns entries applied at or after `at`, in seq order.
/// - `last_applied_at` is the newest applied time among the entries the
///   journal still holds (`None` when none is applied), whatever their seqs.
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
    /// Registers `s` when its name is absent (on demand, by a sync); returns
    /// the registered scope, whose filter an existing scope keeps.
    fn ensure_scope(&mut self, s: &Scope) -> Result<Scope, CacheError>;
    /// Creates or updates `s` (the scopes the configuration file defines,
    /// written when the cache opens: the file wins).
    fn put_scope(&mut self, s: &Scope) -> Result<(), CacheError>;
    /// Adds a new scope; `BAD_ARGS` when the name is registered.
    fn scope_add(&mut self, s: &Scope) -> Result<(), CacheError>;
    /// Changes a registered scope's filter; `SCOPE_UNKNOWN` when it is absent.
    fn scope_set(&mut self, s: &Scope) -> Result<(), CacheError>;
    /// Prunes a scope and removes it from the registry, in one transaction.
    fn scope_remove(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError>;
    /// The registered scope of a name.
    fn scope(&self, s: &ScopeId) -> Result<Option<Scope>, CacheError>;
    /// Every registered scope, by name.
    fn scopes(&self) -> Result<Vec<Scope>, CacheError>;
    /// Every registered scope whose stored membership holds `id`, by name.
    ///
    /// Equal to the scopes whose [`CacheStore::members`] contain `id`; the
    /// provided body reads every scope's members, an implementation with a
    /// membership index overrides it, so a cached `get` does not read the
    /// whole registry (NFR-BDC-0001).
    fn scopes_of(&self, id: &BeadId) -> Result<Vec<Scope>, CacheError> {
        let mut out = Vec::new();
        for s in self.scopes()? {
            if self.members(&s.id)?.contains(id) {
                out.push(s);
            }
        }
        Ok(out)
    }
    /// The opaque `revision` the last ingest of the scope stored.
    fn scope_revision(&self, s: &ScopeId) -> Result<Option<String>, CacheError>;
    /// The ids of the beads the scope holds.
    ///
    /// Equal to the ids of [`CacheStore::rows_in_scope`]; the provided body
    /// loads the rows, an implementation that can read the ids alone overrides it.
    fn members(&self, s: &ScopeId) -> Result<BTreeSet<BeadId>, CacheError> {
        Ok(self
            .rows_in_scope(s)?
            .into_iter()
            .map(|(b, _)| b.id)
            .collect())
    }
    /// Stores a checked snapshot of a registered scope in one transaction (see the rules above).
    fn ingest_scope(
        &mut self,
        s: &ScopeId,
        snapshot: &IngestSnapshot,
        ready: &[BeadId],
        at: &Timestamp,
        revision: Option<&str>,
    ) -> Result<UpsertReport, CacheError>;
    /// Marks the scope's current failure as notified when `since` is it; else `PRECONDITION`.
    fn scope_announce(&mut self, s: &ScopeId, since: &FailingSince) -> Result<(), CacheError>;
    /// Sets `first_offered_at` to `at` on each entry that has none.
    fn mark_offered(&mut self, seqs: &[Seq], at: &Timestamp) -> Result<(), CacheError>;
    /// Takes the sync lease for `holder` (see the rules above).
    fn lease_take(
        &mut self,
        holder: &LeaseHolder,
        now: &Timestamp,
    ) -> Result<LeaseTake, CacheError>;
    /// Releases the lease when `run_id` holds it; `false` when it does not.
    fn lease_release(&mut self, run_id: &RunId) -> Result<bool, CacheError>;
    /// The lease's holder, live or expired; `None` when it is free.
    fn lease(&self) -> Result<Option<LeaseHolder>, CacheError>;
    /// The post-sync hook's backoff state.
    fn hook_state(&self) -> Result<HookState, CacheError>;
    /// Counts one failed hook call with its error and next attempt; returns the new state.
    fn hook_failed(
        &mut self,
        err: &str,
        at: &Timestamp,
        next_attempt_at: Option<&Timestamp>,
    ) -> Result<HookState, CacheError>;
    /// Resets the hook's failure count after a success.
    fn hook_succeeded(&mut self) -> Result<(), CacheError>;
    /// The run record of the sync row named `row`, if it ever ran (REQ-BDC-0731).
    fn sync_run(&self, row: &str) -> Result<Option<SyncRun>, CacheError>;
    /// Records that run `run_id` of `row` started at `at`, in one
    /// transaction with the read of the previous record ([`SyncRun::started`]);
    /// returns the new record.
    fn sync_run_started(
        &mut self,
        row: &str,
        run_id: &RunId,
        at: &Timestamp,
    ) -> Result<SyncRun, CacheError>;
    /// Records that run `run_id` of `row` finished at `at` with `end`
    /// ([`SyncRun::finished`]); `Ok(false)` and nothing written when the
    /// row's record names another run.
    fn sync_run_finished(
        &mut self,
        row: &str,
        run_id: &RunId,
        at: &Timestamp,
        end: SyncRunEnd,
    ) -> Result<bool, CacheError>;
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
    /// The pending entries' count and the oldest one's `created_at`
    /// (REQ-BDC-0727), without holding every pending entry.
    ///
    /// The provided body reads [`CacheStore::pending_entries`]; `SqliteStore`
    /// overrides it with a count and a one-row read, so a long outage's
    /// backlog is never loaded to be counted.
    fn pending_summary(&self) -> Result<PendingSummary, CacheError> {
        let pending = self.pending_entries()?;
        Ok(PendingSummary {
            count: u64::try_from(pending.len()).unwrap_or(u64::MAX),
            oldest_created_at: pending
                .iter()
                .map(|e| e.created_at.clone())
                .min_by_key(Timestamp::unix),
        })
    }
    /// A bead's pending entries, by seq.
    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError>;
    /// Entries applied at or after `at`, by seq.
    ///
    /// As [`CacheStore::has_entries_since`], an implementation may write to
    /// date entries, must not wait for the write lock to do so, and returns
    /// the same entries either way.
    fn applied_since(&self, at: &Timestamp) -> Result<Vec<JournalEntry>, CacheError>;
    /// When the journal last applied an entry: the newest applied time of any
    /// entry still in the journal, `None` when none was applied.
    ///
    /// Judged on the entries' applied times, never on their seqs: a prune
    /// deletes entries, so seqs have gaps and the newest entry need not be the
    /// last row count. The provided body reads every applied entry through
    /// [`CacheStore::applied_since`]; an implementation that can ask an index
    /// overrides it (`SqliteStore` does, with no write, in one read
    /// snapshot). ADR-STORE-0006 amendment (sprint b-9).
    ///
    /// # Panics
    ///
    /// Never: [`Timestamp::MIN_UNIX`] is the earliest instant `from_unix` accepts.
    #[expect(
        clippy::expect_used,
        reason = "invariant: Timestamp::MIN_UNIX is in range by definition"
    )]
    fn last_applied_at(&self) -> Result<Option<Timestamp>, CacheError> {
        let floor = Timestamp::from_unix(Timestamp::MIN_UNIX)
            .expect("Timestamp::MIN_UNIX is a valid timestamp");
        Ok(self
            .applied_since(&floor)?
            .into_iter()
            .filter_map(|e| e.applied_at)
            .max())
    }
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
    /// At most `limit` escalated entries not yet announced, by seq, and how
    /// many there are in all (REQ-BDC-0727's request offers at most 500).
    ///
    /// The provided body reads [`CacheStore::unannounced_escalations`];
    /// `SqliteStore` overrides it with a `LIMIT` read and a count.
    fn unannounced_escalations_first(&self, limit: usize) -> Result<EscalationPage, CacheError> {
        let mut entries = self.unannounced_escalations()?;
        let total = u64::try_from(entries.len()).unwrap_or(u64::MAX);
        entries.truncate(limit);
        Ok(EscalationPage { entries, total })
    }
    /// Records that an escalation was announced.
    fn mark_announced(&mut self, seq: Seq, at: &Timestamp) -> Result<(), CacheError>;
    /// [`CacheStore::mark_announced`], waiting at most `wait` for a write lock
    /// another process holds (lead ruling S33).
    ///
    /// The tick's bounded writes, the post-send markers and the alert clears
    /// that send no message (lead ruling S35), pass the time left before its
    /// end, so a contended write cannot carry the tick past it: one that cannot
    /// finish in time returns the store's usual error, the entry stays
    /// unannounced, and the next tick sends it again (at least once). The
    /// provided body ignores `wait` (it is unbounded), for a store that never
    /// waits for a lock held elsewhere (the in-memory `FakeStore`); a store or
    /// wrapper that can wait overrides both `_within` methods, forwarding the
    /// wait.
    fn mark_announced_within(
        &mut self,
        seq: Seq,
        at: &Timestamp,
        wait: Duration,
    ) -> Result<(), CacheError> {
        let _ = wait;
        self.mark_announced(seq, at)
    }
    /// Applies the rebuild rule to a fresh fingerprint (REQ-BDC-0105).
    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError>;
    /// A scope's sync state (all `None` before its first sync).
    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError>;
    /// Stores a scope's sync state, except `notified_since`, which only
    /// `scope_announce` sets.
    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError>;
    /// Applies one sync result to the scope's sync state in one write
    /// transaction (the `failing_since` lifecycle, see the rules above);
    /// returns the state written.
    fn record_sync_result(
        &mut self,
        s: &ScopeId,
        outcome: &SyncOutcome,
        started: &Timestamp,
    ) -> Result<SyncState, CacheError>;
    /// The scope's last announced sync-failure cause (stable cause key).
    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError>;
    /// Sets (`Some`) or clears (`None`) the scope's sync alert.
    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError>;
    /// [`CacheStore::set_sync_alert`], waiting at most `wait` for a write lock
    /// another process holds, as [`CacheStore::mark_announced_within`] (lead
    /// ruling S33): an alert not recorded in time is announced again next tick.
    /// The tick also clears an alert (`None`) through this method when it
    /// sends no message (lead ruling S35); a clear not recorded in time is
    /// retried next tick.
    fn set_sync_alert_within(
        &mut self,
        s: &ScopeId,
        cause: Option<&str>,
        wait: Duration,
    ) -> Result<(), CacheError> {
        let _ = wait;
        self.set_sync_alert(s, cause)
    }
    /// The outage, while one holds.
    fn outage(&self) -> Result<Option<Outage>, CacheError>;
    /// Records (`Some`) or clears (`None`) the outage.
    fn set_outage(&mut self, o: Option<&Outage>) -> Result<(), CacheError>;
    /// Prunes a scope (see the rules above).
    fn prune_scope(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError>;
    /// Pending entries and unannounced escalations.
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
    /// [`CacheStore::status`]'s `phase:state` keys, where a missing phase is
    /// `None`, so it counts the entries of a scope named `None` as unscoped;
    /// every store of this workspace overrides it.
    ///
    /// Known limitation of the provided body (QA on #75): `None` is a valid
    /// phase name, and v3's `status` keys cannot tell `None:pending` (no
    /// phase) from the scope `None`, so that scope's entries land in
    /// `unscoped` and it gets no `by_scope` bucket; `total` is still right.
    /// The overrides in `FakeStore` and `SqliteStore` read the phase itself
    /// and count such a scope in its own bucket.
    fn journal_counts(&self) -> Result<JournalCounts, CacheError> {
        let mut counts = JournalCounts::default();
        for (key, n) in self.status()?.journal {
            let Some((phase, state)) = key.rsplit_once(':') else {
                continue;
            };
            let Some(state) = JournalState::parse(state) else {
                continue;
            };
            let scope = if phase == "None" {
                None
            } else {
                ScopeId::new(phase).ok()
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
