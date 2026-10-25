//! Surface (b) operations: the `cache` commands (contract §6).

use std::collections::BTreeMap;
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{
    CacheStore, DrainBudget, DrainLock, FingerprintOutcome, JournalState, ScopeId, ScopeStatus,
};
use sc_beads::{BeadId, CacheBackend, Deadline};

use crate::CONTRACT_VERSION;
use crate::b1_stub::{self, StateCounts};
use crate::data::{
    BackendData, CacheEvictData, CacheFlushData, CacheHealthData, CacheRefreshIssuesData,
    CacheRefreshScopesData, CacheScopesData, CacheStatusData, Entry, EscalatedData, HealthState,
    JournalAnnounceData, JournalCountsData, JournalGetData, JournalListData, JournalResolveData,
    OpError, OpenWorkData, ProbeData, ProjectData, RefreshScopeResult, RowsXCache, ScopeData,
    StateCountsData, SyncReportData, opt_time, outage, rows,
};
use crate::error::{ApiError, CODE_PREFIX};
use crate::ops::beads::{META_BD_VERSION, META_SCHEMA_VERSION, META_SHA};
use crate::ops::{CacheFacts, known_scopes};
use crate::request::{
    CacheEvict, CacheFlush, CacheHealth, CacheRefreshIssues, CacheRefreshScopes, JournalAnnounce,
    JournalGet, JournalKey, JournalList, JournalListState, JournalResolve,
};

/// The stored alert of the outage (bd-cache-sync's `OUTAGE_CAUSE`).
const OUTAGE_ALERT: &str = "backend_unavailable";
/// The generic stored alert `bd_cache.py` records for every sync failure.
const SYNC_FAILED_ALERT: &str = "BD_CACHE.SYNC_FAILED";

/// A stored sync alert, in either writer's format (the bare key, or
/// `bd_cache.py`'s `CODE: detail`): `backend_unavailable`, `sync_failed`, a
/// `BD_CACHE` code, or `None` for text with none of these.
fn alert_of(stored: &str) -> Option<String> {
    let key = stored.split_once(": ").map_or(stored, |(k, _)| k).trim();
    match key {
        OUTAGE_ALERT => Some(OUTAGE_ALERT.to_owned()),
        SYNC_FAILED_ALERT => Some("sync_failed".to_owned()),
        k if k.starts_with(CODE_PREFIX) => Some(k.to_owned()),
        _ => None,
    }
}

fn scope_data<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    s: &ScopeStatus,
) -> Result<ScopeData, ApiError> {
    let id = &s.sync.scope;
    let configured = cache.config().scopes.contains(id);
    Ok(ScopeData {
        scope: id.label(),
        configured,
        on_demand: !configured,
        last_started_at: opt_time(s.sync.last_started_at.as_ref()),
        last_ok_at: opt_time(s.sync.last_ok_at.as_ref()),
        last_error: s.sync.last_error.as_deref().map(OpError::from_stored),
        alert: cache.sync_alert(id)?.as_deref().and_then(alert_of),
        beads: s.beads,
    })
}

/// Every scope's Scope, by id.
fn scopes_by_id<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    statuses: &[ScopeStatus],
) -> Result<Vec<ScopeData>, ApiError> {
    let mut sorted: Vec<&ScopeStatus> = statuses.iter().collect();
    sorted.sort_by(|a, b| a.sync.scope.cmp(&b.sync.scope));
    sorted.into_iter().map(|s| scope_data(cache, s)).collect()
}

fn counts(c: StateCounts) -> StateCountsData {
    StateCountsData {
        pending: c.pending,
        applied: c.applied,
        escalated: c.escalated,
        resolved: c.resolved,
    }
}

/// `cache status`: the file and project from `facts`, the recorded
/// fingerprint, journal counts (`journal_counts`), scopes, escalations,
/// outage and stale rows, with [`CONTRACT_VERSION`].
///
/// # Errors
///
/// The store's errors.
pub fn cache_status<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    facts: &CacheFacts,
) -> Result<CacheStatusData, ApiError> {
    let st = cache.status()?.store;
    let jc = b1_stub::journal_counts(cache)?;
    let mut escalated: Vec<Entry> = st.escalated.iter().map(Entry::of).collect();
    escalated.sort_by_key(|e| e.seq);
    let mut stale = st.stale.clone();
    stale.sort();
    Ok(CacheStatusData {
        cache_path: facts.cache_path.clone(),
        contract_version: CONTRACT_VERSION,
        store_schema_version: facts.store_schema_version,
        project: ProjectData {
            database: facts.database.clone(),
            project_id: facts.project_id.clone(),
        },
        backend: BackendData {
            bd_version: st.meta.get(META_BD_VERSION).cloned(),
            bd_schema_sha256: st.meta.get(META_SHA).cloned(),
            bd_schema_version: st.meta.get(META_SCHEMA_VERSION).cloned(),
        },
        journal: JournalCountsData {
            total: counts(jc.total),
            by_scope: jc
                .by_scope
                .into_iter()
                .map(|(s, c)| (s.label(), counts(c)))
                .collect(),
            unscoped: counts(jc.unscoped),
        },
        scopes: scopes_by_id(cache, &st.scopes)?,
        escalated,
        outage: outage(st.outage.as_ref()),
        stale,
    })
}

/// `cache health [--probe]`: `down` while an outage holds, `degraded` with
/// stale rows or unannounced escalations, `ok` otherwise; with `probe`, the
/// backend check runs too.
///
/// # Errors
///
/// The store's errors; with `probe`, the backend check's typed error.
pub fn cache_health<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &CacheHealth,
) -> Result<CacheHealthData, ApiError> {
    let probe = if req.probe {
        let hard = Deadline::after(&*cache.clock(), cache.config().call_timeout);
        let (outcome, model_current) = b1_stub::backend_check(cache, hard)?;
        Some((outcome, model_current))
    } else {
        None
    };
    let st = cache.status()?.store;
    let work = cache.open_work()?;
    let stale_rows = u64::try_from(st.stale.len()).unwrap_or(u64::MAX);
    let state = if st.outage.is_some() {
        HealthState::Down
    } else if stale_rows > 0 || work.escalated_unannounced > 0 {
        HealthState::Degraded
    } else {
        HealthState::Ok
    };
    Ok(CacheHealthData {
        state,
        outage: outage(st.outage.as_ref()),
        open_work: OpenWorkData {
            pending: work.pending,
            escalated_unannounced: work.escalated_unannounced,
        },
        stale_rows,
        probe: probe.map(|(outcome, model_current)| ProbeData {
            bd_version: st.meta.get(META_BD_VERSION).cloned(),
            model_current,
            rebuilt: outcome == FingerprintOutcome::Rebuilt,
        }),
    })
}

/// `cache scopes`: every scope, by id (store reads only, no bd).
///
/// # Errors
///
/// The store's errors.
pub fn cache_scopes<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
) -> Result<CacheScopesData, ApiError> {
    let st = cache.status()?.store;
    Ok(CacheScopesData {
        scopes: scopes_by_id(cache, &st.scopes)?,
    })
}

/// `cache refresh scopes [S…]`: one sync per scope, each under the per-call
/// cap; a failed scope is reported in its result and counted in `failed`.
///
/// # Errors
///
/// The store's errors reading the known scopes.
pub fn cache_refresh_scopes<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &CacheRefreshScopes,
) -> Result<CacheRefreshScopesData, ApiError> {
    let scopes: Vec<ScopeId> = if req.scopes.is_empty() {
        known_scopes(cache)?
    } else {
        req.scopes.clone()
    };
    let mut results = Vec::with_capacity(scopes.len());
    for s in scopes {
        let d = Deadline::after(&*cache.clock(), cache.config().call_timeout);
        let result = match cache.sync(&s, d) {
            Ok(r) => RefreshScopeResult {
                scope: s.label(),
                report: Some(SyncReportData {
                    rebuilt: r.rebuilt,
                    model_current: r.model_current,
                    stored: r.stored,
                    blocked: r.blocked,
                    deferred: r.deferred,
                    outage: outage(r.outage.as_ref()),
                }),
                error: None,
            },
            Err(e) => RefreshScopeResult {
                scope: s.label(),
                report: None,
                error: Some(OpError::of(&e)),
            },
        };
        results.push(result);
    }
    let failed = results.iter().filter(|r| r.error.is_some()).count();
    Ok(CacheRefreshScopesData {
        failed: u64::try_from(failed).unwrap_or(u64::MAX),
        results,
    })
}

/// `cache refresh issues ID…`: one fresh `bd list --id` (REQ-BDC-0907), rows
/// in request order.
///
/// # Errors
///
/// `BD_UNAVAILABLE` (`details.timed_out`) and the other mapped backend
/// errors; the store's errors.
pub fn cache_refresh_issues<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &CacheRefreshIssues,
) -> Result<CacheRefreshIssuesData, ApiError> {
    let timeout = req
        .timeout_ms
        .map_or(cache.config().miss_fill_timeout, Duration::from_millis);
    let fetched = cache.refresh(&req.ids, timeout)?;
    let mut by_id: BTreeMap<BeadId, _> = fetched
        .into_iter()
        .map(|r| (r.bead.id.clone(), r))
        .collect();
    let ordered: Vec<_> = req.ids.iter().filter_map(|id| by_id.remove(id)).collect();
    Ok(CacheRefreshIssuesData {
        x_cache: RowsXCache {
            rows: rows(&ordered),
        },
        issues: ordered.into_iter().map(|r| r.bead).collect(),
    })
}

/// `cache flush`: one drain under the start gate and deadline.
///
/// # Errors
///
/// The store's errors; bd's failures are reported in the data.
pub fn cache_flush<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &CacheFlush,
) -> Result<CacheFlushData, ApiError> {
    let clock = cache.clock();
    let budget = DrainBudget {
        start_gate: Deadline::after(&*clock, Duration::from_millis(req.start_gate_ms)),
        hard: Deadline::after(&*clock, Duration::from_millis(req.deadline_ms)),
    };
    let r = cache.drain(&budget)?;
    let seqs = |v: &[bd_cache_model::Seq]| v.iter().map(|s| s.get()).collect::<Vec<_>>();
    Ok(CacheFlushData {
        busy: r.busy,
        applied: seqs(&r.applied),
        retrying: seqs(&r.retrying),
        escalated: r
            .escalated
            .iter()
            .map(|e| EscalatedData {
                seq: e.seq.get(),
                bead: e.bead.clone(),
                op: e.op.clone(),
                actor: e.actor.as_str().to_owned(),
                scope: e.phase.as_ref().map(|p| format!("phase-{p}")),
                reason: OpError::from_stored(&e.reason),
            })
            .collect(),
        deferred: seqs(&r.deferred),
        deferred_in_flight: seqs(&r.deferred_in_flight),
        outage: outage(r.outage.as_ref()),
    })
}

/// `cache evict --scope S`: prunes the scope.
///
/// # Errors
///
/// `PRUNE_BLOCKED`; the store's errors.
pub fn cache_evict<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &CacheEvict,
) -> Result<CacheEvictData, ApiError> {
    let r = cache.prune(&req.scope)?;
    Ok(CacheEvictData {
        scope: r.scope.label(),
        beads_deleted: r.beads_deleted,
        out_of_scope_swept: r.out_of_scope_swept,
    })
}

/// `cache journal get <seq> | --idempotency-key K`: one entry in any state.
///
/// # Errors
///
/// `NOT_FOUND`; the store's errors.
pub fn journal_get<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &JournalGet,
) -> Result<JournalGetData, ApiError> {
    let e = match &req.key {
        JournalKey::Seq(seq) => cache.entry(*seq)?,
        JournalKey::IdempotencyKey(k) => b1_stub::entry_by_key(cache, k)?,
    };
    Ok(JournalGetData {
        entry: Entry::of(&e),
    })
}

/// `cache journal list`: escalated entries (the default; with `unannounced`,
/// only those not yet announced) or pending ones, by seq.
///
/// # Errors
///
/// The store's errors.
pub fn journal_list<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &JournalList,
) -> Result<JournalListData, ApiError> {
    let mut entries = match (req.state, req.unannounced) {
        (JournalListState::Escalated, true) => cache.unannounced_escalations()?,
        (JournalListState::Escalated, false) => cache.status()?.store.escalated,
        (JournalListState::Pending, _) => b1_stub::pending_entries(cache)?,
    };
    entries.sort_by_key(|e| e.seq);
    Ok(JournalListData {
        entries: entries.iter().map(Entry::of).collect(),
    })
}

/// `cache journal resolve`: resolves an escalated entry; one already
/// resolved is left unchanged with `already_resolved: true`.
///
/// # Errors
///
/// `NOT_ESCALATED` for a pending or applied entry; `NOT_FOUND`; `BAD_ARGS`
/// for a note over the size limit; the store's errors.
pub fn journal_resolve<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &JournalResolve,
) -> Result<JournalResolveData, ApiError> {
    let before = cache.entry(req.seq)?;
    if before.state == JournalState::Resolved {
        return Ok(JournalResolveData {
            entry: Entry::of(&before),
            already_resolved: true,
        });
    }
    cache.resolve(req.seq, &req.note, req.actor.clone())?;
    Ok(JournalResolveData {
        entry: Entry::of(&cache.entry(req.seq)?),
        already_resolved: false,
    })
}

/// `cache journal announce`: records that an escalation was announced; one
/// already announced is left unchanged with `already_announced: true`.
///
/// # Errors
///
/// `NOT_FOUND`; the store's errors.
pub fn journal_announce<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &JournalAnnounce,
) -> Result<JournalAnnounceData, ApiError> {
    let before = cache.entry(req.seq)?;
    if before.announced_at.is_some() {
        return Ok(JournalAnnounceData {
            entry: Entry::of(&before),
            already_announced: true,
        });
    }
    cache.mark_announced(req.seq)?;
    Ok(JournalAnnounceData {
        entry: Entry::of(&cache.entry(req.seq)?),
        already_announced: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerts_decode_from_either_writer() {
        assert_eq!(
            alert_of("backend_unavailable").as_deref(),
            Some(OUTAGE_ALERT)
        );
        assert_eq!(
            alert_of("BD_CACHE.SYNC_FAILED: bd list failed").as_deref(),
            Some("sync_failed")
        );
        assert_eq!(
            alert_of("BD_CACHE.BD_CONTRACT").as_deref(),
            Some("BD_CACHE.BD_CONTRACT")
        );
        assert_eq!(alert_of("something else"), None);
    }
}
