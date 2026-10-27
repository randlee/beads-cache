//! The sync rows on `sc-cron`: the built-in row's job and the resolver
//! (REQ-BDC-0720, REQ-CRON-0016, ADR-SYNC-0008, ADR-CORE-0001).
//!
//! [`sync_resolver`] is the [`TargetResolver`] bd-cache-sync hands sc-cron's
//! worker (`Scheduler::spawn_worker_with`) and `Scheduler::run_due_with`.
//! sc-cron calls it before every launch: it mints the launch's run id on
//! sc-cron's clock (shared by the stop check, the lease, every logged step,
//! the run record and the outcome), decodes the row's payload
//! ([`SyncPayload::decode`]) and returns a Rust target: a [`BuiltinSyncJob`],
//! a [`PythonSyncJob`], or, for a sync row whose payload does not decode,
//! names the other row's target, or lives in a cache file that fails the
//! trust check, a job that records the run as failed (so the row's run
//! record counts it) and returns `RunOutcome::Failed`. A row of another name
//! is not a sync row: a sync payload on it is refused, and any other payload
//! has no target here; sc-cron records both `Failed` and leaves the row
//! active, beside the sync rows. The trust check is made on the cache file
//! the worker opened, never on a path the row holds (REQ-BDC-0723).
//!
//! A `Cache` is `Send` but not `Sync` and its verbs block, so the jobs hold
//! it in an `Arc<Mutex<_>>` ([`SharedCache`]) and run its verbs on tokio's
//! blocking pool. sc-cron times a run out by dropping its future, never its
//! blocking thread; [`join_idle`] waits for a blocking step still inside the
//! cache, up to a deadline (PE-RSH-004).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bd_cache_model::{CacheError, CacheStore, DrainLock, RunId, SyncRunEnd, TickRunReason};
use sc_beads::CacheBackend;
use sc_cron::{
    Clock as CronClock, CronError, HookTarget, Job, RunOutcome, Schedule, StopDecision,
    TargetResolver,
};
use serde_json::Value;

use crate::control::StopFlag;
use crate::history::RunRecord;
use crate::lease::run_id_on;
use crate::payload::{BuiltinPayload, SyncPayload, SyncRow};
use crate::python_job::{PythonSyncJob, RunFiles};
use crate::run::{RunContext, RunStatus, SharedCache, run_builtin};
use crate::trust::TrustedCacheFile;

/// One failed run of a sync row, as `cron run --once` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowFailure {
    pub(crate) row: SyncRow,
    pub(crate) run_id: RunId,
    pub(crate) reason: TickRunReason,
    pub(crate) exit_status: Option<i32>,
    pub(crate) stderr: String,
    pub(crate) detail: String,
}

impl RowFailure {
    /// The failure a `TICK_RUN_FAILED` describes.
    pub(crate) fn of(row: SyncRow, e: &CacheError) -> Option<Self> {
        match e {
            CacheError::TickRunFailed {
                detail,
                run_id,
                reason,
                exit_status,
                stderr,
                ..
            } => Some(Self {
                row,
                run_id: run_id.clone(),
                reason: *reason,
                exit_status: *exit_status,
                stderr: stderr.clone(),
                detail: detail.clone(),
            }),
            _ => None,
        }
    }
}

/// The failures the jobs of one context saw, newest last.
pub(crate) type FailureLog = Arc<Mutex<Vec<RowFailure>>>;

/// The files a sync run uses, as the front end opened them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncFiles {
    /// The cache file the process opened.
    pub cache_path: PathBuf,
    /// The beads directory given to the hook and the script, when set.
    pub beads_dir: Option<PathBuf>,
    /// The repository root (the Python row's payload `repo_root`).
    pub repo_root: PathBuf,
}

/// What the sync jobs share across launches: the cache, the clock, the
/// files and the flags.
pub struct SyncContext<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> {
    /// The cache, shared with the jobs' blocking steps.
    pub cache: SharedCache<S, L, B>,
    /// sc-cron's clock (the run ids and the Python hook's kill budget are on it).
    pub clock: Arc<C>,
    /// The files.
    pub files: SyncFiles,
    /// Set on a signal: runs start no further step (REQ-BDC-0730).
    pub stop: StopFlag,
    /// Set to kill the hook's and the script's trees at once (see
    /// [`RunContext::kill`]).
    pub kill: StopFlag,
    /// Test seam, never set in production (see [`RunContext::expire`]).
    #[doc(hidden)]
    pub expire: StopFlag,
    pub(crate) failures: FailureLog,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> SyncContext<S, L, B, C> {
    /// The context over `cache` and `files`, on `clock`, with its two flags.
    #[must_use]
    pub fn new(
        cache: SharedCache<S, L, B>,
        clock: Arc<C>,
        files: SyncFiles,
        stop: StopFlag,
        kill: StopFlag,
    ) -> Self {
        Self {
            cache,
            clock,
            files,
            stop,
            kill,
            expire: StopFlag::new(),
            failures: FailureLog::default(),
        }
    }

    /// Takes the failures the jobs recorded so far.
    pub(crate) fn take_failures(&self) -> Vec<RowFailure> {
        std::mem::take(&mut *self.failures.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> std::fmt::Debug
    for SyncContext<S, L, B, C>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncContext")
            .field("files", &self.files)
            .finish_non_exhaustive()
    }
}

/// The resolver of the sync rows (module docs).
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

fn resolve<S, L, B, C>(
    ctx: &Arc<SyncContext<S, L, B, C>>,
    row: &Schedule,
) -> Result<HookTarget, CronError>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
{
    let name = row.spec.name.as_str();
    let payload = SyncPayload::decode(&row.spec.payload);
    let Some(sync_row) = SyncRow::of(name) else {
        // Not a sync row: a sync payload on it is refused (ANC-008); any
        // other payload is not this resolver's to run.
        return Err(CronError::InvalidSpec(match payload {
            Ok(SyncPayload::Builtin(_)) => format!(
                "row {name} holds a built-in sync payload; only {} may",
                SyncRow::Builtin.as_str()
            ),
            Ok(SyncPayload::Python(_)) => format!(
                "row {name} holds a Python sync payload; only {} may",
                SyncRow::Python.as_str()
            ),
            Err(e) => format!("row {name} is not a sync row and has no target here: {e}"),
        }));
    };
    let run_id = run_id_on(&*ctx.clock).map_err(|e| CronError::InvalidSpec(e.to_string()))?;
    let unrunnable = |reason, detail: String| -> Result<HookTarget, CronError> {
        Ok(HookTarget::Rust(Arc::new(UnrunnableJob {
            record: RunRecord::new(Arc::clone(&ctx.cache), sync_row, run_id.clone()),
            failures: Arc::clone(&ctx.failures),
            error: crate::python_job::run_failed(&run_id, reason, None, String::new(), detail),
            row: sync_row,
        })))
    };
    match (sync_row, payload) {
        (SyncRow::Builtin, Ok(SyncPayload::Builtin(payload))) => {
            Ok(HookTarget::Rust(Arc::new(BuiltinSyncJob {
                cache: Arc::clone(&ctx.cache),
                payload,
                ctx: RunContext {
                    row: name.to_owned(),
                    period: crate::schedule::row_period(row),
                    cache_path: ctx.files.cache_path.clone(),
                    beads_dir: ctx.files.beads_dir.clone(),
                    repo_root: ctx.files.repo_root.clone(),
                    stop: ctx.stop.clone(),
                    kill: ctx.kill.clone(),
                    expire: ctx.expire.clone(),
                },
                record: RunRecord::new(Arc::clone(&ctx.cache), sync_row, run_id),
                failures: Arc::clone(&ctx.failures),
            })))
        }
        (SyncRow::Python, Ok(SyncPayload::Python(payload))) => {
            // The resolver's own check, on the file the worker opened; each
            // launch checks again (ADR-SYNC-0008).
            if let Err(e) = TrustedCacheFile::check(&ctx.files.cache_path) {
                return unrunnable(TickRunReason::Untrusted, e.to_string());
            }
            Ok(HookTarget::Rust(Arc::new(PythonSyncJob::new(
                Arc::clone(&ctx.cache),
                payload,
                RunFiles {
                    cache_path: ctx.files.cache_path.clone(),
                    beads_dir: ctx.files.beads_dir.clone(),
                    repo_root: ctx.files.repo_root.clone(),
                },
                Arc::clone(&ctx.clock),
                ctx.kill.clone(),
                run_id,
                Arc::clone(&ctx.failures),
            ))))
        }
        (_, Err(e)) => unrunnable(
            TickRunReason::Spawn,
            format!("row {name}: its payload does not decode: {e}"),
        ),
        (row, Ok(_)) => unrunnable(
            TickRunReason::Spawn,
            format!(
                "row {name} holds the {} row's payload",
                row.other().as_str()
            ),
        ),
    }
}

/// The built-in row's job: [`run_builtin`] with the row's payload.
pub struct BuiltinSyncJob<S: CacheStore, L: DrainLock, B: CacheBackend> {
    cache: SharedCache<S, L, B>,
    payload: BuiltinPayload,
    ctx: RunContext,
    record: RunRecord<S, L, B>,
    failures: crate::job::FailureLog,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> std::fmt::Debug for BuiltinSyncJob<S, L, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuiltinSyncJob")
            .field("payload", &self.payload)
            .field("ctx", &self.ctx)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<S, L, B> Job for BuiltinSyncJob<S, L, B>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    async fn should_stop(&self, _payload: &Value) -> StopDecision {
        StopDecision::Continue
    }

    /// One built-in run. A skipped run is `Refused("in_flight")`; a hook
    /// failure is in the reply and the run is `Ok`; a store error is `Failed`.
    async fn run(&self, _payload: &Value, _deadline: Instant) -> RunOutcome {
        self.record.start().await;
        let run_id = self.record.run_id().clone();
        match run_builtin(&self.cache, &self.payload, &self.ctx, run_id.clone()).await {
            Ok(reply) if reply.outcome == RunStatus::Skipped => {
                self.record.finish(SyncRunEnd::Skipped).await;
                RunOutcome::Refused("in_flight".to_owned())
            }
            Ok(_) => {
                self.record.finish(SyncRunEnd::Ok).await;
                RunOutcome::Ok
            }
            Err(e) => {
                self.record.finish(SyncRunEnd::Failed).await;
                self.failures
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(RowFailure {
                        row: SyncRow::Builtin,
                        run_id,
                        reason: TickRunReason::Exit,
                        exit_status: None,
                        stderr: String::new(),
                        detail: e.to_string(),
                    });
                RunOutcome::Failed(e.to_string())
            }
        }
    }
}

/// The job of a sync row the resolver could not run: it records the run as
/// failed and returns the failure (module docs).
struct UnrunnableJob<S: CacheStore, L: DrainLock, B: CacheBackend> {
    record: RunRecord<S, L, B>,
    failures: FailureLog,
    error: CacheError,
    row: SyncRow,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> std::fmt::Debug for UnrunnableJob<S, L, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnrunnableJob")
            .field("row", &self.row)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<S, L, B> Job for UnrunnableJob<S, L, B>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    async fn should_stop(&self, _payload: &Value) -> StopDecision {
        StopDecision::Continue
    }

    async fn run(&self, _payload: &Value, _deadline: Instant) -> RunOutcome {
        self.record.start().await;
        self.record.finish(SyncRunEnd::Failed).await;
        if let Some(failure) = RowFailure::of(self.row, &self.error) {
            self.failures
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(failure);
        }
        tracing::warn!(row = self.row.as_str(), run_id = %self.record.run_id(),
            code = self.error.code(), detail = self.error.detail(), "the row cannot run");
        RunOutcome::Failed(format!("resolve: {}", self.error.detail()))
    }
}

/// Waits until no step of a run holds `cache`, or `deadline` passes; returns
/// whether it is free (PE-RSH-004).
///
/// For shutdown: sc-cron drops a run's future but never its blocking thread,
/// so a drain or a sync may still be inside the cache after the worker has
/// stopped. Call it from a thread that may block. A poisoned mutex is idle
/// (the lease module's poison policy).
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
