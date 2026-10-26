//! Surface (b) operations: the `cache` commands (contract §6).

use std::collections::BTreeMap;
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainBudget, DrainLock, FingerprintOutcome, HOOK_BACKOFF_FROM,
    JournalState, Outage, ScopeFilter, ScopeId, ScopeStatus, StateCounts, SyncReport, SyncRow,
};
use sc_beads::{BeadId, CacheBackend, Deadline, Timestamp};

use crate::CONTRACT_VERSION;
use crate::data::{
    BackendData, CacheEvictData, CacheFlushData, CacheHealthData, CacheRefreshIssuesData,
    CacheRefreshScopesData, CacheScopesData, CacheStatusData, Entry, EscalatedData, HealthReason,
    HealthReasonCode, HealthState, JournalAnnounceData, JournalCountsData, JournalGetData,
    JournalListData, JournalResolveData, LastRunData, OpError, OpenWorkData, OutageData, ProbeData,
    ProjectData, RefreshError, RefreshScopeResult, RowsXCache, ScopeData, StateCountsData,
    SyncHealthData, SyncReportData, opt_time, outage, rows,
};
use crate::error::{ApiError, CODE_PREFIX};
use crate::ops::beads::{META_BD_VERSION, META_SCHEMA_VERSION, META_SHA};
use crate::ops::{CacheFacts, SyncFacts, SyncRows, known_scopes};
use crate::request::{
    CacheEvict, CacheFlush, CacheHealth, CacheRefreshIssues, CacheRefreshScopes, JournalAnnounce,
    JournalGet, JournalKey, JournalList, JournalListState, JournalResolve,
};

/// The stored alert of the outage (bd-cache-sync's `OUTAGE_CAUSE`).
const OUTAGE_ALERT: &str = "backend_unavailable";
/// The generic stored alert another writer of the shared v3 file records for
/// every sync failure (REQ-STORE-0006: the alert of a file another
/// implementation wrote reads back; the store's
/// `an_alert_another_implementation_wrote_reads_back_as_written` pins the row).
const SYNC_FAILED_ALERT: &str = "BD_CACHE.SYNC_FAILED";

/// How far past the per-call cap the `cache health --probe` deadline lies.
///
/// The backend check classifies a timeout at an expired hard deadline as a
/// cut, which records nothing (bd-cache's `outage::classify`). With the hard
/// deadline this much later than `call_timeout`, a bd that hangs is ended by
/// the per-call cap first, so it is classified and recorded as an outage, as
/// contract §6 states for `--probe`. One second covers killing and reaping
/// the bd child; a larger value only lengthens a probe that is cut anyway.
const PROBE_GRACE: Duration = Duration::from_secs(1);

/// A stored sync alert, in either writer's format of the shared v3 file
/// (REQ-STORE-0006, REQ-BDC-0804): this store's bare key, or the other
/// writer's `CODE: detail`. `backend_unavailable`, `sync_failed`, a
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
    let configured = cache.config().scopes.iter().any(|c| c.id == *id);
    Ok(ScopeData {
        scope: id.name().to_owned(),
        configured,
        on_demand: !configured,
        last_started_at: opt_time(s.sync.last_started_at.as_ref()),
        last_ok_at: opt_time(s.sync.last_ok_at.as_ref()),
        last_error: s.sync.last_error.as_deref().map(OpError::from_stored),
        alert: cache.sync_alert(id)?.as_deref().and_then(alert_of),
        beads: s.beads,
        filter: s.filter.as_ref().map(ScopeFilter::parts),
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
        pending: c.get(JournalState::Pending),
        applied: c.get(JournalState::Applied),
        escalated: c.get(JournalState::Escalated),
        resolved: c.get(JournalState::Resolved),
    }
}

/// `cache status`: the file and project from `facts`, the recorded
/// fingerprint, journal counts (`journal_counts`), scopes, escalations,
/// outage and stale rows, with [`CONTRACT_VERSION`].
///
/// # Errors
///
/// The store's errors.
pub(crate) fn cache_status<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    facts: &CacheFacts,
) -> Result<CacheStatusData, ApiError> {
    let st = cache.status()?.store;
    let jc = cache.journal_counts()?;
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
                .map(|(s, c)| (s.name().to_owned(), counts(c)))
                .collect(),
            unscoped: counts(jc.unscoped),
        },
        scopes: scopes_by_id(cache, &st.scopes)?,
        escalated,
        outage: outage(st.outage.as_ref()),
        stale,
    })
}

/// `cache health [--probe]`: `down` while an outage holds, `degraded` with a
/// reason for each cause (#86: stale rows, unresolved escalations, a pending
/// entry older than the limit; REQ-BDC-0732, from `sync`: no sync row
/// enabled, a stale scope, a failing sync row, a failing hook, or the rows'
/// state unreadable), `ok` otherwise; with `probe`, the backend check runs
/// too.
///
/// A probe that could not reach bd (an [`UNREACHABLE_CODES`] error, which
/// includes a deadline cut) is a result, not an error (contract §3, §6):
/// `state` is `down`, `probe` is `null`, and `outage` is the outage the probe
/// recorded, or, when it recorded none (a cut, or a bd that answered with a
/// failure), one observed now from the probe's error.
///
/// The store's status is read once, after the probe, and every judgement of
/// the journal and the scopes is made on that one snapshot (B9Q1-034); the
/// oldest pending age comes from the bounded `pending_summary`, never from
/// loading the backlog (ADR-SYNC-0008, B9Q1-014).
///
/// [`UNREACHABLE_CODES`]: bd_cache_model::UNREACHABLE_CODES
///
/// # Errors
///
/// The store's errors; with `probe`, any other typed error of the backend
/// check (a store failure recording the fingerprint).
pub(crate) fn cache_health<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: CacheHealth,
    sync: Result<SyncFacts, CacheError>,
) -> Result<CacheHealthData, ApiError> {
    let probe = if req.probe {
        let hard = Deadline::after(&*cache.clock(), cache.config().call_timeout + PROBE_GRACE);
        match cache.backend_check(hard) {
            Ok(checked) => Some(Ok(checked)),
            Err(e) if e.is_unreachable() => Some(Err(e)),
            Err(e) => return Err(e.into()),
        }
    } else {
        None
    };
    // Read after the probe: a probe that could not reach bd records the outage.
    let st = cache.status()?.store;
    let work = cache.open_work()?;
    let stale_rows = u64::try_from(st.stale.len()).unwrap_or(u64::MAX);
    let unresolved_escalations = u64::try_from(st.escalated.len()).unwrap_or(u64::MAX);
    let now = cache.clock().now_utc();
    let oldest_pending_age_s = cache
        .pending_summary()?
        .oldest_created_at
        .map(|oldest| u64::try_from(now.unix().saturating_sub(oldest.unix())).unwrap_or(0));
    let last_drain_at = cache.last_applied_at()?;
    let mut reasons = journal_reasons(
        stale_rows,
        unresolved_escalations,
        work.escalated_unannounced,
        oldest_pending_age_s,
        req.max_pending_age_s(),
    );
    let sync = match sync {
        Ok(facts) => {
            reasons.extend(sync_reasons(&st.scopes, &facts, &now));
            Some(SyncHealthData {
                mode: facts.rows.mode(),
                last_run: facts.rows.enabled().map(|r| LastRunData {
                    row: r.row,
                    last_run_at: opt_time(r.last_run_at.as_ref()),
                    outcome: r.outcome.clone(),
                    run_id: r.run_id.clone(),
                    consecutive_failures: r.consecutive_failures,
                }),
            })
        }
        Err(e) => {
            reasons.push(HealthReason {
                code: HealthReasonCode::SyncStateUnreadable,
                message: format!(
                    "the sync rows' state could not be read ({e}), so the sync is not judged; \
                     see `bdc cache schedule show`"
                ),
            });
            None
        }
    };
    let unreachable = match &probe {
        Some(Err(e)) => Some(e),
        _ => None,
    };
    let state = if st.outage.is_some() || unreachable.is_some() {
        HealthState::Down
    } else if reasons.is_empty() {
        HealthState::Ok
    } else {
        HealthState::Degraded
    };
    let outage = outage(st.outage.as_ref()).or_else(|| {
        unreachable.map(|e| {
            OutageData::of(&Outage {
                since: now.clone(),
                last_error: e.to_string(),
            })
        })
    });
    Ok(CacheHealthData {
        state,
        reasons,
        outage,
        open_work: OpenWorkData {
            pending: work.pending,
            escalated_unannounced: work.escalated_unannounced,
        },
        stale_rows,
        oldest_pending_age_s,
        last_drain_at: opt_time(last_drain_at.as_ref()),
        unresolved_escalations,
        sync,
        probe: match probe {
            Some(Ok((outcome, model_current))) => Some(ProbeData {
                bd_version: st.meta.get(META_BD_VERSION).cloned(),
                model_current,
                rebuilt: outcome == FingerprintOutcome::Rebuilt,
            }),
            Some(Err(_)) | None => None,
        },
    })
}

/// The reasons of #86 (contract §6): stale rows, unresolved escalations
/// (`escalated_unannounced` of them not yet announced), and an oldest pending
/// entry older than `max_age` seconds.
fn journal_reasons(
    stale_rows: u64,
    unresolved_escalations: u64,
    escalated_unannounced: u64,
    oldest_pending_age_s: Option<u64>,
    max_age: u64,
) -> Vec<HealthReason> {
    let mut reasons = Vec::new();
    if stale_rows > 0 {
        reasons.push(HealthReason {
            code: HealthReasonCode::StaleRows,
            message: format!(
                "{stale_rows} row(s) failed their read-back and are stale; \
                 `bdc cache refresh issues <id>...` re-reads them"
            ),
        });
    }
    if unresolved_escalations > 0 {
        reasons.push(HealthReason {
            code: HealthReasonCode::UnresolvedEscalations,
            message: format!(
                "{unresolved_escalations} escalated journal entries are unresolved \
                 ({escalated_unannounced} not yet announced); see `bdc cache journal list` \
                 and settle each with `bdc cache journal resolve <seq> --note T --actor A`"
            ),
        });
    }
    if let Some(age) = oldest_pending_age_s.filter(|a| *a > max_age) {
        reasons.push(HealthReason {
            code: HealthReasonCode::PendingTooOld,
            message: format!(
                "the oldest pending journal entry is {age} s old (limit {max_age} s): \
                 the drainer is not running or cannot apply it; run `bdc cache flush`"
            ),
        });
    }
    reasons
}

/// How many consecutive failed runs of the enabled sync row make health
/// `sync_failing` (REQ-BDC-0732: "the enabled row's last three runs failed").
/// Only health judges it; nothing in the sync acts on it.
pub(crate) const SYNC_FAILING_RUNS: u32 = 3;

/// How many periods of the enabled row a scope's last successful sync may be
/// old before health is `sync_stale` (REQ-BDC-0732: "older than three
/// periods"). Only health judges it.
pub(crate) const SYNC_STALE_PERIODS: u32 = 3;

/// The degraded-sync reasons of REQ-BDC-0732, judged at `now` (the cache's
/// clock) from what the front end read of the sync rows and from `scopes`,
/// the store status health read once:
///
/// - `no_sync_row_enabled`: no sync row is enabled, whether both rows are
///   disabled (the `both_disabled` flag of `schedule show`, REQ-BDC-0731) or
///   the table holds no sync row at all (lead ruling, sprint b-9).
/// - `sync_stale`: a registered scope (the scopes the built-in row syncs,
///   REQ-BDC-0721) whose last successful sync, or before its first one the
///   enabled row's `created_at`, is more than [`SYNC_STALE_PERIODS`] periods
///   before `now`. A row with no period (no next run, or none after it) is
///   not judged.
/// - `sync_failing`: the enabled row's last [`SYNC_FAILING_RUNS`] runs failed.
/// - `hook_failing`: the hook the built-in row names failed
///   [`HOOK_BACKOFF_FROM`] times in a row, the count from which the sync
///   backs it off (one owner, `bd-cache-model`).
fn sync_reasons(scopes: &[ScopeStatus], sync: &SyncFacts, now: &Timestamp) -> Vec<HealthReason> {
    let mut reasons = Vec::new();
    let Some(row) = sync.rows.enabled() else {
        reasons.push(HealthReason {
            code: HealthReasonCode::NoSyncRowEnabled,
            message: match sync.rows {
                SyncRows::NoRows => "no sync row exists, so nothing syncs the cache on a \
                                     schedule; `bdc cache schedule set` writes the built-in \
                                     row, `bdc cache sync use-python` the Python row"
                    .to_owned(),
                _ => "no sync row is enabled, so nothing syncs the cache on a schedule; \
                      `bdc cache schedule enable` or `bdc cache sync use-builtin` enables the \
                      built-in row, `bdc cache sync use-python` the Python row"
                    .to_owned(),
            },
        });
        reasons.extend(hook_reason(sync));
        return reasons;
    };
    if let Some(period) = row.period {
        let limit = period.saturating_mul(SYNC_STALE_PERIODS);
        let limit_s = i64::try_from(limit.as_secs()).unwrap_or(i64::MAX);
        let mut stale: Vec<&str> = scopes
            .iter()
            .filter(|s| s.registered)
            .filter(|s| {
                let since = s.sync.last_ok_at.as_ref().unwrap_or(&row.created_at);
                now.unix().saturating_sub(since.unix()) > limit_s
            })
            .map(|s| s.sync.scope.name())
            .collect();
        stale.sort_unstable();
        if !stale.is_empty() {
            reasons.push(HealthReason {
                code: HealthReasonCode::SyncStale,
                message: format!(
                    "the last successful sync of scope(s) {} is older than {SYNC_STALE_PERIODS} \
                     periods of the {} row ({} s): the worker or the scheduler that runs \
                     `bdc cache cron run` may not be running; see `bdc cache schedule show`",
                    stale.join(", "),
                    row_word(row.row),
                    limit.as_secs()
                ),
            });
        }
    }
    if row.consecutive_failures >= SYNC_FAILING_RUNS {
        reasons.push(HealthReason {
            code: HealthReasonCode::SyncFailing,
            message: format!(
                "the {} sync row's last {} runs failed (latest outcome: {}); \
                 see `bdc cache schedule show`",
                row_word(row.row),
                row.consecutive_failures,
                row.outcome.as_deref().unwrap_or("not recorded")
            ),
        });
    }
    reasons.extend(hook_reason(sync));
    reasons
}

/// `hook_failing` when the hook the built-in row names failed
/// [`HOOK_BACKOFF_FROM`] times in a row or more.
fn hook_reason(sync: &SyncFacts) -> Option<HealthReason> {
    let failures = sync.hook_failures.filter(|n| *n >= HOOK_BACKOFF_FROM)?;
    Some(HealthReason {
        code: HealthReasonCode::HookFailing,
        message: format!(
            "the post-sync hook failed {failures} times in a row and is backed off; \
             see its last error in `bdc cache schedule show`"
        ),
    })
}

/// The row's word in a reason's message: `builtin` or `python`.
fn row_word(row: SyncRow) -> &'static str {
    match row {
        SyncRow::Builtin => "builtin",
        SyncRow::Python => "python",
    }
}

/// `cache scopes`: every scope, by id (store reads only, no bd).
///
/// # Errors
///
/// The store's errors.
pub(crate) fn cache_scopes<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
) -> Result<CacheScopesData, ApiError> {
    let st = cache.status()?.store;
    Ok(CacheScopesData {
        scopes: scopes_by_id(cache, &st.scopes)?,
    })
}

/// `cache refresh scopes [S…]`: one sync per scope, each under the per-call
/// cap; a failed scope is reported in its result and counted in `failed`. A
/// sync that returned an outage is failed too, with `BD_UNAVAILABLE` (S75),
/// and so is one the deadline cut (`deferred`): it refreshed nothing.
///
/// # Errors
///
/// The store's errors reading the known scopes.
pub(crate) fn cache_refresh_scopes<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &CacheRefreshScopes,
) -> Result<CacheRefreshScopesData, ApiError> {
    let scopes: Vec<ScopeId> = if req.scopes.is_empty() {
        known_scopes(cache)?
    } else {
        req.scopes.clone()
    };
    let call_timeout = cache.config().call_timeout;
    let deadline_after = req.timeout_ms.map_or(call_timeout, Duration::from_millis);
    let mut results = Vec::with_capacity(scopes.len());
    for s in scopes {
        let d = Deadline::after(&*cache.clock(), deadline_after);
        let result = match cache.sync(&s, d) {
            Ok(r) => RefreshScopeResult {
                scope: s.name().to_owned(),
                error: sync_report_error(&s, &r, deadline_after),
                report: Some(SyncReportData {
                    rebuilt: r.rebuilt,
                    model_current: r.model_current,
                    stored: r.stored,
                    blocked: r.blocked,
                    deferred: r.deferred,
                    outage: outage(r.outage.as_ref()),
                }),
            },
            Err(e) => RefreshScopeResult {
                scope: s.name().to_owned(),
                report: None,
                error: Some(refresh_error(&e)),
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

/// The error of a sync that returned a report, or `None` when it refreshed.
///
/// A sync that could not reach bd serves stale rows (ADR-BDC-0005), but it
/// did not refresh: a failed result (lead ruling S75). A sync the deadline
/// cut (`deferred`) did not complete either: bd did not answer within the
/// per-call cap, which is also the sync's whole deadline, so the core
/// classified the timeout as a cut and recorded no outage (QA RCR-001).
fn sync_report_error(s: &ScopeId, r: &SyncReport, deadline: Duration) -> Option<RefreshError> {
    let unavailable = |message: String| {
        RefreshError::of(OpError {
            code: format!("{CODE_PREFIX}BD_UNAVAILABLE"),
            message,
        })
    };
    if let Some(o) = &r.outage {
        return Some(unavailable(OpError::from_stored(&o.last_error).message));
    }
    r.deferred.then(|| {
        unavailable(format!(
            "the sync of {} was cut: bd did not answer within the sync deadline ({} ms); \
             nothing was refreshed",
            s.name(),
            deadline.as_millis()
        ))
        .with_detail("timed_out", true)
    })
}

/// A failed sync's error with the details its envelope error would carry.
fn refresh_error(e: &CacheError) -> RefreshError {
    RefreshError {
        details: ApiError::from(e).details().clone(),
        ..RefreshError::of(OpError::of(e))
    }
}

/// `cache refresh issues ID…`: one fresh `bd list --id` (REQ-BDC-0907), rows
/// in request order.
///
/// # Errors
///
/// `BD_UNAVAILABLE` (`details.timed_out`) and the other mapped backend
/// errors; the store's errors.
pub(crate) fn cache_refresh_issues<S: CacheStore, L: DrainLock, B: CacheBackend>(
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
pub(crate) fn cache_flush<S: CacheStore, L: DrainLock, B: CacheBackend>(
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
                scope: e.phase.as_ref().map(|p| p.name().to_owned()),
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
pub(crate) fn cache_evict<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &CacheEvict,
) -> Result<CacheEvictData, ApiError> {
    let r = cache.prune(&req.scope)?;
    Ok(CacheEvictData {
        scope: r.scope.name().to_owned(),
        beads_deleted: r.beads_deleted,
        out_of_scope_swept: r.out_of_scope_swept,
    })
}

/// `cache journal get <seq> | --idempotency-key K`: one entry in any state.
///
/// # Errors
///
/// `NOT_FOUND`; the store's errors.
pub(crate) fn journal_get<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &JournalGet,
) -> Result<JournalGetData, ApiError> {
    let e = match &req.key {
        JournalKey::Seq(seq) => cache.entry(*seq)?,
        JournalKey::IdempotencyKey(k) => cache.entry_by_key(k)?,
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
pub(crate) fn journal_list<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: JournalList,
) -> Result<JournalListData, ApiError> {
    let mut entries = match (req.state, req.unannounced) {
        (JournalListState::Escalated, true) => cache.unannounced_escalations()?,
        (JournalListState::Escalated, false) => cache.status()?.store.escalated,
        (JournalListState::Pending, _) => cache.pending_entries()?,
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
pub(crate) fn journal_resolve<S: CacheStore, L: DrainLock, B: CacheBackend>(
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
pub(crate) fn journal_announce<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: JournalAnnounce,
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
