//! bd-cache core: the [`Cache`] facade over a store, a drain lock and a backend.
//!
//! A [`Cache`] is a filtered local replica of a beads database
//! (`docs/architecture.md`, Target Design). It is generic over the three
//! ports from `bd-cache-model` and `sc-beads`, so the core never names the
//! SQLite store or the bd CLI (`docs/architecture.md`, Boundaries):
//!
//! - `S:` [`CacheStore`]: rows, journal, scopes, ready bases, outage;
//! - `L:` [`DrainLock`]: the cross-process single drain writer;
//! - `B:` [`CacheBackend`]: bd, through `sc-bd-cli` or a test double.
//!
//! The verbs are split by owner: the write path (`write`, `drain`) is sprint
//! a-5's, the read path (`get`, `entry`, `refresh`, `list`, `ready`, `sync`,
//! `resolve`, `prune`, `status`) sprint a-6's. Both call the shared helpers
//! this crate defines privately, with sprint a-1:
//!
//! - `call(cache, kind, hard, f)`: every backend call. It hands `f` the
//!   earlier of the caller's hard deadline and the per-call cap
//!   ([`CacheConfig::call_timeout`]) and classifies the result as a
//!   `Result<T, CallFailure>`: a timeout after the hard deadline expired is
//!   [`CallFailure::Cut`] (no outage, no attempt); an unreachable backend on
//!   a check or list is a [`CallFailure::Outage`] and is recorded; on a write
//!   it is a [`CallFailure::Error`] that counts an attempt. Only a list that
//!   answers clears the outage, never the backend check alone (ADR-BDC-0005).
//! - `outage::note_unavailable` / `outage::note_ok`: record and clear the outage.
//! - `view::current_view`: the stored row with pending entries replayed.
//! - `backend_check::run`: fingerprint and the rebuild rule (REQ-BDC-0105).
//! - `miss::fill`: one bounded `bd list --id` for a cache miss.
//!
//! # Threading
//!
//! A `Cache` is `Send` but not `Sync`: it owns one store connection and every
//! verb takes `&self` or `&mut self` on one thread (ADR-CORE-0001 in
//! `docs/bd-cache/architecture.md`). Concurrency across processes goes through
//! the drain lock and the store's own transactions.

#![deny(missing_docs)]

use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

#[doc(no_inline)]
pub use bd_cache_model::{
    CacheConfig, CacheError, CacheStore, CallFailure, CallKind, DrainBudget, DrainLock,
    DrainReport, EscalationPage, FailingSince, GetReply, HookState, IdemKey, IngestSnapshot,
    JournalCounts, JournalEntry, LeaseHolder, LeaseTake, ListReply, OpenWork, PendingSummary,
    PruneReport, ReadyReply, RowView, Scope, ScopeId, Seq, StateCounts, StatusReply, SyncReport,
    SyncRun, SyncRunEnd, SyncState, UpsertReport, WriteOutcome, WriteReply,
};
use sc_beads::{Actor, BeadId, CacheBackend, Clock, Deadline, Guards};

mod backend_check;
mod drain;
mod get;
mod ingest;
mod list;
mod miss;
mod outage;
mod prune;
mod ready;
mod refresh;
mod resolve;
mod scopes;
mod status;
mod sync;
mod view;
mod write;

pub(crate) use outage::call;

#[doc(no_inline)]
pub use bd_cache_model::CLOSED_STATUS;

/// The bd-cache facade.
///
/// Not `Sync` (one store connection); see the crate docs, Threading.
pub struct Cache<S: CacheStore, L: DrainLock, B: CacheBackend> {
    pub(crate) config: CacheConfig,
    pub(crate) store: S,
    pub(crate) lock: L,
    pub(crate) backend: B,
    pub(crate) clock: Arc<dyn Clock>,
    /// Keeps `Cache` `!Sync` whatever the store is (ADR-CORE-0001).
    _not_sync: PhantomData<Cell<()>>,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> fmt::Debug for Cache<S, L, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cache")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> Cache<S, L, B> {
    /// Builds a cache and writes every configured scope to the store's
    /// registry: created, or updated to the file's filter (the file wins,
    /// ADR-BDC-0013).
    ///
    /// The backend must have been built with the same `clock`.
    ///
    /// # Errors
    ///
    /// The store's error when writing a scope fails.
    pub fn new(
        config: CacheConfig,
        mut store: S,
        lock: L,
        backend: B,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, CacheError> {
        for s in &config.scopes {
            store.put_scope(s)?;
        }
        Ok(Self {
            config,
            store,
            lock,
            backend,
            clock,
            _not_sync: PhantomData,
        })
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// The clock the cache and its backend share (ADR-CORE-0001).
    ///
    /// Callers that measure time against the cache's deadlines, such as a
    /// scheduler's tick, must read this clock and no other. It is a shared
    /// handle, so it can be held while the cache is borrowed mutably.
    #[must_use]
    pub fn clock(&self) -> Arc<dyn Clock> {
        Arc::clone(&self.clock)
    }

    /// The cached row with its pending entries replayed; on a miss one bounded
    /// `bd list --id` (`miss_timeout`, else the configured miss-fill timeout).
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` for a bead in neither cache nor bd; `BD_UNAVAILABLE` when a
    /// miss cannot be filled in time; store errors.
    pub fn get(
        &mut self,
        id: &BeadId,
        miss_timeout: Option<Duration>,
    ) -> Result<GetReply, CacheError> {
        get::get(self, id, miss_timeout)
    }

    /// The cached bead as `get` would answer, or `None` when the cache does not
    /// hold it. Never calls bd and never fills a miss. Its cost does not grow
    /// with the size of the bead's scope while no write is in flight (nothing
    /// pending, nothing applied since the scope's base was fetched); with one in
    /// flight the scope's whole base is adjusted, as `get` does (ADR-CORE-0206).
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn cached(&self, id: &BeadId) -> Result<Option<GetReply>, CacheError> {
        get::cached(self, id)
    }

    /// A fresh read of `ids` (one `bd list --id`, all statuses) under `timeout`,
    /// stored with force, with pending entries replayed (REQ-BDC-0907).
    ///
    /// # Errors
    ///
    /// The mapped backend error, or store errors.
    pub fn refresh(
        &mut self,
        ids: &[BeadId],
        timeout: Duration,
    ) -> Result<Vec<RowView>, CacheError> {
        refresh::refresh(self, ids, timeout)
    }

    /// One journal entry by seq, in any state (REQ-BDC-0908).
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` for an unknown seq; store errors.
    pub fn entry(&self, seq: Seq) -> Result<JournalEntry, CacheError> {
        get::entry(self, seq)
    }

    /// The journal entry an idempotency key journals, in any state (REQ-BDC-0908).
    ///
    /// Answers a retry after a lost reply (`journal get --idempotency-key`).
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when no entry holds `key`; store errors.
    pub fn entry_by_key(&self, key: &IdemKey) -> Result<JournalEntry, CacheError> {
        get::entry_by_key(self, key)
    }

    /// The cached rows of a known scope with the overlay applied.
    ///
    /// # Errors
    ///
    /// `SCOPE_UNKNOWN` for a scope never configured or synced; store errors.
    pub fn list(
        &self,
        s: &ScopeId,
        status: Option<&str>,
        label: Option<&str>,
    ) -> Result<ListReply, CacheError> {
        list::list(self, s, status, label)
    }

    /// The scope's ready beads; with `fresh_within`, bd's answer if it comes
    /// in time, else the cached adjusted answer. Never exceeds `fresh_within`.
    ///
    /// # Errors
    ///
    /// `SCOPE_UNKNOWN`; store errors.
    pub fn ready(
        &mut self,
        s: &ScopeId,
        fresh_within: Option<Duration>,
    ) -> Result<ReadyReply, CacheError> {
        ready::ready(self, s, fresh_within)
    }

    /// Journals one write after validating it and checking its precondition.
    ///
    /// # Errors
    ///
    /// `BAD_OP`, `BAD_ARGS` (also for args or a key over
    /// [`MAX_WRITE_ARGS_BYTES`](bd_cache_model::MAX_WRITE_ARGS_BYTES) /
    /// [`MAX_IDEM_KEY_BYTES`](bd_cache_model::MAX_IDEM_KEY_BYTES)),
    /// `BAD_PRECONDITION`, `IDEM_CONFLICT`, `PRECONDITION`, a miss-fill error,
    /// or store errors.
    pub fn write(
        &mut self,
        id: &BeadId,
        op_name: &str,
        args_json: &str,
        pre: Guards,
        key: IdemKey,
        actor: Actor,
    ) -> Result<WriteReply, CacheError> {
        write::write(self, id, op_name, args_json, pre, key, actor)
    }

    /// Journals one write as [`Cache::write`] does, and answers the stored
    /// entry with the bead's views before and after it (REQ-BDC-0809).
    ///
    /// The entry and its `changed` are stored in one store transaction, so a
    /// concurrent reader sees neither or both. `changed` is whether the write
    /// changes the view; for a close it is `!already_closed`, where
    /// `already_closed` describes the view before the write. `after` is the
    /// row a [`Cache::get`] answers right after the call; `before` and
    /// `after` are snapshots read on either side of the insert transaction,
    /// so `after` may already reflect a concurrent drain (lead ruling S64).
    ///
    /// A retry with the same key and the same `(bead, op)` is a duplicate,
    /// whatever its guards and actor (contract §5.1): it journals nothing and
    /// returns the stored entry with its stored `changed`, and `before` and
    /// `after` are both the current view. An entry stored without `changed`
    /// (by the v3 Python writer) answers a prediction while pending and `None`
    /// otherwise (lead ruling S68). Unlike [`Cache::write`], a duplicate
    /// for a bead the cache no longer holds fills it (one bounded `bd list`).
    ///
    /// # Errors
    ///
    /// As [`Cache::write`]; on a duplicate, also the miss-fill errors of a
    /// bead the cache no longer holds.
    pub fn write_returning(
        &mut self,
        id: &BeadId,
        op_name: &str,
        args_json: &str,
        pre: Guards,
        key: IdemKey,
        actor: Actor,
    ) -> Result<WriteOutcome, CacheError> {
        write::write_returning(self, id, op_name, args_json, pre, key, actor)
    }

    /// Applies pending entries in seq order within the budget.
    ///
    /// # Errors
    ///
    /// Store errors; backend failures are reported in the [`DrainReport`].
    pub fn drain(&mut self, budget: &DrainBudget) -> Result<DrainReport, CacheError> {
        drain::drain(self, budget)
    }

    /// Syncs one scope (registering it on demand) and refreshes its ready base.
    ///
    /// # Errors
    ///
    /// `SYNC_FAILED` and the other typed errors of a failed sync; store errors.
    pub fn sync(&mut self, s: &ScopeId, d: Deadline) -> Result<SyncReport, CacheError> {
        sync::sync(self, s, d)
    }

    /// `sync` of the phase-a scope a 0.1 argument names (REQ-BDC-0812 as
    /// amended): `x` registers the phase-a scope `x` (label: the legacy prefix
    /// followed by `x`) when absent; the 0.1 Python `sync` calls this.
    ///
    /// # Errors
    ///
    /// As [`Cache::sync`], and `SCOPE_UNKNOWN` when `s` is registered as a
    /// filter scope, which a 0.1 name never means (nothing is synced).
    pub fn sync_legacy(&mut self, s: &ScopeId, d: Deadline) -> Result<SyncReport, CacheError> {
        sync::sync_legacy(self, s, d)
    }

    /// The phase-a scope a 0.1 name resolved to: `id` is what the 0.1 prefix
    /// rule ([`bd_cache_model::legacy_scope`]) gave for the argument, applied
    /// once where the argument was parsed; this checks only that the name is
    /// not a registered filter scope (REQ-BDC-0812 as amended, ADR-BDC-0013).
    /// It strips nothing, so a 0.1 name is never resolved twice. For the 0.1
    /// verbs only; every other interface takes a scope name as written
    /// ([`ScopeId::new`]).
    ///
    /// # Errors
    ///
    /// `SCOPE_UNKNOWN` when `id`'s registered scope has a filter.
    pub fn legacy_scope(&self, id: &ScopeId) -> Result<ScopeId, CacheError> {
        scopes::legacy(self, id)
    }

    /// Every registered scope with its filter, by name (`scopes list`).
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn scopes(&self) -> Result<Vec<Scope>, CacheError> {
        self.store.scopes()
    }

    /// The registered scope of a name.
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn scope(&self, s: &ScopeId) -> Result<Option<Scope>, CacheError> {
        self.store.scope(s)
    }

    /// The opaque `revision` the scope's last ingest stored (REQ-BDC-0726).
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn scope_revision(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        self.store.scope_revision(s)
    }

    /// `scopes add`: registers a new scope (ADR-BDC-0013).
    ///
    /// # Errors
    ///
    /// `CONFIG_INVALID` when `.beads/bd-cache.toml` defines the name;
    /// `BAD_ARGS` when the name is registered.
    pub fn scopes_add(&mut self, scope: &Scope) -> Result<(), CacheError> {
        scopes::add(self, scope)
    }

    /// `scopes set`: changes a registry scope's filter; a changed filter
    /// starts the scope again (its memberships, sync state and ready base go).
    ///
    /// # Errors
    ///
    /// `CONFIG_INVALID` when `.beads/bd-cache.toml` defines the scope;
    /// `SCOPE_UNKNOWN` when it is not registered.
    pub fn scopes_set(&mut self, scope: &Scope) -> Result<(), CacheError> {
        scopes::set(self, scope)
    }

    /// `scopes remove`: prunes the scope and removes it from the registry
    /// (REQ-BDC-0602 as amended): its memberships go, not the rows another
    /// scope holds.
    ///
    /// # Errors
    ///
    /// `CONFIG_INVALID` when `.beads/bd-cache.toml` defines the scope;
    /// `SCOPE_UNKNOWN` when it is not registered; `PRUNE_BLOCKED` while it
    /// has pending or escalated writes.
    pub fn scopes_remove(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        scopes::remove(self, s)
    }

    /// Stores a snapshot of a scope a caller fetched through bd, by the
    /// built-in sync's rules, in one transaction (REQ-BDC-0726; module
    /// `ingest`). Starts no bd process.
    ///
    /// # Errors
    ///
    /// `SCOPE_UNKNOWN` for an unregistered scope; `INGEST_INVALID` for an
    /// empty snapshot without `allow_empty`, or one over the store's limits,
    /// with the stored scope untouched.
    pub fn ingest_scope(
        &mut self,
        s: &ScopeId,
        snapshot: &IngestSnapshot,
        revision: Option<&str>,
        allow_empty: bool,
    ) -> Result<UpsertReport, CacheError> {
        ingest::ingest(self, s, snapshot, revision, allow_empty)
    }

    /// A scope's sync state, its failure state included (REQ-BDC-0728).
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError> {
        self.store.sync_state(s)
    }

    /// Marks the scope's current failure as notified, compare-and-set on
    /// `since` (REQ-BDC-0728): a mark meant for an old failure never silences
    /// a new one.
    ///
    /// # Errors
    ///
    /// `PRECONDITION` when `since` is not the scope's current `failing_since`.
    pub fn scope_announce(&mut self, s: &ScopeId, since: &FailingSince) -> Result<(), CacheError> {
        self.store.scope_announce(s, since)
    }

    /// Records that these entries were offered to a post-sync hook now:
    /// `first_offered_at` is set once and never changed (REQ-BDC-0727).
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` for an absent seq.
    pub fn mark_offered(&mut self, seqs: &[Seq]) -> Result<(), CacheError> {
        let at = self.clock.now_utc();
        self.store.mark_offered(seqs, &at)
    }

    /// Takes the sync lease for `holder` at the clock's now (REQ-BDC-0722,
    /// the store call; the guard around a run is the sync crate's).
    ///
    /// # Errors
    ///
    /// `BAD_ARGS` for an `expires_at` not after now or more than
    /// [`MAX_LEASE_TTL`](bd_cache_model::MAX_LEASE_TTL) past it; the store's failure.
    pub fn lease_take(&mut self, holder: &LeaseHolder) -> Result<LeaseTake, CacheError> {
        let now = self.clock.now_utc();
        self.store.lease_take(holder, &now)
    }

    /// Releases the lease when `run_id` holds it; `false` when it does not.
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn lease_release(&mut self, run_id: &bd_cache_model::RunId) -> Result<bool, CacheError> {
        self.store.lease_release(run_id)
    }

    /// The lease's holder, live or expired.
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn lease(&self) -> Result<Option<LeaseHolder>, CacheError> {
        self.store.lease()
    }

    /// The post-sync hook's backoff state (REQ-BDC-0727).
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn hook_state(&self) -> Result<HookState, CacheError> {
        self.store.hook_state()
    }

    /// Counts one failed hook call now, with its error and next attempt.
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn hook_failed(
        &mut self,
        err: &str,
        next_attempt_at: Option<&sc_beads::Timestamp>,
    ) -> Result<HookState, CacheError> {
        let at = self.clock.now_utc();
        self.store.hook_failed(err, &at, next_attempt_at)
    }

    /// Resets the hook's failure count after a success.
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn hook_succeeded(&mut self) -> Result<(), CacheError> {
        self.store.hook_succeeded()
    }

    /// The run record of the sync row `row` (REQ-BDC-0731), if it ever ran.
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn sync_run(&self, row: &str) -> Result<Option<SyncRun>, CacheError> {
        self.store.sync_run(row)
    }

    /// Records that run `run_id` of `row` started now; a previous run that
    /// never finished is counted as a failure ([`SyncRun::started`]).
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn sync_run_started(
        &mut self,
        row: &str,
        run_id: &bd_cache_model::RunId,
    ) -> Result<SyncRun, CacheError> {
        let at = self.clock.now_utc();
        self.store.sync_run_started(row, run_id, &at)
    }

    /// Records that run `run_id` of `row` finished now with `end`; `false`
    /// when the row's record names another run.
    ///
    /// # Errors
    ///
    /// The store's failure.
    pub fn sync_run_finished(
        &mut self,
        row: &str,
        run_id: &bd_cache_model::RunId,
        end: SyncRunEnd,
    ) -> Result<bool, CacheError> {
        let at = self.clock.now_utc();
        self.store.sync_run_finished(row, run_id, &at, end)
    }

    /// The pending entries' count and the oldest one's time, without reading
    /// them all (REQ-BDC-0727).
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn pending_summary(&self) -> Result<PendingSummary, CacheError> {
        self.store.pending_summary()
    }

    /// At most `limit` unannounced escalations, oldest first, with their
    /// total (REQ-BDC-0727).
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn unannounced_escalations_first(
        &self,
        limit: usize,
    ) -> Result<EscalationPage, CacheError> {
        self.store.unannounced_escalations_first(limit)
    }

    /// Resolves an escalated entry with the lead's note.
    ///
    /// # Errors
    ///
    /// `BAD_ARGS` for a note over
    /// [`MAX_NOTE_BYTES`](bd_cache_model::MAX_NOTE_BYTES); `NOT_ESCALATED`;
    /// store errors.
    pub fn resolve(&mut self, seq: Seq, note: &str, actor: Actor) -> Result<(), CacheError> {
        resolve::resolve(self, seq, note, actor)
    }

    /// Prunes a scope.
    ///
    /// # Errors
    ///
    /// `SCOPE_UNKNOWN` for an unregistered scope (nothing is pruned or
    /// swept); `PRUNE_BLOCKED`; store errors.
    pub fn prune(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        prune::prune(self, s)
    }

    /// The store's status with the outage.
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn status(&self) -> Result<StatusReply, CacheError> {
        status::status(self)
    }

    /// The journal counted by state: in total, per scope and unscoped
    /// (contract §6, `cache status` `journal`).
    ///
    /// The store's one grouped query, never a scan of the journal; starts no
    /// bd call. `total` is the sum of the scope buckets and `unscoped`.
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn journal_counts(&self) -> Result<JournalCounts, CacheError> {
        status::journal_counts(self)
    }

    /// Whether `scope` has synced at least once (its `sync_state.last_ok_at` is set).
    ///
    /// Reads the scope's sync state alone through [`CacheStore::sync_state`]:
    /// no `bd` call and no scan of the journal or the rows, unlike
    /// [`status`](Self::status) (lead ruling S30, QA5-004). A scope that was
    /// never registered has never synced.
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn scope_synced(&self, scope: &ScopeId) -> Result<bool, CacheError> {
        Ok(self.store.sync_state(scope)?.last_ok_at.is_some())
    }

    /// Escalated entries not yet announced (for an external scheduler's sender).
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.store.unannounced_escalations()
    }

    /// Records that an escalation was announced, at the clock's UTC time.
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` for an unknown seq; store errors.
    pub fn mark_announced(&mut self, seq: Seq) -> Result<(), CacheError> {
        let at = self.clock.now_utc();
        self.store.mark_announced(seq, &at)
    }

    /// [`Cache::mark_announced`], waiting at most `wait` for a write lock
    /// another process holds (lead ruling S33): the tick passes the time it
    /// has left, so the marker write cannot carry it past its end.
    ///
    /// # Errors
    ///
    /// As [`Cache::mark_announced`]; a write lock held past `wait` is the
    /// store's usual failure (`STORE_FAILED` for SQLite), and the entry stays
    /// unannounced.
    pub fn mark_announced_within(&mut self, seq: Seq, wait: Duration) -> Result<(), CacheError> {
        let at = self.clock.now_utc();
        self.store.mark_announced_within(seq, &at, wait)
    }

    /// The scope's last announced sync-failure cause.
    ///
    /// Like every verb that names a scope, the alert verbs refuse a name
    /// that is not registered with `SCOPE_UNKNOWN` and register nothing
    /// (ADR-BDC-0013; the exception of its amendment 1 ended in sprint b-8,
    /// which removed the tick's phase files).
    ///
    /// # Errors
    ///
    /// `SCOPE_UNKNOWN`; store errors.
    pub fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        ready::require_known(self, s)?;
        self.store.sync_alert(s)
    }

    /// Sets or clears the scope's sync-failure cause.
    ///
    /// # Errors
    ///
    /// `SCOPE_UNKNOWN`; store errors.
    pub fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError> {
        ready::require_known(self, s)?;
        self.store.set_sync_alert(s, cause)
    }

    /// [`Cache::set_sync_alert`], waiting at most `wait` for a write lock
    /// another process holds (lead ruling S33), as
    /// [`Cache::mark_announced_within`].
    ///
    /// # Errors
    ///
    /// `SCOPE_UNKNOWN`; as [`Cache::mark_announced_within`]; the alert is
    /// left as it was.
    pub fn set_sync_alert_within(
        &mut self,
        s: &ScopeId,
        cause: Option<&str>,
        wait: Duration,
    ) -> Result<(), CacheError> {
        ready::require_known(self, s)?;
        self.store.set_sync_alert_within(s, cause, wait)
    }

    /// Every pending journal entry, by seq (`cache journal list --state
    /// pending`; lead ruling S66).
    ///
    /// A store read only: no bd call.
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.store.pending_entries()
    }

    /// The backend check on its own (`cache health --probe`; lead ruling S66):
    /// one `fingerprint` call under `hard`, then the rebuild rule, exactly as
    /// a sync or a non-empty drain starts.
    ///
    /// Returns what the store did with the fingerprint (`Rebuilt` when the
    /// schema changed: cached rows dropped, journal kept) and `model_current`,
    /// whether the live schema is the pinned one.
    ///
    /// # Errors
    ///
    /// `BD_UNAVAILABLE` when `hard` cuts the call or bd is unreachable (the
    /// latter is recorded as an outage; the check never clears one);
    /// `BD_VERSION_FAILED` / `BD_SCHEMA_FAILED`; store errors.
    pub fn backend_check(
        &mut self,
        hard: Deadline,
    ) -> Result<(bd_cache_model::FingerprintOutcome, bool), CacheError> {
        backend_check::run(self, hard)
            .map_err(|f| f.into_error("bd version / bd schema: no answer before the deadline"))
    }

    /// Pending and unannounced-escalated counts (`cache health` `open_work`).
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn open_work(&self) -> Result<OpenWork, CacheError> {
        self.store.open_work()
    }
}

/// Test hooks onto the crate-private helpers (feature `testing`).
///
/// Lets the helper tests (`tests/helpers_*.rs`) and the wave-2 sprints'
/// tests drive `call`, the outage record, `view::current_view`,
/// `backend_check::run` and `miss::fill` directly, and reach the parts a
/// test built the cache from. Not for production use.
#[cfg(feature = "testing")]
pub mod testing {
    use std::time::Duration;

    use bd_cache_model::{
        CacheError, CacheStore, CallFailure, CallKind, DrainLock, FingerprintOutcome, Freshness,
    };
    use sc_beads::{Bead, BeadId, BeadsCacheError, CacheBackend, Clock, Deadline, ProjectRef};

    use crate::Cache;

    /// Runs `f` through the `call` helper (cap, classification, outage record).
    ///
    /// # Errors
    ///
    /// The classified [`CallFailure`] when `f` fails.
    pub fn call<S: CacheStore, L: DrainLock, B: CacheBackend, T>(
        cache: &mut Cache<S, L, B>,
        kind: CallKind,
        hard: Deadline,
        f: impl FnOnce(&B, &ProjectRef, Deadline) -> Result<T, BeadsCacheError>,
    ) -> Result<T, CallFailure> {
        crate::outage::call(cache, kind, hard, f)
    }

    /// Records an unreachable backend (keeps the outage start).
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn note_unavailable<S: CacheStore, L: DrainLock, B: CacheBackend>(
        cache: &mut Cache<S, L, B>,
        err: &str,
    ) -> Result<(), CacheError> {
        crate::outage::note_unavailable(cache, err)
    }

    /// Clears the outage.
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn note_ok<S: CacheStore, L: DrainLock, B: CacheBackend>(
        cache: &mut Cache<S, L, B>,
    ) -> Result<(), CacheError> {
        crate::outage::note_ok(cache)
    }

    /// `view::current_view`.
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn current_view<S: CacheStore, L: DrainLock, B: CacheBackend>(
        cache: &Cache<S, L, B>,
        id: &BeadId,
    ) -> Result<Option<(Bead, Freshness)>, CacheError> {
        crate::view::current_view(cache, id)
    }

    /// `backend_check::run`.
    ///
    /// # Errors
    ///
    /// The classified [`CallFailure`].
    pub fn backend_check<S: CacheStore, L: DrainLock, B: CacheBackend>(
        cache: &mut Cache<S, L, B>,
        hard: Deadline,
    ) -> Result<(FingerprintOutcome, bool), CallFailure> {
        crate::backend_check::run(cache, hard)
    }

    /// `miss::fill`.
    ///
    /// # Errors
    ///
    /// `BD_UNAVAILABLE` when the fill is cut or the backend is unreachable;
    /// the mapped backend error; store errors.
    pub fn miss_fill<S: CacheStore, L: DrainLock, B: CacheBackend>(
        cache: &mut Cache<S, L, B>,
        id: &BeadId,
        timeout: Option<Duration>,
    ) -> Result<Option<Bead>, CacheError> {
        crate::miss::fill(cache, id, timeout)
    }

    /// The store the cache owns.
    #[must_use]
    pub fn store<S: CacheStore, L: DrainLock, B: CacheBackend>(cache: &Cache<S, L, B>) -> &S {
        &cache.store
    }

    /// The store, mutably.
    #[must_use]
    pub fn store_mut<S: CacheStore, L: DrainLock, B: CacheBackend>(
        cache: &mut Cache<S, L, B>,
    ) -> &mut S {
        &mut cache.store
    }

    /// The backend the cache owns.
    #[must_use]
    pub fn backend<S: CacheStore, L: DrainLock, B: CacheBackend>(cache: &Cache<S, L, B>) -> &B {
        &cache.backend
    }

    /// The cache's clock.
    #[must_use]
    pub fn clock<S: CacheStore, L: DrainLock, B: CacheBackend>(
        cache: &Cache<S, L, B>,
    ) -> &dyn Clock {
        &*cache.clock
    }
}
