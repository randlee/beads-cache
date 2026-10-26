//! One sync at a time: [`TickLease`], the RAII guard over the cache file's
//! `tick_lease` row, and the run id (REQ-BDC-0722, ADR-SYNC-0008).
//!
//! Every run takes the lease first: the built-in row's, the Python row's, an
//! on-demand `tick` and the immediate run of `sync use-python`. It is taken
//! under `BEGIN IMMEDIATE` (the store's `lease_take`), refused while a live
//! lease is held, taken over once expired, and expires at the run's deadline
//! plus [`LEASE_MARGIN`]. It is released on every exit path: explicitly by
//! [`TickLease::release`], or, on a run aborted mid-await, by the guard's drop,
//! which hands the release to `spawn_blocking` when a tokio runtime is
//! current. A release that fails or does not run is logged and left to
//! expiry; a `kill -9` is covered only by expiry.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{CacheError, CacheStore, DrainLock, LeaseHolder, LeaseTake, RunId};
use sc_beads::{CacheBackend, Clock, Deadline, Timestamp};

/// How long after a run's deadline its lease expires (REQ-BDC-0722).
pub const LEASE_MARGIN: Duration = Duration::from_secs(10);

/// A run id, `<unix-ms>-<pid>-<counter>` (ADR-SYNC-0008).
///
/// The time is the cache's clock; the counter makes ids of one process
/// unique within a millisecond.
#[must_use]
pub fn new_run_id(clock: &dyn Clock) -> RunId {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ms = clock.now_utc().unix().saturating_mul(1000);
    // Never empty, so `RunId::new` cannot refuse it; the fallback is unreachable in practice.
    RunId::new(format!("{ms}-{}-{n}", std::process::id())).unwrap_or_else(|_| fallback_run_id(n))
}

#[expect(
    clippy::expect_used,
    reason = "a fixed non-empty literal is always a valid run id"
)]
fn fallback_run_id(n: u64) -> RunId {
    RunId::new(format!("run-{n}")).expect("non-empty")
}

/// This host's name, for the lease row (`HOSTNAME`, `COMPUTERNAME`, or `localhost`).
fn host() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| "localhost".to_owned())
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
/// The run functions take `&TickLease`, so no sync work runs without one.
#[must_use = "a lease is released by TickLease::release; dropping it leaves the release to its fallback or to expiry"]
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
    /// Takes the lease for `run_id`, expiring [`LEASE_MARGIN`] after `deadline`.
    ///
    /// # Errors
    ///
    /// The store's errors.
    pub fn take<S: CacheStore, L: DrainLock, B: CacheBackend>(
        cache: &mut Cache<S, L, B>,
        run_id: RunId,
        deadline: Deadline,
    ) -> Result<LeaseTaken, CacheError> {
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
        Ok(match cache.lease_take(&holder)? {
            LeaseTake::Held(other) => LeaseTaken::Held(other),
            LeaseTake::Taken => LeaseTaken::Taken(Self::new(holder, None)),
            LeaseTake::TookOver(old) => LeaseTaken::Taken(Self::new(holder, Some(old))),
        })
    }

    fn new(holder: LeaseHolder, took_over: Option<LeaseHolder>) -> Self {
        Self {
            holder,
            took_over,
            released: false,
            fallback: None,
        }
    }

    /// The run that holds it.
    #[must_use]
    pub fn run_id(&self) -> &RunId {
        &self.holder.run_id
    }

    /// The lease row this run wrote.
    #[must_use]
    pub fn holder(&self) -> &LeaseHolder {
        &self.holder
    }

    /// The expired holder this run took the lease over from, if any.
    #[must_use]
    pub fn took_over(&self) -> Option<&LeaseHolder> {
        self.took_over.as_ref()
    }

    /// Sets the release the guard runs if it drops unreleased (a run aborted
    /// mid-await): on `spawn_blocking` when a tokio runtime is current, else inline.
    pub fn with_fallback(mut self, release: impl FnOnce(RunId) + Send + Sync + 'static) -> Self {
        self.fallback = Some(Box::new(release));
        self
    }

    /// Releases the lease; `false` (logged) when the store did not free it.
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
        let Some(release) = self.fallback.take() else {
            tracing::warn!(run_id = %run_id, "a lease was dropped unreleased; it is left to expiry");
            return;
        };
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            drop(handle.spawn_blocking(move || release(run_id)));
        } else {
            release(run_id);
        }
    }
}
