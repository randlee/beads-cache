//! One sync at a time: [`TickLease`], the RAII guard over the cache file's
//! `tick_lease` row, and the run id (REQ-BDC-0722, ADR-SYNC-0008).
//!
//! Every run takes the lease first: the built-in row's, the Python row's, an
//! on-demand `tick` and the immediate run of `sync use-python`. It is taken
//! under `BEGIN IMMEDIATE` (the store's `lease_take`), refused while a live
//! lease is held, taken over once expired, and expires at the run's deadline
//! plus [`LEASE_MARGIN`]. It is released on every exit path: explicitly by
//! [`TickLease::release`], or by the guard's drop. [`TickLease::take`]
//! installs that drop-release inside the blocking step that takes the lease,
//! before the lease value exists, so no path ever holds a lease without it,
//! not even a run aborted while it waits for the take. A release that fails
//! or cannot run is logged and left to expiry; a `kill -9` is covered only by
//! expiry.
//!
//! # The cache mutex
//!
//! Runs share the cache behind a `std::sync::Mutex` ([`SharedCache`]).
//! Policy, stated once: a run that finds the mutex poisoned (a panic in an
//! earlier step) fails with `STORE_FAILED` and trusts nothing it would read
//! ([`hold`]); a lease release, the drop-release included, takes a poisoned
//! mutex all the same, since it only frees the lease row and leaving the
//! lease to expiry would block every later run; and [`join_idle`]
//! (`crate::job`) treats a poisoned mutex as idle. A run waits for the mutex at
//! most until its deadline ([`hold_within`]): one held by a step sc-cron
//! timed out (its blocking thread is never aborted) makes the run skip as
//! `in_flight`, so no blocking thread queues behind a hung step.
//!
//! The drop-release never waits for the mutex on the dropping thread: it
//! releases inline when the mutex is free (the usual case: a run aborted
//! while its hook or script ran, or a blocking step's result dropped after
//! the step let go of the mutex), so it does not depend on a blocking pool
//! that may be shutting down; when the mutex is held (perhaps by the
//! dropping thread itself), it hands the release to the blocking pool if a
//! tokio runtime is current, and otherwise logs it and leaves the lease to
//! expiry. So it cannot deadlock. Every run of this crate releases
//! explicitly while it holds the mutex.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, TryLockError};
use std::time::{Duration, Instant};

use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainLock, LeaseHolder, LeaseTake, ROW_TIMEOUT_MARGIN, RunId,
};
use sc_beads::{CacheBackend, Clock, Deadline, Timestamp};

use crate::run::SharedCache;

/// How long after a run's deadline its lease expires (REQ-BDC-0722).
///
/// The model's row-timeout margin: the built-in row's sc-cron timeout is its
/// budget plus the same 10 s, so sc-cron never aborts a run before its lease
/// release, and a lease taken for the longest run lasts exactly the most the
/// store accepts (`MAX_LEASE_TTL`).
pub const LEASE_MARGIN: Duration = ROW_TIMEOUT_MARGIN;

/// How often [`hold_within`] tries the cache mutex again.
const HOLD_RETRY: Duration = Duration::from_millis(5);

/// A run id, `<unix-ms>-<pid>-<counter>` (ADR-SYNC-0008): the run's start in
/// Unix milliseconds on the clock that starts it, this process's id, and a
/// counter that makes the ids of one process unique.
///
/// A scheduled run's id is minted by the resolver on sc-cron's clock, which
/// reads milliseconds; an on-demand run's on the cache's clock, which reads
/// whole seconds (its milliseconds are then `000`).
///
/// # Errors
///
/// Never in practice: the text is never blank, which is all `RunId` refuses;
/// the refusal is passed on rather than replaced.
pub fn new_run_id(unix_ms: i64) -> Result<RunId, CacheError> {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    RunId::new(format!("{unix_ms}-{}-{n}", std::process::id())).map_err(|e| CacheError::BadArgs {
        detail: format!("run id: {e}"),
    })
}

/// A run id on the cache's clock (an on-demand run).
pub(crate) fn run_id_now(clock: &dyn Clock) -> Result<RunId, CacheError> {
    new_run_id(clock.now_utc().unix().saturating_mul(1000))
}

/// A run id on sc-cron's clock (a scheduled run, minted by the resolver).
pub(crate) fn run_id_on<C: sc_cron::Clock + ?Sized>(clock: &C) -> Result<RunId, CacheError> {
    new_run_id(clock.now().timestamp_millis())
}

/// This host's name, for the lease row (`HOSTNAME`, `COMPUTERNAME`, or `localhost`).
fn host() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| "localhost".to_owned())
}

/// The cache's mutex, held.
pub(crate) type Held<'a, S, L, B> = MutexGuard<'a, Cache<S, L, B>>;

/// Takes the cache's mutex; a poisoned one is an error, never trusted.
///
/// # Errors
///
/// `STORE_FAILED` for a poisoned mutex.
pub(crate) fn hold<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Mutex<Cache<S, L, B>>,
) -> Result<MutexGuard<'_, Cache<S, L, B>>, CacheError> {
    cache.lock().map_err(|e| poisoned(&e))
}

/// Takes the cache's mutex within `wait`; `None` when another step still
/// holds it then (module docs).
///
/// # Errors
///
/// `STORE_FAILED` for a poisoned mutex.
pub(crate) fn hold_within<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Mutex<Cache<S, L, B>>,
    wait: Duration,
) -> Result<Option<Held<'_, S, L, B>>, CacheError> {
    let until = Instant::now()
        .checked_add(wait)
        .unwrap_or_else(Instant::now);
    loop {
        match cache.try_lock() {
            Ok(guard) => return Ok(Some(guard)),
            Err(TryLockError::Poisoned(e)) => return Err(poisoned(&e)),
            Err(TryLockError::WouldBlock) => {}
        }
        let now = Instant::now();
        if now >= until {
            return Ok(None);
        }
        std::thread::sleep((until - now).min(HOLD_RETRY));
    }
}

fn poisoned(e: &dyn std::fmt::Display) -> CacheError {
    CacheError::StoreFailed {
        detail: format!(
            "the cache mutex was poisoned by a panic in an earlier run ({e}); restart the process"
        ),
        source: None,
    }
}

/// What taking the lease gave.
#[derive(Debug)]
#[must_use]
pub enum LeaseTaken {
    /// This run holds the lease (free, or taken over from an expired holder).
    Taken(TickLease),
    /// A live lease is held by another run: do nothing (REQ-BDC-0722).
    Held(LeaseHolder),
}

/// A release to run when a [`TickLease`] drops unreleased.
type Fallback = Box<dyn FnOnce(RunId) + Send + Sync>;

/// The held sync lease; release it with [`TickLease::release`].
///
/// Built only by [`TickLease::take`], with its drop-release installed. The
/// run functions take `&TickLease`, so no sync work runs without one.
#[must_use = "a lease is released by TickLease::release; dropping it hands the release to its drop"]
pub struct TickLease {
    holder: LeaseHolder,
    took_over: Option<LeaseHolder>,
    released: bool,
    fallback: Option<Fallback>,
}

impl std::fmt::Debug for TickLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TickLease")
            .field("holder", &self.holder)
            .field("took_over", &self.took_over)
            .field("released", &self.released)
            .finish_non_exhaustive()
    }
}

impl TickLease {
    /// Takes the lease for `run_id` on `cache` (the guard of `shared`),
    /// expiring [`LEASE_MARGIN`] after `deadline`, with the drop-release
    /// over `shared` installed before the lease is returned.
    ///
    /// # Errors
    ///
    /// The store's errors.
    pub fn take<S, L, B>(
        shared: &SharedCache<S, L, B>,
        cache: &mut Cache<S, L, B>,
        run_id: RunId,
        deadline: Deadline,
    ) -> Result<LeaseTaken, CacheError>
    where
        S: CacheStore + 'static,
        L: DrainLock + 'static,
        B: CacheBackend + 'static,
    {
        let clock = cache.clock();
        let now = clock.now_utc();
        let ttl = deadline.remaining(&*clock).saturating_add(LEASE_MARGIN);
        let secs = i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX).max(1);
        let expires_at = Timestamp::from_unix(now.unix().saturating_add(secs)).map_err(|e| {
            CacheError::BadArgs {
                detail: format!("lease expiry: {e}"),
            }
        })?;
        let holder = LeaseHolder {
            run_id,
            pid: std::process::id(),
            host: host(),
            expires_at,
        };
        let took_over = match cache.lease_take(&holder)? {
            LeaseTake::Held(other) => return Ok(LeaseTaken::Held(other)),
            LeaseTake::Taken => None,
            LeaseTake::TookOver(old) => Some(old),
        };
        let fallback = std::sync::Arc::clone(shared);
        Ok(LeaseTaken::Taken(Self {
            holder,
            took_over,
            released: false,
            fallback: Some(Box::new(move |run_id: RunId| {
                let released = match fallback.try_lock() {
                    Ok(mut guard) => Some(guard.lease_release(&run_id)),
                    Err(TryLockError::Poisoned(p)) => Some(p.into_inner().lease_release(&run_id)),
                    Err(TryLockError::WouldBlock) => None,
                };
                match released {
                    Some(result) => {
                        log_release(&run_id, result);
                    }
                    None => {
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            drop(handle.spawn_blocking(move || {
                                let mut guard =
                                    fallback.lock().unwrap_or_else(PoisonError::into_inner);
                                log_release(&run_id, guard.lease_release(&run_id));
                            }));
                        } else {
                            tracing::warn!(run_id = %run_id,
                                "a lease dropped while its cache was held, outside a runtime; it is left to expiry");
                        }
                    }
                }
            })),
        }))
    }

    /// The run that holds it.
    #[must_use]
    pub fn run_id(&self) -> &RunId {
        &self.holder.run_id
    }

    /// The lease row this run wrote.
    #[cfg(feature = "testing")]
    #[must_use]
    pub fn holder(&self) -> &LeaseHolder {
        &self.holder
    }

    /// The expired holder this run took the lease over from, if any.
    #[must_use]
    pub fn took_over(&self) -> Option<&LeaseHolder> {
        self.took_over.as_ref()
    }

    /// Releases the lease on `cache` (held by the caller); `false` (logged)
    /// when the store did not free it.
    pub fn release<S: CacheStore, L: DrainLock, B: CacheBackend>(
        mut self,
        cache: &mut Cache<S, L, B>,
    ) -> bool {
        self.released = true;
        log_release(self.run_id(), cache.lease_release(self.run_id()))
    }
}

/// Logs a release that did not free the lease; returns whether it did.
pub(crate) fn log_release(run_id: &RunId, result: Result<bool, CacheError>) -> bool {
    match result {
        Ok(true) => true,
        Ok(false) => {
            tracing::warn!(run_id = %run_id, "the lease was no longer this run's; nothing released");
            false
        }
        Err(e) => {
            tracing::warn!(run_id = %run_id, code = e.code(), detail = e.detail(),
                "the lease release failed; it is left to expiry");
            false
        }
    }
}

impl Drop for TickLease {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let run_id = self.holder.run_id.clone();
        match self.fallback.take() {
            Some(release) => release(run_id),
            None => {
                tracing::warn!(run_id = %run_id, "a lease was dropped unreleased; it is left to expiry");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_id_is_unix_ms_pid_and_a_counter() {
        let a = new_run_id(1_761_472_800_123).unwrap();
        let b = new_run_id(1_761_472_800_123).unwrap();
        let pid = std::process::id().to_string();
        let parts: Vec<&str> = a.as_str().split('-').collect();
        assert_eq!(parts.first(), Some(&"1761472800123"));
        assert_eq!(parts.get(1), Some(&pid.as_str()));
        assert_ne!(a, b, "the counter makes them unique");
    }
}
