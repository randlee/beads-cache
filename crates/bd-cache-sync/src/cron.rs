//! `cron run` and `cron run --once`: the sync rows on sc-cron's worker
//! (REQ-BDC-0720, REQ-BDC-0730, ADR-SYNC-0008).
//!
//! [`cron_run`] runs sc-cron's worker over the cache file's schedule table
//! with [`sync_resolver`] and `MissedTickPolicy::Skip`, until the context's
//! stop flag is set (a signal). Then it starts no new run, waits for the runs
//! in flight up to the shutdown timeout, and aborts those still going, which
//! drops their supervisors (killing the hook's and the Python row's process
//! trees) and their lease guards (releasing the lease); the built-in run's
//! blocking part sees the stop flag and returns. [`cron_run_once`] runs what
//! is due once and waits for each run to its own deadline.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bd_cache_model::{CacheError, CacheStore, DrainLock, RunId, TickRunReason};
use sc_beads::CacheBackend;
use sc_cron::{Clock as CronClock, DueRun, JitterSource, MissedTickPolicy, RunOutcome, ScheduleStore, Scheduler};

use crate::history::RowHistory;
use crate::job::{SyncContext, join_idle, sync_resolver};
use crate::schedule::schedule_store_failed;

/// The default `--shutdown-timeout-ms` (REQ-BDC-0730).
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// The largest `--shutdown-timeout-ms` (REQ-BDC-0730).
pub const MAX_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(600);

/// How `cron run` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CronStopped {
    /// Every run in flight finished before the shutdown timeout.
    pub drained: bool,
    /// No blocking step was left inside the cache afterwards.
    pub idle: bool,
}

/// Runs the worker until `ctx.stop` is set, then shuts it down (module docs).
///
/// # Panics
///
/// As `Scheduler::spawn_worker_with`: outside a tokio runtime, or with a
/// worker of this scheduler already running.
pub async fn cron_run<St, S, L, B, C, J>(
    scheduler: Scheduler<St, C, J>,
    ctx: Arc<SyncContext<S, L, B, C>>,
    shutdown_timeout: Duration,
) -> CronStopped
where
    St: ScheduleStore + 'static,
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    let scheduler = scheduler.with_missed_tick_policy(MissedTickPolicy::Skip);
    let worker = scheduler.spawn_worker_with(sync_resolver(Arc::clone(&ctx)));
    tracing::info!(action = "cron_run", "worker started");
    ctx.stop.wait().await;
    tracing::info!(action = "cron_run", shutdown_timeout_ms = u64::try_from(shutdown_timeout.as_millis()).unwrap_or(u64::MAX),
        "stopping: no new run; waiting for the runs in flight");
    let drained = worker
        .shutdown(tokio::time::Instant::now() + shutdown_timeout)
        .await;
    if !drained {
        tracing::warn!(action = "cron_run", "runs still going at the shutdown timeout were killed with their process trees");
        ctx.kill.stop();
    }
    let cache = Arc::clone(&ctx.cache);
    let idle = tokio::task::spawn_blocking(move || {
        join_idle(&cache, Instant::now() + DEFAULT_SHUTDOWN_TIMEOUT)
    })
    .await
    .unwrap_or(false);
    tracing::info!(action = "cron_run", drained, idle, "worker stopped");
    CronStopped { drained, idle }
}

/// Runs every due row once and waits for each to its own deadline (REQ-BDC-0730).
///
/// # Errors
///
/// `TICK_RUN_FAILED` naming each row whose run failed or timed out (the
/// others ran); `STORE_FAILED` when the table cannot be read.
pub async fn cron_run_once<St, S, L, B, C, J>(
    scheduler: &Scheduler<St, C, J>,
    ctx: Arc<SyncContext<S, L, B, C>>,
) -> Result<Vec<DueRun>, CacheError>
where
    St: ScheduleStore + 'static,
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    let runs = scheduler
        .run_due_with(sync_resolver(ctx))
        .await
        .map_err(schedule_store_failed)?;
    let failed: Vec<&DueRun> = runs
        .iter()
        .filter(|r| matches!(r.outcome, RunOutcome::Failed(_) | RunOutcome::TimedOut))
        .collect();
    let Some(first) = failed.first() else {
        return Ok(runs);
    };
    let history = RowHistory::of(Some(&first.outcome));
    let run_id = history
        .run_id
        .and_then(|r| RunId::new(r).ok())
        .unwrap_or_else(|| crate::lease::new_run_id(&sc_beads::SystemClock));
    let rows: Vec<String> = failed.iter().map(|r| r.name.to_string()).collect();
    let detail = failed
        .iter()
        .map(|r| format!("{}: {}", r.name, crate::text::cut(&r.outcome.to_string(), 1024)))
        .collect::<Vec<_>>()
        .join("; ");
    Err(CacheError::TickRunFailed {
        detail,
        row: rows.join(","),
        run_id,
        reason: if matches!(first.outcome, RunOutcome::TimedOut) {
            TickRunReason::Timeout
        } else {
            TickRunReason::Exit
        },
        exit_status: None,
        stderr: String::new(),
        restored: None,
    })
}
