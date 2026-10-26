//! The sync rows on `sc-cron`: the built-in row's job and the resolver
//! (REQ-BDC-0720, REQ-CRON-0016, ADR-SYNC-0008, ADR-CORE-0001).
//!
//! [`sync_resolver`] is the [`TargetResolver`] bd-cache-sync hands sc-cron's
//! worker (`Scheduler::spawn_worker_with`) and `Scheduler::run_due_with`.
//! sc-cron calls it before every launch: it decodes the row's payload
//! ([`SyncPayload::decode`]) and returns a Rust target, a
//! [`BuiltinSyncJob`] or a [`PythonSyncJob`]. A payload it cannot decode, a
//! Python payload on a row other than [`PYTHON_ROW`], a built-in payload on a
//! row other than [`BUILTIN_ROW`], or a Python row whose cache file fails the
//! trust check is an error: sc-cron records `RunOutcome::Failed` and leaves
//! the row active, and `schedule show` flags it.
//!
//! A `Cache` is `Send` but not `Sync` and its verbs block, so the jobs hold
//! it in an `Arc<Mutex<_>>` ([`SharedCache`]) and run its verbs on tokio's
//! blocking pool. sc-cron times a run out by dropping its future, never its
//! blocking thread; [`join_idle`] waits for a blocking step still inside the
//! cache, up to a deadline (PE-RSH-004).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;
use sc_cron::{
    Clock as CronClock, CronError, HookTarget, Job, RunOutcome, Schedule, StopDecision,
    TargetResolver,
};
use serde_json::Value;

use crate::control::StopFlag;
use crate::history::RowHistory;
use crate::payload::{BUILTIN_ROW, BuiltinPayload, PYTHON_ROW, SyncPayload};
use crate::python_job::PythonSyncJob;
use crate::run::{RunContext, RunStatus, SharedCache, run_builtin};
use crate::trust::TrustedCacheFile;

/// What the sync jobs share across launches: the cache, the clocks, the
/// files and the stop flag.
pub struct SyncContext<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> {
    /// The cache, shared with the jobs' blocking steps.
    pub cache: SharedCache<S, L, B>,
    /// sc-cron's clock (the Python hook's kill budget is measured on it).
    pub clock: Arc<C>,
    /// The cache file.
    pub cache_path: PathBuf,
    /// The beads directory given to the hook and the script, when set.
    pub beads_dir: Option<PathBuf>,
    /// Set on a signal: runs start no further step (REQ-BDC-0730).
    pub stop: StopFlag,
    /// Set to kill the hook's and the script's trees at once (see
    /// [`RunContext::kill`]).
    pub kill: StopFlag,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> std::fmt::Debug
    for SyncContext<S, L, B, C>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncContext")
            .field("cache_path", &self.cache_path)
            .field("beads_dir", &self.beads_dir)
            .finish_non_exhaustive()
    }
}

/// The resolver of the sync rows (module docs).
///
/// # Examples
///
/// ```no_run
/// # fn doc(ctx: std::sync::Arc<bd_cache_sync::SyncContext<
/// #     bd_cache_store::SqliteStore, bd_cache_store::FlockDrainLock, sc_bd_cli::BdCli,
/// #     sc_cron::SystemClock>>,
/// #     scheduler: &sc_cron::Scheduler<bd_cache_store::CacheScheduleStore, sc_cron::SystemClock, sc_cron::SystemJitter>) {
/// let worker = scheduler.spawn_worker_with(bd_cache_sync::sync_resolver(ctx));
/// # drop(worker);
/// # }
/// ```
#[must_use]
pub fn sync_resolver<S, L, B, C>(ctx: Arc<SyncContext<S, L, B, C>>) -> TargetResolver
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
{
    Arc::new(move |row: &Schedule| resolve(&ctx, row))
}

fn resolve<S, L, B, C>(ctx: &Arc<SyncContext<S, L, B, C>>, row: &Schedule) -> Result<HookTarget, CronError>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
{
    let name = row.spec.name.as_str();
    let payload = SyncPayload::decode(&row.spec.payload)
        .map_err(|e| CronError::InvalidSpec(format!("row {name}: {e}")))?;
    let history = RowHistory::of(row.last_outcome.as_ref());
    match payload {
        SyncPayload::Builtin(payload) if name == BUILTIN_ROW => {
            Ok(HookTarget::Rust(Arc::new(BuiltinSyncJob {
                cache: Arc::clone(&ctx.cache),
                payload,
                ctx: RunContext {
                    row: name.to_owned(),
                    period: crate::schedule::row_period(row),
                    cache_path: ctx.cache_path.clone(),
                    beads_dir: ctx.beads_dir.clone(),
                    stop: ctx.stop.clone(),
                    kill: ctx.kill.clone(),
                },
                history,
                clock: Arc::clone(&ctx.clock),
            })))
        }
        SyncPayload::Python(payload) if name == PYTHON_ROW => {
            // The resolver's own check; each launch checks again (ADR-SYNC-0008).
            TrustedCacheFile::check(&payload.cache_path)
                .map_err(|e| CronError::HookFailed(e.to_string()))?;
            Ok(HookTarget::Rust(Arc::new(PythonSyncJob::new(
                Arc::clone(&ctx.cache),
                payload,
                Arc::clone(&ctx.clock),
                ctx.kill.clone(),
                history,
            ))))
        }
        SyncPayload::Builtin(_) => Err(CronError::InvalidSpec(format!(
            "row {name} holds a built-in sync payload; only {BUILTIN_ROW} may"
        ))),
        SyncPayload::Python(_) => Err(CronError::InvalidSpec(format!(
            "row {name} holds a Python sync payload; only {PYTHON_ROW} may"
        ))),
    }
}

/// The built-in row's job: [`run_builtin`] with the row's payload.
pub struct BuiltinSyncJob<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> {
    cache: SharedCache<S, L, B>,
    payload: BuiltinPayload,
    ctx: RunContext,
    history: RowHistory,
    clock: Arc<C>,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> std::fmt::Debug
    for BuiltinSyncJob<S, L, B, C>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltinSyncJob")
            .field("payload", &self.payload)
            .field("ctx", &self.ctx)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<S, L, B, C> Job for BuiltinSyncJob<S, L, B, C>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
{
    async fn should_stop(&self, _payload: &Value) -> StopDecision {
        StopDecision::Continue
    }

    /// One built-in run. A skipped run is `Refused("in_flight")`; a hook
    /// failure is in the reply and the run is `Ok`; a store error is `Failed`.
    async fn run(&self, _payload: &Value, _deadline: Instant) -> RunOutcome {
        match run_builtin(&self.cache, &self.payload, &self.ctx).await {
            Ok(reply) if reply.outcome == RunStatus::Skipped => {
                RunOutcome::Refused("in_flight".to_owned())
            }
            Ok(_) => RunOutcome::Ok,
            Err(e) => {
                let now = sc_cron::Timestamp::from_datetime(self.clock.now()).to_string();
                let run_id = crate::lease::new_run_id(&sc_beads::SystemClock);
                self.history.failed(&run_id, &now, &e.to_string())
            }
        }
    }
}

/// Waits until no step of a run holds `cache`, or `deadline` passes; returns
/// whether it is free (PE-RSH-004).
///
/// For shutdown: sc-cron drops a run's future but never its blocking thread,
/// so a drain or a sync may still be inside the cache after the worker has
/// stopped. Call it from a thread that may block.
#[must_use]
pub fn join_idle<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &SharedCache<S, L, B>,
    deadline: Instant,
) -> bool {
    loop {
        match cache.try_lock() {
            Ok(_) | Err(std::sync::TryLockError::Poisoned(_)) => return true,
            Err(std::sync::TryLockError::WouldBlock) => {}
        }
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        // The cache mutex has no waiter to notify: re-try at a short interval.
        std::thread::sleep((deadline - now).min(Duration::from_millis(5)));
    }
}
