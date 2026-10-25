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
    DrainReport, GetReply, IdemKey, JournalEntry, ListReply, OpenWork, PruneReport, ReadyReply,
    RowView, ScopeId, Seq, StatusReply, SyncReport, WriteReply,
};
use sc_beads::{Actor, BeadId, CacheBackend, Clock, Deadline, Guards};

mod backend_check;
mod drain;
mod get;
mod list;
mod miss;
mod outage;
mod prune;
mod ready;
mod refresh;
mod resolve;
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
    /// Builds a cache and registers every configured scope with the store.
    ///
    /// The backend must have been built with the same `clock`.
    ///
    /// # Errors
    ///
    /// The store's error when registering a scope fails.
    pub fn new(
        config: CacheConfig,
        mut store: S,
        lock: L,
        backend: B,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, CacheError> {
        for s in &config.scopes {
            store.ensure_scope(s)?;
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
    /// `PRUNE_BLOCKED`; store errors.
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
    /// # Errors
    ///
    /// Store errors.
    pub fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        self.store.sync_alert(s)
    }

    /// Sets or clears the scope's sync-failure cause.
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError> {
        self.store.set_sync_alert(s, cause)
    }

    /// [`Cache::set_sync_alert`], waiting at most `wait` for a write lock
    /// another process holds (lead ruling S33), as
    /// [`Cache::mark_announced_within`].
    ///
    /// # Errors
    ///
    /// As [`Cache::mark_announced_within`]; the alert is left as it was.
    pub fn set_sync_alert_within(
        &mut self,
        s: &ScopeId,
        cause: Option<&str>,
        wait: Duration,
    ) -> Result<(), CacheError> {
        self.store.set_sync_alert_within(s, cause, wait)
    }

    /// Pending and unannounced-escalated counts (the stop rule).
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
