//! `sync` of one scope, including its ready base (REQ-BDC-0501..0504, 0008,
//! 0909; ADR-BDC-0002, 0005).
//!
//! In order, all under the caller's deadline `d` (each call also capped by
//! `call_timeout`):
//!
//! 1. register the scope (`ensure_scope`): a sync of an unconfigured scope
//!    id makes it an on-demand scope (REQ-BDC-0909);
//! 2. `backend_check::run` (REQ-BDC-0105); a rebuild drops the cursor;
//! 3. one `list_scope` (`CallKind::Sync`) of the scope's bd filter, in every
//!    status (REQ-BDC-0702), with the cursor `last_ok_at - 2 min` once a sync
//!    has succeeded (REQ-BDC-0501);
//! 4. every row checked before any is stored (REQ-BDC-0008): bd must return
//!    only rows the filter selects; then stored unforced, so a newer stored
//!    row is kept (REQ-BDC-0502), with membership from the whole filter, its
//!    status criterion included;
//! 5. stale rows re-read with one `list_by_ids` (`CallKind::Refresh`) and
//!    stored forced, which clears their stale mark (REQ-BDC-0503);
//! 6. the ready base: `list_blocked` and `list_ready` (`CallKind::Ready`)
//!    and `replace_ready_base`, its `at` taken before the first of the two;
//!    if either fails the previous base is kept and marked stale
//!    (ADR-BDC-0002) and a `tracing` warning names the error's code and
//!    detail (ADR-CORE-0203);
//! 7. `sync_state`: `last_ok_at` is the sync's start time (REQ-BDC-0504),
//!    and the scope's failure state is cleared (`failing_since`,
//!    `notified_since`; REQ-BDC-0704).
//!
//! Failures of steps 2 to 5 (ADR-BDC-0005, ADR-CORE-0202):
//!
//! | Failure | Reply | `sync_state` |
//! |---|---|---|
//! | reachable backend (`call` gives `Error`) | `Err(SYNC_FAILED)`, `phase` set, `cause` the underlying code | `last_error` = the error; `failing_since` kept, or set to the start |
//! | unreachable backend (`Outage`, recorded by `call`) | `Ok`, `outage` set | `last_started_at`; `failing_since` kept, or set to the start |
//! | deadline cut (`Cut`), or `d` expired before the start | `Ok`, `deferred` | `last_started_at` only (nothing when not started) |
//!
//! `failing_since` is when the current run of failures and outages began; a
//! sync that passes clears it, and with it the hook's `notified_since`
//! (REQ-BDC-0704..0706). No sync touches a sync alert.

use std::time::Duration;

use bd_cache_model::{
    CacheError, CacheStore, CallFailure, CallKind, DrainLock, FingerprintOutcome, Scope, ScopeId,
    SyncCause, SyncReport, SyncState,
};
use sc_beads::{Bead, CacheBackend, Deadline, SyncCursor, Timestamp};

use crate::Cache;
use crate::ready::{fetch_base, store_base};
use crate::refresh::check_requested;

/// How far before `last_ok_at` an incremental sync's cursor starts (REQ-BDC-0501).
pub(crate) const CURSOR_MARGIN: Duration = Duration::from_secs(120);

/// Why the row steps of a sync stopped.
enum Stop {
    /// A backend call failed, classified by `call`.
    Backend(CallFailure),
    /// The store failed; returned as is.
    Store(CacheError),
}

impl Stop {
    /// A reachable backend gave an answer the cache refuses (`BD_CONTRACT`).
    fn bad_answer(e: CacheError) -> Self {
        Self::Backend(CallFailure::Error(e))
    }
}

impl From<CacheError> for Stop {
    fn from(e: CacheError) -> Self {
        Self::Store(e)
    }
}

impl From<CallFailure> for Stop {
    fn from(f: CallFailure) -> Self {
        Self::Backend(f)
    }
}

/// `sync(scope, d)`: see the module docs.
pub(crate) fn sync<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    scope: &Scope,
    d: Deadline,
) -> Result<SyncReport, CacheError> {
    let s = scope.id();
    cache.store.ensure_scope(s)?;
    let mut report = SyncReport {
        scope: s.clone(),
        rebuilt: false,
        model_current: false,
        stored: 0,
        blocked: 0,
        deferred: false,
        outage: None,
    };
    if d.expired(&*cache.clock) {
        report.deferred = true;
        return Ok(report);
    }
    let started = cache.clock.now_utc();
    if let Err(stop) = sync_rows(cache, scope, d, &mut report) {
        return finish_stopped(cache, s, started, stop, report);
    }
    match fetch_base(cache, scope, d) {
        Ok(base) => {
            store_base(cache, s, &base)?;
            report.blocked = base.blocked_len();
        }
        // The previous base stays, marked stale; an outage is already recorded.
        Err(failure) => {
            let e = failure.into_error("bd blocked / bd ready: cut by the sync deadline");
            sc_beads::logging::tracing::warn!(
                phase = %s,
                code = e.code(),
                detail = e.detail(),
                "ready base fetch failed; the previous base is kept, marked stale"
            );
            cache.store.mark_ready_base_stale(s)?;
        }
    }
    cache.store.record_sync(&SyncState {
        scope: s.clone(),
        last_started_at: Some(started.clone()),
        last_ok_at: Some(started),
        last_error: None,
        failing_since: None,
        notified_since: None,
    })?;
    report.outage = cache.store.outage()?;
    Ok(report)
}

/// Steps 2 to 5: backend check, scope list, stale re-read.
fn sync_rows<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &Scope,
    d: Deadline,
    report: &mut SyncReport,
) -> Result<(), Stop> {
    let (outcome, model_current) = crate::backend_check::run(cache, d)?;
    report.rebuilt = outcome == FingerprintOutcome::Rebuilt;
    report.model_current = model_current;
    // Read after the check: a rebuild deletes the sync state, so the next
    // list is a full one.
    let cursor = cache
        .store
        .sync_state(s.id())?
        .last_ok_at
        .map(|t| SyncCursor::new(t.minus(CURSOR_MARGIN).as_str()));
    let filter = s.filter().clone();
    let rows = crate::call(cache, CallKind::Sync, d, |b, p, dl| {
        b.list_scope(p, &filter, cursor.as_ref(), dl)
    })?;
    check_scope_rows(s, &rows).map_err(Stop::bad_answer)?;
    let mut stored = cache.store.upsert_rows(Some(s), &rows, false)?.stored.len();
    let stale = cache.store.stale_rows()?;
    if !stale.is_empty() {
        let fresh = crate::call(cache, CallKind::Refresh, d, |b, p, dl| {
            b.list_by_ids(p, &stale, dl)
        })?;
        check_requested(&stale, &fresh).map_err(Stop::bad_answer)?;
        stored += cache.store.upsert_rows(None, &fresh, true)?.stored.len();
    }
    report.stored = u32::try_from(stored).unwrap_or(u32::MAX);
    Ok(())
}

/// `BD_CONTRACT` when a scope list returned a row the scope's filter does
/// not select, naming the field that fails it; checked before any row of the
/// list is stored (REQ-BDC-0008). An ingest checks its rows the same way.
pub(crate) fn check_scope_rows(s: &Scope, rows: &[Bead]) -> Result<(), CacheError> {
    let Some(row) = rows.iter().find(|r| !s.selects(r)) else {
        return Ok(());
    };
    let f = s.filter();
    let field = if !f.labels().iter().all(|l| row.has_label(l))
        || (!f.labels_any().is_empty() && !f.labels_any().iter().any(|l| row.has_label(l)))
    {
        "labels"
    } else if f
        .issue_type()
        .is_some_and(|t| row.issue_type.as_deref() != Some(t))
    {
        "issue_type"
    } else {
        "assignee"
    };
    Err(CacheError::BdContract {
        bead: Some(row.id.clone()),
        field: field.into(),
        detail: format!(
            "{} is outside the filter of scope {}: its {field} do not match",
            row.id,
            s.id()
        ),
        source: None,
    })
}

/// Records and reports a sync whose row steps stopped (the table in the module docs).
fn finish_stopped<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    started: Timestamp,
    stop: Stop,
    mut report: SyncReport,
) -> Result<SyncReport, CacheError> {
    let failure = match stop {
        Stop::Store(e) => return Err(e),
        Stop::Backend(f) => f,
    };
    let mut state = cache.store.sync_state(s)?;
    state.last_started_at = Some(started.clone());
    match failure {
        CallFailure::Cut => {
            cache.store.record_sync(&state)?;
            report.deferred = true;
            Ok(report)
        }
        CallFailure::Outage(_) => {
            // An outage is a failure of the scope too (REQ-BDC-0704): the run
            // of failures keeps the time it began.
            state.failing_since.get_or_insert(started);
            cache.store.record_sync(&state)?;
            report.outage = cache.store.outage()?;
            Ok(report)
        }
        CallFailure::Error(e) => {
            let e = sync_failed(s, e)?;
            state.last_error = Some(e.to_string());
            state.failing_since.get_or_insert(started);
            cache.store.record_sync(&state)?;
            Err(e)
        }
    }
}

/// The `SYNC_FAILED` for a failure of a reachable backend, keeping the
/// backend error as its source; any other error (a store failure surfaced
/// by a helper) comes back as `Err`, unchanged.
fn sync_failed(s: &ScopeId, e: CacheError) -> Result<CacheError, CacheError> {
    match e {
        CacheError::SyncFailed {
            detail,
            cause,
            source,
            ..
        } => Ok(CacheError::SyncFailed {
            detail: format!("scope {s}: {detail}"),
            scope: Some(s.clone()),
            cause,
            source,
        }),
        other => match SyncCause::of(&other) {
            Some(cause) => Ok(CacheError::SyncFailed {
                detail: match &other {
                    // Name the bead and field, which the code's own text leaves out.
                    CacheError::BdContract {
                        bead,
                        field,
                        detail,
                        ..
                    } => format!(
                        "scope {s}: {}: {}: {field}: {detail}",
                        other.code(),
                        bead.as_ref().map_or("?", |b| b.as_str())
                    ),
                    _ => format!("scope {s}: {other}"),
                },
                scope: Some(s.clone()),
                cause: Some(cause),
                source: other.backend_cause().cloned(),
            }),
            None => Err(other),
        },
    }
}
