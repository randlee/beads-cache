//! [`FakeStore`] and [`FakeDrainLock`].

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use sc_beads::{Actor, Bead, BeadId, BlockedRow, Edge, Fingerprint, Timestamp, rfc3339_to_unix};

use super::lock;
use crate::error::CacheError;
use crate::journal::{IdemKey, JournalEntry, JournalState, NewEntry, Seq};
use crate::op::normalize;
use crate::reply::PruneReport;
use crate::scope::ScopeId;
use crate::state::{
    FingerprintOutcome, Freshness, OpenWork, Outage, ReadyBase, ScopeStatus, StoreStatus,
    SyncState, UpsertReport,
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

fn alert_key(s: &ScopeId) -> String {
    format!("sync_alert:{}", s.name())
}

#[derive(Debug, Default)]
struct State {
    scopes: BTreeSet<ScopeId>,
    /// Normalized rows without `dependencies` (those live in `edges`) and their stale flag.
    rows: BTreeMap<BeadId, (Bead, bool)>,
    edges: BTreeSet<Edge>,
    members: BTreeMap<BeadId, BTreeSet<ScopeId>>,
    ready: BTreeMap<ScopeId, ReadyBase>,
    journal: BTreeMap<Seq, JournalEntry>,
    last_seq: i64,
    meta: BTreeMap<String, String>,
    sync: BTreeMap<ScopeId, SyncState>,
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
}

/// In-memory [`CacheStore`] keeping every rule of the port.
///
/// Clones share one store: hand a clone to a second `Cache` to model two
/// processes on one file, or keep a clone to inspect state a test drove.
/// It stores the outage and sync alerts as `meta` keys (`outage_since`,
/// `outage_error`, `sync_alert:<phase>`) as BD-CACHE v3 does.
///
/// # Examples
///
/// ```
/// use bd_cache_model::{CacheStore, ScopeId};
/// use bd_cache_model::testing::FakeStore;
///
/// let mut store = FakeStore::new();
/// let mut b = sc_beads::testing::bead("b1");
/// b.labels = vec!["phase-x".into()];
/// let x = ScopeId::phase("x").unwrap();
/// store.upsert_rows(Some(&x), &[b], false).unwrap();
/// assert_eq!(store.rows_in_scope(&x).unwrap().len(), 1);
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
    fn ensure_scope(&mut self, s: &ScopeId) -> Result<(), CacheError> {
        lock(&self.state).scopes.insert(s.clone());
        Ok(())
    }

    fn scopes(&self) -> Result<Vec<ScopeId>, CacheError> {
        Ok(lock(&self.state).scopes.iter().cloned().collect())
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
        s: Option<&ScopeId>,
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
                    m.insert(s.clone());
                } else {
                    m.remove(s);
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
                announced_at: None,
                created_at: at.clone(),
                applied_at: None,
            },
        );
        Ok(seq)
    }

    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError> {
        Ok(lock(&self.state).entries(|e| e.state == JournalState::Pending))
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
        e.last_error = err.map(str::to_owned);
        e.applied_at = Some(at.clone());
        Ok(())
    }

    fn record_attempt(&mut self, seq: Seq, err: &str, _at: &Timestamp) -> Result<u32, CacheError> {
        let mut st = lock(&self.state);
        let e = st.entry_mut(seq)?;
        e.attempts += 1;
        e.last_error = Some(err.to_owned());
        Ok(e.attempts)
    }

    fn escalate(&mut self, seq: Seq, reason: &str, _at: &Timestamp) -> Result<(), CacheError> {
        let mut st = lock(&self.state);
        let e = st.entry_mut(seq)?;
        e.state = JournalState::Escalated;
        e.last_error = Some(reason.to_owned());
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
        let mut text = e.last_error.take().unwrap_or_default();
        // Writing to a String cannot fail.
        write!(text, " | resolved by {actor}: {note}").unwrap_or_default();
        e.last_error = Some(text);
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
        lock(&self.state).sync.insert(st.scope.clone(), st.clone());
        Ok(())
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
        let mut st = lock(&self.state);
        let open = st
            .journal
            .values()
            .filter(|e| e.scope.as_ref() == Some(s))
            .filter(|e| matches!(e.state, JournalState::Pending | JournalState::Escalated))
            .count();
        if open > 0 {
            return Err(CacheError::PruneBlocked {
                detail: format!("phase {} has {open} pending or escalated writes", s.name()),
            });
        }
        let mut report = PruneReport {
            scope: s.clone(),
            beads_deleted: 0,
            out_of_scope_swept: 0,
        };
        let held: Vec<BeadId> = st
            .members
            .iter()
            .filter(|(_, m)| m.contains(s))
            .map(|(id, _)| id.clone())
            .collect();
        for id in held {
            let last = st.members.get_mut(&id).is_some_and(|m| {
                m.remove(s);
                m.is_empty()
            });
            if last && !st.has_open_entry(&id) {
                st.delete_bead(&id);
                report.beads_deleted += 1;
            }
        }
        st.journal.retain(|_, e| e.scope.as_ref() != Some(s));
        st.sync.remove(s);
        st.ready.remove(s);
        let unscoped: Vec<BeadId> = st
            .rows
            .keys()
            .filter(|id| st.members.get(*id).is_none_or(BTreeSet::is_empty))
            .filter(|id| !st.has_open_entry(id))
            .cloned()
            .collect();
        for id in unscoped {
            st.delete_bead(&id);
            report.out_of_scope_swept += 1;
        }
        Ok(report)
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

    fn status(&self) -> Result<StoreStatus, CacheError> {
        let st = lock(&self.state);
        let mut journal = BTreeMap::new();
        for e in st.journal.values() {
            let phase = e.scope.as_ref().map_or("None", ScopeId::name);
            *journal.entry(format!("{phase}:{}", e.state)).or_insert(0) += 1;
        }
        let names: BTreeSet<ScopeId> = st.scopes.iter().chain(st.sync.keys()).cloned().collect();
        let scopes = names
            .into_iter()
            .map(|s| ScopeStatus {
                beads: st.members.values().filter(|m| m.contains(&s)).count() as u64,
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

/// An in-process drain lock; clones share one lock.
#[derive(Debug, Clone, Default)]
pub struct FakeDrainLock {
    held: Arc<Mutex<bool>>,
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
        *lock(&self.held)
    }
}

struct FakeGuard {
    held: Arc<Mutex<bool>>,
}

impl Drop for FakeGuard {
    fn drop(&mut self) {
        *lock(&self.held) = false;
    }
}

impl DrainGuard for FakeGuard {}

impl DrainLock for FakeDrainLock {
    fn try_acquire(&self) -> Result<Option<Box<dyn DrainGuard>>, CacheError> {
        let mut held = lock(&self.held);
        if *held {
            return Ok(None);
        }
        *held = true;
        Ok(Some(Box::new(FakeGuard {
            held: Arc::clone(&self.held),
        })))
    }
}
