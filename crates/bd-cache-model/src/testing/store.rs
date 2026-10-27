//! [`FakeStore`] and [`FakeDrainLock`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use sc_beads::{Actor, Bead, BeadId, BlockedRow, Edge, Fingerprint, Timestamp, rfc3339_to_unix};

use super::lock;
use crate::budget::check_lease_expiry;
use crate::error::{CacheError, PreconditionReason};
use crate::ingest::IngestSnapshot;
use crate::journal::{
    IdemKey, JournalEntry, JournalState, NewEntry, Seq, error_code_prefix, resolve_note,
};
use crate::op::normalize;
use crate::reply::{JournalCounts, PruneReport};
use crate::scope::{MAX_REGISTERED_SCOPES, Scope, ScopeFilter, ScopeId};
use crate::state::{
    FailingSince, FingerprintOutcome, Freshness, HookState, LeaseHolder, LeaseTake, OpenWork,
    Outage, PendingSummary, ReadyBase, RunId, ScopeStatus, StoreStatus, SyncOutcome, SyncRun,
    SyncRunEnd, SyncState, UpsertReport, clip_hook_error,
};
use crate::store::{CacheStore, DrainGuard, DrainLock};

const META_SHA: &str = "bd_schema_sha256";
const META_VERSION: &str = "bd_version";
const META_SCHEMA_VERSION: &str = "bd_schema_version";
const META_OUTAGE_SINCE: &str = "outage_since";
const META_OUTAGE_ERROR: &str = "outage_error";

/// True when `stored` is a later instant than `incoming` (RFC 3339 compared by
/// instant at second precision). When either value does not parse, the stored
/// row is not newer, so the incoming row is stored; text is never ordered.
fn newer(stored: &str, incoming: &str) -> bool {
    match (rfc3339_to_unix(stored), rfc3339_to_unix(incoming)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// Stores `err` as the entry's `last_error`, with its code.
fn set_error(e: &mut JournalEntry, err: Option<&str>) {
    e.last_error = err.map(str::to_owned);
    e.last_error_code = err.and_then(error_code_prefix).map(str::to_owned);
}

fn alert_key(s: &ScopeId) -> String {
    format!("sync_alert:{}", s.name())
}

#[derive(Debug, Default)]
struct State {
    /// The registry: each scope's filter (`None` = a phase-a scope) and revision.
    scopes: BTreeMap<ScopeId, (Option<ScopeFilter>, Option<String>)>,
    lease: Option<LeaseHolder>,
    hook: HookState,
    /// Each sync row's run record, by row name.
    sync_runs: BTreeMap<String, SyncRun>,
    /// Normalized rows without `dependencies` (those live in `edges`) and their stale flag.
    rows: BTreeMap<BeadId, (Bead, bool)>,
    edges: BTreeSet<Edge>,
    members: BTreeMap<BeadId, BTreeSet<ScopeId>>,
    ready: BTreeMap<ScopeId, ReadyBase>,
    journal: BTreeMap<Seq, JournalEntry>,
    last_seq: i64,
    meta: BTreeMap<String, String>,
    sync: BTreeMap<ScopeId, SyncState>,
    /// How many times the whole pending backlog was loaded (`pending_entries`).
    pending_loads: u64,
}

impl State {
    fn row(&self, id: &BeadId) -> Option<(Bead, Freshness)> {
        self.rows.get(id).map(|(b, stale)| {
            let mut b = b.clone();
            b.dependencies = self
                .edges
                .iter()
                .filter(|e| e.issue_id == *id)
                .cloned()
                .collect();
            (
                b,
                if *stale {
                    Freshness::Stale
                } else {
                    Freshness::Fresh
                },
            )
        })
    }

    fn store(&mut self, row: &Bead) {
        let mut b = normalize(row);
        self.edges.retain(|e| e.issue_id != b.id);
        self.edges.extend(std::mem::take(&mut b.dependencies));
        self.rows.insert(b.id.clone(), (b, false));
    }

    fn delete_bead(&mut self, id: &BeadId) {
        self.rows.remove(id);
        self.edges.retain(|e| e.issue_id != *id);
        self.members.remove(id);
    }

    fn entry_mut(&mut self, seq: Seq) -> Result<&mut JournalEntry, CacheError> {
        self.journal
            .get_mut(&seq)
            .ok_or_else(|| CacheError::NotFound {
                detail: format!("journal entry {seq} is absent"),
            })
    }

    fn has_open_entry(&self, id: &BeadId) -> bool {
        self.journal.values().any(|e| {
            e.bead == *id && matches!(e.state, JournalState::Pending | JournalState::Escalated)
        })
    }

    fn outage(&self) -> Option<Outage> {
        self.meta
            .get(META_OUTAGE_SINCE)
            .and_then(|since| Timestamp::parse(since).ok())
            .map(|since| Outage {
                since,
                last_error: self
                    .meta
                    .get(META_OUTAGE_ERROR)
                    .cloned()
                    .unwrap_or_default(),
            })
    }

    fn stale(&self) -> Vec<BeadId> {
        self.rows
            .iter()
            .filter(|(_, (_, stale))| *stale)
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn entries(&self, f: impl Fn(&JournalEntry) -> bool) -> Vec<JournalEntry> {
        self.journal.values().filter(|e| f(e)).cloned().collect()
    }

    /// Drops what a scope's filter derived: its memberships, sync state and
    /// ready base (a changed filter starts the scope again; the rows stay).
    fn reset_scope(&mut self, s: &ScopeId) {
        for m in self.members.values_mut() {
            m.remove(s);
        }
        self.sync.remove(s);
        self.ready.remove(s);
    }

    /// Writes a scope's filter, resetting it when the filter changed.
    fn write_scope(&mut self, s: &Scope) {
        let changed = self
            .scopes
            .get(&s.id)
            .is_some_and(|(filter, _)| *filter != s.filter);
        if changed {
            self.reset_scope(&s.id);
        }
        let revision = self.scopes.get(&s.id).and_then(|(_, r)| r.clone());
        self.scopes
            .insert(s.id.clone(), (s.filter.clone(), revision));
    }

    fn scope(&self, s: &ScopeId) -> Option<Scope> {
        self.scopes.get(s).map(|(filter, _)| Scope {
            id: s.clone(),
            filter: filter.clone(),
        })
    }

    fn prune(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        let open = self
            .journal
            .values()
            .filter(|e| e.scope.as_ref() == Some(s))
            .filter(|e| matches!(e.state, JournalState::Pending | JournalState::Escalated))
            .count();
        if open > 0 {
            return Err(CacheError::PruneBlocked {
                detail: format!("scope {} has {open} pending or escalated writes", s.name()),
            });
        }
        let mut report = PruneReport {
            scope: s.clone(),
            beads_deleted: 0,
            out_of_scope_swept: 0,
        };
        let held: Vec<BeadId> = self
            .members
            .iter()
            .filter(|(_, m)| m.contains(s))
            .map(|(id, _)| id.clone())
            .collect();
        for id in held {
            let last = self.members.get_mut(&id).is_some_and(|m| {
                m.remove(s);
                m.is_empty()
            });
            if last && !self.has_open_entry(&id) {
                self.delete_bead(&id);
                report.beads_deleted += 1;
            }
        }
        self.journal.retain(|_, e| e.scope.as_ref() != Some(s));
        self.sync.remove(s);
        self.ready.remove(s);
        let unscoped: Vec<BeadId> = self
            .rows
            .keys()
            .filter(|id| self.members.get(*id).is_none_or(BTreeSet::is_empty))
            .filter(|id| !self.has_open_entry(id))
            .cloned()
            .collect();
        for id in unscoped {
            self.delete_bead(&id);
            report.out_of_scope_swept += 1;
        }
        Ok(report)
    }
}

/// How a registration treats a name that is (or is not) registered: the
/// one registration body the four registry calls share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Register {
    /// `ensure_scope`: register when absent; a registered scope is kept.
    IfAbsent,
    /// `put_scope`: add or set.
    Put,
    /// `scope_add`: refuse a registered name.
    Add,
    /// `scope_set`: refuse an absent name.
    Set,
}

impl State {
    /// `SCOPE_UNKNOWN` for `s`, listing the registered scopes.
    fn unknown(&self, s: &ScopeId) -> CacheError {
        CacheError::scope_unknown(
            format!("scope {s} is not registered"),
            self.scopes.keys().cloned().collect(),
        )
    }

    /// The one registration rule; returns the scope registered afterwards.
    fn register(&mut self, s: &Scope, mode: Register) -> Result<Scope, CacheError> {
        let known = self.scope(&s.id);
        match (&known, mode) {
            (Some(k), Register::IfAbsent) => return Ok(k.clone()),
            (Some(_), Register::Add) => {
                return Err(CacheError::BadArgs {
                    detail: format!("scope {} is already registered; use scopes set", s.id),
                });
            }
            (None, Register::Set) => return Err(self.unknown(&s.id)),
            (None, _) if self.scopes.len() >= MAX_REGISTERED_SCOPES => {
                return Err(CacheError::registry_full(&s.id));
            }
            _ => {}
        }
        self.write_scope(s);
        Ok(s.clone())
    }
}

/// In-memory [`CacheStore`] keeping every rule of the port.
///
/// Clones share one store: hand a clone to a second `Cache` to model two
/// processes on one file, or keep a clone to inspect state a test drove.
/// It stores the outage and sync alerts as `meta` keys (`outage_since`,
/// `outage_error`, `sync_alert:<scope>`) as BD-CACHE v3 does.
///
/// # Examples
///
/// ```
/// use bd_cache_model::{CacheStore, Scope, ScopeFilter, ScopeId};
/// use bd_cache_model::testing::FakeStore;
///
/// let mut store = FakeStore::new();
/// let mut b = sc_beads::testing::bead("b1");
/// b.labels = vec!["area".into()];
/// let x = Scope::new(ScopeId::new("x").unwrap(), ScopeFilter::label("area").unwrap());
/// store.upsert_rows(Some(&x), &[b], false).unwrap();
/// assert_eq!(store.rows_in_scope(&x.id).unwrap().len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct FakeStore {
    state: Arc<Mutex<State>>,
}

impl FakeStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every journal entry, by seq (for assertions).
    #[must_use]
    pub fn journal(&self) -> Vec<JournalEntry> {
        lock(&self.state).journal.values().cloned().collect()
    }

    /// Ids of every stored row (for assertions).
    #[must_use]
    pub fn row_ids(&self) -> Vec<BeadId> {
        lock(&self.state).rows.keys().cloned().collect()
    }

    /// How many times [`CacheStore::pending_entries`] loaded the whole
    /// pending backlog (for assertions that a read stays bounded, as
    /// `SqliteStore`'s [`CacheStore::pending_summary`] does).
    #[must_use]
    pub fn pending_loads(&self) -> u64 {
        lock(&self.state).pending_loads
    }

    /// The scopes holding a bead (for assertions).
    #[must_use]
    pub fn memberships(&self, id: &BeadId) -> Vec<ScopeId> {
        lock(&self.state)
            .members
            .get(id)
            .map(|m| m.iter().cloned().collect())
            .unwrap_or_default()
    }
}

impl CacheStore for FakeStore {
    fn ensure_scope(&mut self, s: &Scope) -> Result<Scope, CacheError> {
        lock(&self.state).register(s, Register::IfAbsent)
    }

    fn put_scope(&mut self, s: &Scope) -> Result<(), CacheError> {
        lock(&self.state).register(s, Register::Put).map(drop)
    }

    fn scope_add(&mut self, s: &Scope) -> Result<(), CacheError> {
        lock(&self.state).register(s, Register::Add).map(drop)
    }

    fn scope_set(&mut self, s: &Scope) -> Result<(), CacheError> {
        lock(&self.state).register(s, Register::Set).map(drop)
    }

    fn scope_remove(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        let mut st = lock(&self.state);
        if !st.scopes.contains_key(s) {
            return Err(st.unknown(s));
        }
        let report = st.prune(s)?;
        st.scopes.remove(s);
        Ok(report)
    }

    fn scope(&self, s: &ScopeId) -> Result<Option<Scope>, CacheError> {
        Ok(lock(&self.state).scope(s))
    }

    fn scopes(&self) -> Result<Vec<Scope>, CacheError> {
        let st = lock(&self.state);
        Ok(st.scopes.keys().filter_map(|id| st.scope(id)).collect())
    }

    fn scopes_of(&self, id: &BeadId) -> Result<Vec<Scope>, CacheError> {
        let st = lock(&self.state);
        Ok(st
            .members
            .get(id)
            .into_iter()
            .flatten()
            .filter_map(|s| st.scope(s))
            .collect())
    }

    fn scope_revision(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        Ok(lock(&self.state)
            .scopes
            .get(s)
            .and_then(|(_, revision)| revision.clone()))
    }

    fn ingest_scope(
        &mut self,
        s: &ScopeId,
        snapshot: &IngestSnapshot,
        ready: &[BeadId],
        at: &Timestamp,
        revision: Option<&str>,
    ) -> Result<UpsertReport, CacheError> {
        let rows = snapshot.rows();
        // One guard: the registration check and the write are one step.
        let mut st = lock(&self.state);
        let Some(s) = st.scope(s) else {
            return Err(st.unknown(s));
        };
        let mut report = UpsertReport::default();
        let mut members = BTreeSet::new();
        for row in rows {
            if s.admits(row) {
                members.insert(row.id.clone());
            }
            let existing = st
                .rows
                .get(&row.id)
                .map(|(b, _)| (b.updated_at.clone(), b.status.clone()));
            if let Some((updated_at, _)) = &existing
                && newer(updated_at, &row.updated_at)
            {
                report.skipped_newer.push(row.id.clone());
                continue;
            }
            if let Some((_, status)) = &existing
                && *status != row.status
            {
                report.status_changed.push(row.id.clone());
            }
            st.store(row);
            report.stored.push(row.id.clone());
        }
        for (id, m) in &mut st.members {
            if !members.contains(id) {
                m.remove(&s.id);
            }
        }
        for id in &members {
            st.members
                .entry(id.clone())
                .or_default()
                .insert(s.id.clone());
        }
        let blocked: Vec<BlockedRow> = snapshot
            .blocked()
            .iter()
            .filter(|b| members.contains(&b.id))
            .cloned()
            .collect();
        let ready: Vec<BeadId> = ready
            .iter()
            .filter(|id| members.contains(*id))
            .cloned()
            .collect();
        st.ready.insert(
            s.id.clone(),
            ReadyBase {
                blocked,
                ready,
                fetched_at: at.clone(),
                stale: false,
            },
        );
        st.sync.insert(
            s.id.clone(),
            SyncState {
                scope: s.id.clone(),
                last_started_at: Some(at.clone()),
                last_ok_at: Some(at.clone()),
                last_error: None,
                failing_since: None,
                notified_since: None,
            },
        );
        st.scopes.insert(
            s.id.clone(),
            (s.filter.clone(), revision.map(str::to_owned)),
        );
        Ok(report)
    }

    fn scope_announce(&mut self, s: &ScopeId, since: &FailingSince) -> Result<(), CacheError> {
        let mut st = lock(&self.state);
        let state = st.sync.get_mut(s);
        match state {
            Some(state) if state.failing_since.as_ref() == Some(since) => {
                state.notified_since = Some(since.clone());
                Ok(())
            }
            other => Err(stale_since(
                s,
                since,
                other.and_then(|st| st.failing_since.as_ref()),
            )),
        }
    }

    fn mark_offered(&mut self, seqs: &[Seq], at: &Timestamp) -> Result<(), CacheError> {
        let mut st = lock(&self.state);
        for seq in seqs {
            let e = st.entry_mut(*seq)?;
            if e.first_offered_at.is_none() {
                e.first_offered_at = Some(at.clone());
            }
        }
        Ok(())
    }

    fn lease_take(
        &mut self,
        holder: &LeaseHolder,
        now: &Timestamp,
    ) -> Result<LeaseTake, CacheError> {
        check_lease_expiry(&holder.expires_at, now)?;
        let mut st = lock(&self.state);
        let took = match st.lease.take() {
            None => LeaseTake::Taken,
            Some(live) if live.expires_at.unix() > now.unix() => {
                let held = live.clone();
                st.lease = Some(live);
                return Ok(LeaseTake::Held(held));
            }
            Some(expired) => LeaseTake::TookOver(expired),
        };
        st.lease = Some(holder.clone());
        Ok(took)
    }

    fn lease_release(&mut self, run_id: &RunId) -> Result<bool, CacheError> {
        let mut st = lock(&self.state);
        if st.lease.as_ref().is_some_and(|l| l.run_id == *run_id) {
            st.lease = None;
            return Ok(true);
        }
        Ok(false)
    }

    fn lease(&self) -> Result<Option<LeaseHolder>, CacheError> {
        Ok(lock(&self.state).lease.clone())
    }

    fn hook_state(&self) -> Result<HookState, CacheError> {
        Ok(lock(&self.state).hook.clone())
    }

    fn hook_failed(
        &mut self,
        err: &str,
        at: &Timestamp,
        next_attempt_at: Option<&Timestamp>,
    ) -> Result<HookState, CacheError> {
        let mut st = lock(&self.state);
        let h = &mut st.hook;
        h.consecutive_failures = h.consecutive_failures.saturating_add(1);
        if h.first_failure_at.is_none() {
            h.first_failure_at = Some(at.clone());
        }
        h.last_error = Some(clip_hook_error(err).to_owned());
        h.next_attempt_at = next_attempt_at.cloned();
        Ok(h.clone())
    }

    fn hook_succeeded(&mut self) -> Result<(), CacheError> {
        lock(&self.state).hook = HookState::default();
        Ok(())
    }

    fn sync_run(&self, row: &str) -> Result<Option<SyncRun>, CacheError> {
        Ok(lock(&self.state).sync_runs.get(row).cloned())
    }

    fn sync_run_started(
        &mut self,
        row: &str,
        run_id: &RunId,
        at: &Timestamp,
    ) -> Result<SyncRun, CacheError> {
        let mut st = lock(&self.state);
        let record = SyncRun::started(st.sync_runs.get(row), run_id, at);
        st.sync_runs.insert(row.to_owned(), record.clone());
        Ok(record)
    }

    fn sync_run_finished(
        &mut self,
        row: &str,
        run_id: &RunId,
        at: &Timestamp,
        end: SyncRunEnd,
    ) -> Result<bool, CacheError> {
        let mut st = lock(&self.state);
        match st.sync_runs.get_mut(row) {
            Some(record) if record.run_id == *run_id => {
                *record = record.finished(at, end);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn get_row(&self, id: &BeadId) -> Result<Option<(Bead, Freshness)>, CacheError> {
        Ok(lock(&self.state).row(id))
    }

    fn rows_in_scope(&self, s: &ScopeId) -> Result<Vec<(Bead, Freshness)>, CacheError> {
        let st = lock(&self.state);
        Ok(st
            .members
            .iter()
            .filter(|(_, m)| m.contains(s))
            .filter_map(|(id, _)| st.row(id))
            .collect())
    }

    fn upsert_rows(
        &mut self,
        s: Option<&Scope>,
        rows: &[Bead],
        force: bool,
    ) -> Result<UpsertReport, CacheError> {
        let mut st = lock(&self.state);
        let mut report = UpsertReport::default();
        for row in rows {
            let existing = st
                .rows
                .get(&row.id)
                .map(|(b, _)| (b.updated_at.clone(), b.status.clone()));
            if let Some((updated_at, _)) = &existing
                && !force
                && newer(updated_at, &row.updated_at)
            {
                report.skipped_newer.push(row.id.clone());
                continue;
            }
            if let Some((_, status)) = &existing
                && *status != row.status
            {
                report.status_changed.push(row.id.clone());
            }
            st.store(row);
            if let Some(s) = s {
                let m = st.members.entry(row.id.clone()).or_default();
                if s.holds(row) {
                    m.insert(s.id.clone());
                } else {
                    m.remove(&s.id);
                }
            }
            report.stored.push(row.id.clone());
        }
        Ok(report)
    }

    fn stale_rows(&self) -> Result<Vec<BeadId>, CacheError> {
        Ok(lock(&self.state).stale())
    }

    fn dependents_of(&self, id: &BeadId) -> Result<Vec<Edge>, CacheError> {
        Ok(lock(&self.state)
            .edges
            .iter()
            .filter(|e| e.depends_on_id == *id)
            .cloned()
            .collect())
    }

    fn replace_ready_base(
        &mut self,
        s: &ScopeId,
        blocked: &[BlockedRow],
        ready: &[BeadId],
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        lock(&self.state).ready.insert(
            s.clone(),
            ReadyBase {
                blocked: blocked.to_vec(),
                ready: ready.to_vec(),
                fetched_at: at.clone(),
                stale: false,
            },
        );
        Ok(())
    }

    fn ready_base(&self, s: &ScopeId) -> Result<Option<ReadyBase>, CacheError> {
        Ok(lock(&self.state).ready.get(s).cloned())
    }

    fn mark_ready_base_stale(&mut self, s: &ScopeId) -> Result<(), CacheError> {
        if let Some(b) = lock(&self.state).ready.get_mut(s) {
            b.stale = true;
        }
        Ok(())
    }

    fn entry_by_key(&self, k: &IdemKey) -> Result<Option<JournalEntry>, CacheError> {
        Ok(lock(&self.state)
            .journal
            .values()
            .find(|e| e.idem_key == *k)
            .cloned())
    }

    fn entry_by_seq(&self, seq: Seq) -> Result<Option<JournalEntry>, CacheError> {
        Ok(lock(&self.state).journal.get(&seq).cloned())
    }

    fn insert_entry(&mut self, e: NewEntry, at: &Timestamp) -> Result<Seq, CacheError> {
        let mut st = lock(&self.state);
        if let Some(held) = st.journal.values().find(|j| j.idem_key == e.idem_key) {
            return Err(CacheError::IdemConflict {
                detail: format!(
                    "idem_key {} already journals entry {}",
                    e.idem_key, held.seq
                ),
                seq: held.seq,
            });
        }
        st.last_seq += 1;
        let seq = Seq::new(st.last_seq);
        st.journal.insert(
            seq,
            JournalEntry {
                seq,
                idem_key: e.idem_key,
                scope: e.scope,
                bead: e.bead,
                op: e.op,
                precondition: e.precondition,
                actor: e.actor,
                state: JournalState::Pending,
                attempts: 0,
                last_error: None,
                last_error_code: None,
                note: None,
                changed: e.changed,
                announced_at: None,
                created_at: at.clone(),
                applied_at: None,
                first_offered_at: None,
            },
        );
        Ok(seq)
    }

    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError> {
        let mut st = lock(&self.state);
        st.pending_loads = st.pending_loads.saturating_add(1);
        Ok(st.entries(|e| e.state == JournalState::Pending))
    }

    fn pending_summary(&self) -> Result<PendingSummary, CacheError> {
        // Bounded like `SqliteStore`'s: counted in place, the backlog not loaded.
        let st = lock(&self.state);
        let pending = st
            .journal
            .values()
            .filter(|e| e.state == JournalState::Pending);
        Ok(PendingSummary {
            count: u64::try_from(pending.clone().count()).unwrap_or(u64::MAX),
            oldest_created_at: pending
                .map(|e| e.created_at.clone())
                .min_by_key(Timestamp::unix),
        })
    }

    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError> {
        Ok(lock(&self.state).entries(|e| e.state == JournalState::Pending && e.bead == *id))
    }

    fn applied_since(&self, at: &Timestamp) -> Result<Vec<JournalEntry>, CacheError> {
        Ok(lock(&self.state).entries(|e| {
            e.state == JournalState::Applied && e.applied_at.as_ref().is_some_and(|t| t >= at)
        }))
    }

    fn apply_with_readback(
        &mut self,
        seq: Seq,
        row: Option<&Bead>,
        err: Option<&str>,
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        let mut st = lock(&self.state);
        let bead = st.entry_mut(seq)?.bead.clone();
        match row {
            Some(r) => st.store(r),
            None => {
                if let Some((_, stale)) = st.rows.get_mut(&bead) {
                    *stale = true;
                }
            }
        }
        let e = st.entry_mut(seq)?;
        e.state = JournalState::Applied;
        set_error(e, err);
        e.applied_at = Some(at.clone());
        Ok(())
    }

    fn record_attempt(&mut self, seq: Seq, err: &str, _at: &Timestamp) -> Result<u32, CacheError> {
        let mut st = lock(&self.state);
        let e = st.entry_mut(seq)?;
        e.attempts += 1;
        set_error(e, Some(err));
        Ok(e.attempts)
    }

    fn escalate(&mut self, seq: Seq, reason: &str, _at: &Timestamp) -> Result<(), CacheError> {
        let mut st = lock(&self.state);
        let e = st.entry_mut(seq)?;
        e.state = JournalState::Escalated;
        set_error(e, Some(reason));
        Ok(())
    }

    fn resolve(
        &mut self,
        seq: Seq,
        note: &str,
        actor: &Actor,
        _at: &Timestamp,
    ) -> Result<(), CacheError> {
        let mut st = lock(&self.state);
        let e = st
            .journal
            .get_mut(&seq)
            .filter(|e| e.state == JournalState::Escalated);
        let Some(e) = e else {
            let state = st.journal.get(&seq).map_or("absent", |e| e.state.as_str());
            return Err(CacheError::NotEscalated {
                detail: format!("journal entry {seq} is {state}"),
            });
        };
        e.state = JournalState::Resolved;
        e.note = Some(resolve_note(actor, note));
        Ok(())
    }

    fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError> {
        Ok(lock(&self.state)
            .entries(|e| e.state == JournalState::Escalated && e.announced_at.is_none()))
    }

    fn mark_announced(&mut self, seq: Seq, at: &Timestamp) -> Result<(), CacheError> {
        let mut st = lock(&self.state);
        st.entry_mut(seq)?.announced_at = Some(at.clone());
        Ok(())
    }

    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError> {
        let mut st = lock(&self.state);
        let outcome = match st.meta.get(META_SHA) {
            None => FingerprintOutcome::Recorded,
            Some(sha) if sha == f.schema_sha256.as_str() => {
                return Ok(FingerprintOutcome::Unchanged);
            }
            Some(_) => FingerprintOutcome::Rebuilt,
        };
        if outcome == FingerprintOutcome::Rebuilt {
            st.rows.clear();
            st.edges.clear();
            st.members.clear();
            st.sync.clear();
            st.ready.clear();
        }
        st.meta.insert(META_SHA.into(), f.schema_sha256.to_string());
        st.meta.insert(META_VERSION.into(), f.bd_version.clone());
        st.meta
            .insert(META_SCHEMA_VERSION.into(), f.schema_version.clone());
        Ok(outcome)
    }

    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError> {
        Ok(lock(&self.state)
            .sync
            .get(s)
            .cloned()
            .unwrap_or_else(|| SyncState::new(s.clone())))
    }

    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError> {
        let mut state = lock(&self.state);
        let notified = state
            .sync
            .get(&st.scope)
            .and_then(|k| k.notified_since.clone());
        state.sync.insert(
            st.scope.clone(),
            SyncState {
                notified_since: notified,
                ..st.clone()
            },
        );
        Ok(())
    }

    fn record_sync_result(
        &mut self,
        s: &ScopeId,
        outcome: &SyncOutcome,
        started: &Timestamp,
    ) -> Result<SyncState, CacheError> {
        // One guard: the read and the write are one step.
        let mut st = lock(&self.state);
        let state = st
            .sync
            .entry(s.clone())
            .or_insert_with(|| SyncState::new(s.clone()));
        state.last_started_at = Some(started.clone());
        match outcome {
            SyncOutcome::Ok => {
                state.last_ok_at = Some(started.clone());
                state.last_error = None;
                state.record_result(true, started);
            }
            SyncOutcome::Failed(err) => {
                state.last_error = Some(err.clone());
                state.record_result(false, started);
            }
            SyncOutcome::Deferred => {}
        }
        Ok(state.clone())
    }

    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        Ok(lock(&self.state).meta.get(&alert_key(s)).cloned())
    }

    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError> {
        let mut st = lock(&self.state);
        match cause {
            Some(c) => st.meta.insert(alert_key(s), c.to_owned()),
            None => st.meta.remove(&alert_key(s)),
        };
        Ok(())
    }

    fn outage(&self) -> Result<Option<Outage>, CacheError> {
        Ok(lock(&self.state).outage())
    }

    fn set_outage(&mut self, o: Option<&Outage>) -> Result<(), CacheError> {
        let mut st = lock(&self.state);
        if let Some(o) = o {
            st.meta
                .insert(META_OUTAGE_SINCE.into(), o.since.to_string());
            st.meta
                .insert(META_OUTAGE_ERROR.into(), o.last_error.clone());
        } else {
            st.meta.remove(META_OUTAGE_SINCE);
            st.meta.remove(META_OUTAGE_ERROR);
        }
        Ok(())
    }

    fn prune_scope(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        lock(&self.state).prune(s)
    }

    fn open_work(&self) -> Result<OpenWork, CacheError> {
        let st = lock(&self.state);
        let count =
            |f: &dyn Fn(&JournalEntry) -> bool| st.journal.values().filter(|e| f(e)).count() as u64;
        Ok(OpenWork {
            pending: count(&|e| e.state == JournalState::Pending),
            escalated_unannounced: count(&|e| {
                e.state == JournalState::Escalated && e.announced_at.is_none()
            }),
        })
    }

    fn journal_counts(&self) -> Result<JournalCounts, CacheError> {
        // One guard: a consistent snapshot.
        let st = lock(&self.state);
        let mut counts = JournalCounts::default();
        for e in st.journal.values() {
            counts.add(e.scope.as_ref(), e.state, 1);
        }
        Ok(counts)
    }

    fn status(&self) -> Result<StoreStatus, CacheError> {
        let st = lock(&self.state);
        let mut journal = BTreeMap::new();
        for e in st.journal.values() {
            let phase = e.scope.as_ref().map_or("None", ScopeId::name);
            *journal.entry(format!("{phase}:{}", e.state)).or_insert(0) += 1;
        }
        let names: BTreeSet<ScopeId> = st.scopes.keys().chain(st.sync.keys()).cloned().collect();
        let scopes = names
            .into_iter()
            .map(|s| ScopeStatus {
                beads: st.members.values().filter(|m| m.contains(&s)).count() as u64,
                filter: st.scopes.get(&s).and_then(|(f, _)| f.clone()),
                registered: st.scopes.contains_key(&s),
                sync: st
                    .sync
                    .get(&s)
                    .cloned()
                    .unwrap_or_else(|| SyncState::new(s.clone())),
            })
            .collect();
        // One guard for the whole reply: a consistent snapshot.
        Ok(StoreStatus {
            meta: st.meta.clone(),
            journal,
            scopes,
            escalated: st.entries(|e| e.state == JournalState::Escalated),
            outage: st.outage(),
            stale: st.stale(),
        })
    }
}

/// `PRECONDITION` for a `scope_announce` whose `since` is not the current failure.
fn stale_since(s: &ScopeId, since: &FailingSince, current: Option<&FailingSince>) -> CacheError {
    CacheError::Precondition {
        detail: format!(
            "scope {s}: since {since} is not its current failing_since ({})",
            current.map_or("none", FailingSince::as_str)
        ),
        unmet: BTreeMap::new(),
        reason: PreconditionReason::PreconditionFailed,
        source: None,
    }
}

/// An in-process drain lock; clones share one lock.
#[derive(Debug, Clone, Default)]
pub struct FakeDrainLock {
    held: Arc<AtomicBool>,
}

impl FakeDrainLock {
    /// A free lock.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True while a guard is alive.
    #[must_use]
    pub fn is_held(&self) -> bool {
        self.held.load(Ordering::SeqCst)
    }
}

struct FakeGuard {
    held: Arc<AtomicBool>,
}

impl Drop for FakeGuard {
    fn drop(&mut self) {
        self.held.store(false, Ordering::SeqCst);
    }
}

impl DrainGuard for FakeGuard {}

impl DrainLock for FakeDrainLock {
    fn try_acquire(&self) -> Result<Option<Box<dyn DrainGuard>>, CacheError> {
        // Free -> held in one step: two callers cannot both win.
        if self
            .held
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(None);
        }
        Ok(Some(Box::new(FakeGuard {
            held: Arc::clone(&self.held),
        })))
    }
}
