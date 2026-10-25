//! One run of the built-in sync (REQ-BDC-0701, 0702, 0703; ANC-002).
//!
//! [`run_sync`] is what the built-in row of the cron table runs, through
//! [`SyncJob`](crate::SyncJob) or on demand (`bdc cache tick`). Only one
//! runner may drive a cache file. Time is read from the cache's own clock
//! ([`Cache::clock`]), so the deadlines and the budget margins are on one
//! timeline with the backend's calls.

use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{CacheError, CacheStore, DrainBudget, DrainLock, HARD_MARGIN};
use sc_beads::{CacheBackend, Deadline};

use crate::control::StopFlag;
use crate::report::{ScopeError, ScopeResult, SyncRun};

/// The default budget of a run: the built-in row's default timeout, and so
/// the distance from a run's start to its end `T` (REQ-BDC-0703).
pub const DEFAULT_BUDGET: Duration = Duration::from_secs(25);

/// Runs the built-in sync once; it must be finished by `end` (`T`).
///
/// In order (REQ-BDC-0703):
///
/// 1. the journal is drained with `DrainBudget { start_gate: T - 5 s, hard:
///    T - 2 s }`;
/// 2. each configured scope is synced, in configuration order, with its bd
///    filter and deadline `T` (REQ-BDC-0702); a scope not started by
///    `T - 2 s` is deferred to the next run.
///
/// Every bd call is capped by the per-call cap and by the time left before
/// its deadline (NFR-BDC-0004). A sync that fails is reported in the scope's
/// result with its error, and the scope's failure state is recorded
/// (REQ-BDC-0704); an outage is a result, never an error (ADR-BDC-0005). The
/// stop flag is checked after the drain and between scopes: a run that finds
/// it set ends with [`SyncRun::stopped`].
///
/// # Errors
///
/// A store error, or an error of the cache that is not a sync failure.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use bd_cache::Cache;
/// use bd_cache_model::testing::{FakeDrainLock, FakeStore};
/// use bd_cache_model::{CacheConfig, Scope, ScopeId};
/// use bd_cache_sync::{DEFAULT_BUDGET, StopFlag, run_sync};
/// use sc_beads::testing::{FakeBackend, ManualClock};
/// use sc_beads::{Clock, Deadline, ProjectRef, ScopeFilter};
///
/// let clock = Arc::new(ManualClock::default());
/// let all = Scope::new(ScopeId::new("all").unwrap(), ScopeFilter::All).unwrap();
/// let mut cache = Cache::new(
///     CacheConfig::new(ProjectRef::local(None), vec![all]),
///     FakeStore::new(),
///     FakeDrainLock::new(),
///     FakeBackend::new(Arc::clone(&clock)),
///     Arc::clone(&clock) as Arc<dyn Clock>,
/// )
/// .unwrap();
/// let end = Deadline::after(&*clock, DEFAULT_BUDGET);
/// let run = run_sync(&mut cache, end, &StopFlag::new()).unwrap();
/// assert_eq!(run.scopes.len(), 1);
/// assert!(run.scopes[0].error.is_none());
/// ```
pub fn run_sync<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    end: Deadline,
    stop: &StopFlag,
) -> Result<SyncRun, CacheError> {
    let clock = cache.clock();
    let mut run = SyncRun::started();
    if stop.is_stopped() {
        run.stopped = true;
        return Ok(run);
    }
    run.drain = Some(cache.drain(&DrainBudget::before_tick_end(end))?);
    let gate = Deadline::at(
        end.instant()
            .checked_sub(HARD_MARGIN)
            .unwrap_or_else(|| end.instant()),
    );
    let scopes = cache.config().scopes.clone();
    for scope in &scopes {
        if stop.is_stopped() {
            run.stopped = true;
            break;
        }
        let id = scope.id();
        let (result, error, deferred) = if gate.expired(&*clock) {
            (None, None, true)
        } else {
            match cache.sync(id, end) {
                Ok(report) => {
                    let deferred = report.deferred;
                    (Some(report), None, deferred)
                }
                Err(e @ CacheError::SyncFailed { .. }) => (None, Some(ScopeError::of(&e)), false),
                Err(other) => return Err(other),
            }
        };
        let state = cache.sync_state(id)?;
        run.scopes
            .push(ScopeResult::new(id.clone(), result, error, deferred, &state));
    }
    run.outage = cache.outage()?;
    Ok(run)
}
