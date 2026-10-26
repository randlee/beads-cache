//! [`Counting`]: a `CacheStore` that counts what the cache reads from it.
//!
//! "Touched" is the number of store calls plus the number of items they
//! return (rows, ids, entries, blocked rows), so a read that loads a whole
//! scope shows up as a count that grows with the scope. It is a
//! deterministic stand-in for latency (NFR-BDC-0001).
//!
//! It also records the `wait` of every bounded marker write
//! (`mark_announced_within`, `set_sync_alert_within`, lead ruling S33), so a
//! test can check what the tick passed, and can be armed to fail the next
//! bounded write with `BD_CACHE.STORE_FAILED` (lead ruling S35, QA7-003).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bd_cache_model::{
    BaseEntry, CacheError, CacheStore, FailingSince, FingerprintOutcome, Freshness, HookState,
    IdemKey, IngestSnapshot, JournalEntry, LeaseHolder, LeaseTake, NewEntry, OpenWork, Outage,
    PruneReport, ReadyBase, Scope, ScopeId, Seq, StoreStatus, SyncState, UpsertReport,
};
use sc_beads::{Actor, Bead, BeadId, BlockedRow, Edge, Fingerprint, Timestamp};

/// Wraps a store and counts what its read methods return.
pub struct Counting<S> {
    inner: S,
    touched: Arc<AtomicU64>,
    waits: Arc<Mutex<Vec<Duration>>>,
    fail_next: Arc<AtomicBool>,
}

impl<S> Counting<S> {
    /// Wraps `inner`; read the count through the returned handle.
    pub fn new(inner: S) -> (Self, Arc<AtomicU64>) {
        let touched = Arc::new(AtomicU64::new(0));
        (
            Self {
                inner,
                touched: Arc::clone(&touched),
                waits: Arc::default(),
                fail_next: Arc::default(),
            },
            touched,
        )
    }

    /// The `wait` of every bounded marker write so far, in order (shared handle).
    pub fn waits(&self) -> Arc<Mutex<Vec<Duration>>> {
        Arc::clone(&self.waits)
    }

    /// A switch (shared handle): set it, and the next bounded write fails
    /// with `BD_CACHE.STORE_FAILED` without reaching the inner store; the
    /// switch then resets itself.
    pub fn fail_next_bounded_write(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.fail_next)
    }

    /// Records `wait`, then fails if the switch is set.
    fn waited(&self, wait: Duration) -> Result<(), CacheError> {
        self.waits.lock().unwrap().push(wait);
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(CacheError::store_failed(
                "bounded write",
                std::io::Error::other("injected"),
            ));
        }
        Ok(())
    }

    fn add(&self, n: usize) {
        self.touched.fetch_add(1 + n as u64, Ordering::SeqCst);
    }

    fn counted<T>(&self, r: Result<Vec<T>, CacheError>) -> Result<Vec<T>, CacheError> {
        if let Ok(v) = &r {
            self.add(v.len());
        }
        r
    }
}

impl<S: CacheStore> CacheStore for Counting<S> {
    fn ensure_scope(&mut self, s: &Scope) -> Result<Scope, CacheError> {
        self.inner.ensure_scope(s)
    }
    fn put_scope(&mut self, s: &Scope) -> Result<(), CacheError> {
        self.inner.put_scope(s)
    }
    fn scope_add(&mut self, s: &Scope) -> Result<(), CacheError> {
        self.inner.scope_add(s)
    }
    fn scope_set(&mut self, s: &Scope) -> Result<(), CacheError> {
        self.inner.scope_set(s)
    }
    fn scope_remove(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        self.inner.scope_remove(s)
    }
    fn scope(&self, s: &ScopeId) -> Result<Option<Scope>, CacheError> {
        let r = self.inner.scope(s);
        self.add(0);
        r
    }
    fn scopes(&self) -> Result<Vec<Scope>, CacheError> {
        self.counted(self.inner.scopes())
    }
    fn scope_revision(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        self.inner.scope_revision(s)
    }
    fn ingest_scope(
        &mut self,
        s: &Scope,
        snapshot: &IngestSnapshot,
        ready: &[BeadId],
        at: &Timestamp,
        revision: Option<&str>,
    ) -> Result<UpsertReport, CacheError> {
        self.inner.ingest_scope(s, snapshot, ready, at, revision)
    }
    fn scope_announce(&mut self, s: &ScopeId, since: &FailingSince) -> Result<(), CacheError> {
        self.inner.scope_announce(s, since)
    }
    fn mark_offered(&mut self, seqs: &[Seq], at: &Timestamp) -> Result<(), CacheError> {
        self.inner.mark_offered(seqs, at)
    }
    fn lease_take(&mut self, h: &LeaseHolder, now: &Timestamp) -> Result<LeaseTake, CacheError> {
        self.inner.lease_take(h, now)
    }
    fn lease_release(&mut self, run_id: &str) -> Result<bool, CacheError> {
        self.inner.lease_release(run_id)
    }
    fn lease(&self) -> Result<Option<LeaseHolder>, CacheError> {
        self.inner.lease()
    }
    fn hook_state(&self) -> Result<HookState, CacheError> {
        self.inner.hook_state()
    }
    fn hook_failed(
        &mut self,
        err: &str,
        at: &Timestamp,
        next: Option<&Timestamp>,
    ) -> Result<HookState, CacheError> {
        self.inner.hook_failed(err, at, next)
    }
    fn hook_succeeded(&mut self) -> Result<(), CacheError> {
        self.inner.hook_succeeded()
    }
    fn get_row(&self, id: &BeadId) -> Result<Option<(Bead, Freshness)>, CacheError> {
        let r = self.inner.get_row(id);
        if let Ok(row) = &r {
            self.add(usize::from(row.is_some()));
        }
        r
    }
    fn rows_in_scope(&self, s: &ScopeId) -> Result<Vec<(Bead, Freshness)>, CacheError> {
        self.counted(self.inner.rows_in_scope(s))
    }
    fn upsert_rows(
        &mut self,
        s: Option<&Scope>,
        rows: &[Bead],
        force: bool,
    ) -> Result<UpsertReport, CacheError> {
        self.inner.upsert_rows(s, rows, force)
    }
    fn stale_rows(&self) -> Result<Vec<BeadId>, CacheError> {
        self.counted(self.inner.stale_rows())
    }
    fn dependents_of(&self, id: &BeadId) -> Result<Vec<Edge>, CacheError> {
        self.counted(self.inner.dependents_of(id))
    }
    fn replace_ready_base(
        &mut self,
        s: &ScopeId,
        blocked: &[BlockedRow],
        ready: &[BeadId],
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        self.inner.replace_ready_base(s, blocked, ready, at)
    }
    fn ready_base(&self, s: &ScopeId) -> Result<Option<ReadyBase>, CacheError> {
        let r = self.inner.ready_base(s);
        if let Ok(Some(b)) = &r {
            self.add(b.blocked.len() + b.ready.len());
        }
        r
    }
    fn base_entry(&self, s: &ScopeId, id: &BeadId) -> Result<Option<BaseEntry>, CacheError> {
        let r = self.inner.base_entry(s, id);
        if let Ok(Some(e)) = &r {
            self.add(e.blocked_by.len());
        }
        r
    }
    fn has_entries_since(&self, at: &Timestamp) -> Result<bool, CacheError> {
        let r = self.inner.has_entries_since(at);
        if r.is_ok() {
            self.add(0);
        }
        r
    }
    fn mark_ready_base_stale(&mut self, s: &ScopeId) -> Result<(), CacheError> {
        self.inner.mark_ready_base_stale(s)
    }
    fn entry_by_key(&self, k: &IdemKey) -> Result<Option<JournalEntry>, CacheError> {
        self.inner.entry_by_key(k)
    }
    fn entry_by_seq(&self, seq: Seq) -> Result<Option<JournalEntry>, CacheError> {
        self.inner.entry_by_seq(seq)
    }
    fn insert_entry(&mut self, e: NewEntry, at: &Timestamp) -> Result<Seq, CacheError> {
        self.inner.insert_entry(e, at)
    }
    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.counted(self.inner.pending_entries())
    }
    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError> {
        self.counted(self.inner.pending_for(id))
    }
    fn applied_since(&self, at: &Timestamp) -> Result<Vec<JournalEntry>, CacheError> {
        self.counted(self.inner.applied_since(at))
    }
    fn apply_with_readback(
        &mut self,
        seq: Seq,
        row: Option<&Bead>,
        err: Option<&str>,
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        self.inner.apply_with_readback(seq, row, err, at)
    }
    fn record_attempt(&mut self, seq: Seq, err: &str, at: &Timestamp) -> Result<u32, CacheError> {
        self.inner.record_attempt(seq, err, at)
    }
    fn escalate(&mut self, seq: Seq, reason: &str, at: &Timestamp) -> Result<(), CacheError> {
        self.inner.escalate(seq, reason, at)
    }
    fn resolve(
        &mut self,
        seq: Seq,
        note: &str,
        actor: &Actor,
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        self.inner.resolve(seq, note, actor, at)
    }
    fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.counted(self.inner.unannounced_escalations())
    }
    fn mark_announced(&mut self, seq: Seq, at: &Timestamp) -> Result<(), CacheError> {
        self.inner.mark_announced(seq, at)
    }
    fn mark_announced_within(
        &mut self,
        seq: Seq,
        at: &Timestamp,
        wait: Duration,
    ) -> Result<(), CacheError> {
        self.waited(wait)?;
        self.inner.mark_announced_within(seq, at, wait)
    }
    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError> {
        self.inner.record_fingerprint(f)
    }
    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError> {
        self.inner.sync_state(s)
    }
    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError> {
        self.inner.record_sync(st)
    }
    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        self.inner.sync_alert(s)
    }
    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError> {
        self.inner.set_sync_alert(s, cause)
    }
    fn set_sync_alert_within(
        &mut self,
        s: &ScopeId,
        cause: Option<&str>,
        wait: Duration,
    ) -> Result<(), CacheError> {
        self.waited(wait)?;
        self.inner.set_sync_alert_within(s, cause, wait)
    }
    fn outage(&self) -> Result<Option<Outage>, CacheError> {
        self.inner.outage()
    }
    fn set_outage(&mut self, o: Option<&Outage>) -> Result<(), CacheError> {
        self.inner.set_outage(o)
    }
    fn prune_scope(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        self.inner.prune_scope(s)
    }
    fn open_work(&self) -> Result<OpenWork, CacheError> {
        self.inner.open_work()
    }
    fn status(&self) -> Result<StoreStatus, CacheError> {
        self.inner.status()
    }
}
