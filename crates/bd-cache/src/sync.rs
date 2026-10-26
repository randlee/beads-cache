//! `sync` of one scope, including its ready base (REQ-BDC-0501..0504, 0008,
//! 0909; ADR-BDC-0002, 0005).
//!
//! In order, all under the caller's deadline `d` (each call also capped by
//! `call_timeout`):
//!
//! 1. register the scope (`ensure_scope`): a sync of an unregistered name
//!    makes it an on-demand label scope with that one label (REQ-BDC-0909 as
//!    amended by ADR-BDC-0013); the 0.1 path registers the phase-a scope its
//!    argument names instead ([`sync_legacy`]);
//! 2. `backend_check::run` (REQ-BDC-0105); a rebuild drops the cursor;
//! 3. one `list_scope` of the scope's filter (`CallKind::Sync`; ADR-BDC-0013's
//!    argv, all seven statuses), with the cursor `last_ok_at - 2 min` once a
//!    sync has succeeded (REQ-BDC-0501);
//! 4. every row checked before any is stored (REQ-BDC-0008), then stored
//!    unforced, so a newer stored row is kept (REQ-BDC-0502); membership is
//!    the scope's filter with its status criterion, so a bead whose status
//!    left the set leaves the scope;
//! 5. stale rows re-read with one `list_by_ids` (`CallKind::Refresh`) and
//!    stored forced, which clears their stale mark (REQ-BDC-0503);
//! 6. the ready base: `list_blocked` and `list_ready` (`CallKind::Ready`),
//!    intersected with the scope's members, and `replace_ready_base`, its `at`
//!    taken before the first of the two;
//!    if either fails the previous base is kept and marked stale
//!    (ADR-BDC-0002) and a `tracing` warning names the error's code and
//!    detail (ADR-CORE-0203);
//! 7. `sync_state`: `last_ok_at` is the sync's start time (REQ-BDC-0504),
//!    and `failing_since` is cleared (REQ-BDC-0728).
//!
//! Failures of steps 2 to 5 (ADR-BDC-0005, ADR-CORE-0202). A failure or an
//! outage is a non-ok result: `failing_since` is set to the sync's start on
//! the first one after an ok, and kept across later ones (REQ-BDC-0728); the
//! outage is recorded per scope, its error in `last_error` (REQ-BDC-0729):
//!
//! | Failure | Reply | `sync_state` |
//! |---|---|---|
//! | reachable backend (`call` gives `Error`) | `Err(SYNC_FAILED)`, `phase` set, `cause` the underlying code | `last_started_at`; `last_error` = the error; `failing_since` |
//! | unreachable backend (`Outage`, recorded by `call`) | `Ok`, `outage` set | `last_started_at`; `last_error` = the outage's error; `failing_since` |
//! | deadline cut (`Cut`), or `d` expired before the start | `Ok`, `deferred` | `last_started_at` only (nothing when not started) |
//!
//! No sync touches a sync alert; announcing is the scheduler's (a-7).

use std::time::Duration;

use bd_cache_model::{
    CacheError, CacheStore, CallFailure, CallKind, DrainLock, FingerprintOutcome, Scope, ScopeId,
    SyncCause, SyncReport,
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

/// `sync(scope, d)` of a name taken as written: the registered scope, or an
/// on-demand label scope with that one label (see the module docs).
pub(crate) fn sync<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    d: Deadline,
) -> Result<SyncReport, CacheError> {
    let scope = match cache.store.scope(s)? {
        Some(scope) => scope,
        None => Scope::on_demand(s.clone()).map_err(|e| CacheError::BadArgs {
            detail: format!("scope {s} cannot be registered on demand: {e}"),
        })?,
    };
    sync_scope(cache, &scope, d)
}

/// `sync` of the phase-a scope a 0.1 argument names (REQ-BDC-0812 as
/// amended): registered as a phase-a scope when absent. A registered scope of
/// that name with a filter is another scope, never the one the 0.1 name
/// means: `SCOPE_UNKNOWN`, and nothing is registered or synced.
pub(crate) fn sync_legacy<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    d: Deadline,
) -> Result<SyncReport, CacheError> {
    match cache.store.scope(s)? {
        Some(scope) if !scope.is_legacy() => Err(not_legacy(cache, s)?),
        _ => sync_scope(cache, &Scope::legacy(s.clone()), d),
    }
}

/// `SCOPE_UNKNOWN` for a 0.1 name whose registered scope has a filter.
pub(crate) fn not_legacy<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    s: &ScopeId,
) -> Result<CacheError, CacheError> {
    Ok(CacheError::ScopeUnknown {
        detail: format!("scope {s} is a filter scope, not the phase-a scope a 0.1 name means"),
        known: cache.store.scopes()?.into_iter().map(|k| k.id).collect(),
    })
}

/// `sync` of `scope`, registered first when its name is absent; a
/// registered scope of the name keeps its own filter.
pub(crate) fn sync_scope<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    scope: &Scope,
    d: Deadline,
) -> Result<SyncReport, CacheError> {
    let scope = cache.store.ensure_scope(scope)?;
    let s = &scope.id;
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
    if let Err(stop) = sync_rows(cache, &scope, d, &mut report) {
        return finish_stopped(cache, s, &started, stop, report);
    }
    match fetch_base(cache, &scope, d) {
        Ok(base) => {
            report.blocked = store_base(cache, &scope, &base)?;
        }
        // The previous base stays, marked stale; an outage is already recorded.
        Err(failure) => {
            let e = failure.into_error("bd blocked / bd ready: cut by the sync deadline");
            sc_beads::logging::tracing::warn!(
                scope = %s,
                code = e.code(),
                detail = e.detail(),
                "ready base fetch failed; the previous base is kept, marked stale"
            );
            cache.store.mark_ready_base_stale(s)?;
        }
    }
    let mut state = cache.store.sync_state(s)?;
    state.last_started_at = Some(started.clone());
    state.last_ok_at = Some(started.clone());
    state.last_error = None;
    state.record_result(true, &started);
    cache.store.record_sync(&state)?;
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
        .sync_state(&s.id)?
        .last_ok_at
        .map(|t| SyncCursor::new(t.minus(CURSOR_MARGIN).as_str()));
    let filter = s.backend_filter();
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

/// `BD_CONTRACT` when a scope list returned a row its filter does not select
/// (labels, type or assignee); checked before any row of the list is stored
/// (REQ-BDC-0008).
fn check_scope_rows(s: &Scope, rows: &[Bead]) -> Result<(), CacheError> {
    match rows.iter().find(|r| !s.selects(r)) {
        Some(row) => Err(CacheError::BdContract {
            bead: Some(row.id.clone()),
            field: "labels".into(),
            detail: format!(
                "bd list for scope {} returned {}, which its filter does not select",
                s.id, row.id
            ),
            source: None,
        }),
        None => Ok(()),
    }
}

/// Records and reports a sync whose row steps stopped (the table in the module docs).
fn finish_stopped<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    started: &Timestamp,
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
        // The outage is this scope's result too (REQ-BDC-0729): its error
        // and the failure state are recorded per scope.
        CallFailure::Outage(e) => {
            state.last_error = Some(e.to_string());
            state.record_result(false, started);
            cache.store.record_sync(&state)?;
            report.outage = cache.store.outage()?;
            Ok(report)
        }
        CallFailure::Error(e) => {
            let e = sync_failed(s, e)?;
            state.last_error = Some(e.to_string());
            state.record_result(false, started);
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
