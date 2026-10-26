//! `cron run` and `cron run --once`: the sync rows on sc-cron's worker
//! (REQ-BDC-0720, REQ-BDC-0730, ADR-SYNC-0008).
//!
//! [`Schedules::cron_run`] runs sc-cron's worker over the cache file's
//! schedule table with the sync resolver and `MissedTickPolicy::Skip`, until
//! the stop flag is set (a signal) or the worker ends by itself (a fault,
//! reported). On a signal it starts no new run and gives the runs in flight,
//! the built-in run's bd calls, the hook and the Python script alike, up to
//! the shutdown timeout `T`. Only when `T` expires does it abort the runs
//! still going (dropping their supervisors kills the hook's and the script's
//! process trees) and call the caller's `on_expiry` (bdc's request that every
//! bounded bd call stop), then wait, for at most `T` again, for the killed
//! blocking steps to leave the cache and the lease to be released. A stop
//! therefore takes at most `2 × T` (`--shutdown-timeout-ms 0` kills at once;
//! a lease whose release then has no time left is left to expiry). A second
//! stop request during the wait ([`Schedules::cron_run_hurried`]) does what the
//! expiry does, at once.
//! [`Schedules::cron_run_once`] runs what is due once and waits for each run
//! to its own deadline.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bd_cache_api::ApiError;
use bd_cache_model::{CacheError, CacheStore, DrainLock, TickRunReason};
use sc_beads::CacheBackend;
use sc_cron::{Clock as CronClock, JitterSource, RunOutcome, ScheduleStore};
use serde::Serialize;
use serde_json::json;

use crate::control::StopFlag;
use crate::job::{RowFailure, SyncContext, join_idle, sync_resolver};
use crate::lease::hold;
use crate::payload::SyncRow;
use crate::run::blocking;
use crate::schedule::{Schedules, schedule_store_failed};
use crate::text::cut;

/// How often `cron run` looks whether its worker ended by itself: the
/// worker has no completion signal, only `is_finished`.
const WORKER_CHECK: Duration = Duration::from_millis(250);

/// How `cron run` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CronStopped {
    /// Every run in flight finished before the shutdown timeout.
    pub drained: bool,
    /// No blocking step was left inside the cache afterwards.
    pub idle: bool,
    /// The worker ended by itself, before any signal: a fault, logged.
    pub worker_ended: bool,
    /// Until when the rest of the shutdown may take (the runtime's
    /// blocking pool included): the second `T` of the module docs.
    pub finish_by: Instant,
}

/// One run `cron run --once` started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DueRunView {
    /// The row.
    pub row: String,
    /// Its outcome, as sc-cron recorded it.
    pub outcome: String,
}

/// What `cron run --once` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronOnce {
    /// Every run it started.
    pub ran: Vec<DueRunView>,
    failed: Vec<RowFailure>,
}

impl CronOnce {
    /// The reply's `data`: `{once: true, ran: [{row, outcome}]}`.
    #[must_use]
    pub fn data(&self) -> serde_json::Value {
        json!({"once": true, "ran": self.ran})
    }

    /// `TICK_RUN_FAILED` when a run failed or timed out: the first failure's
    /// row, run id and reason, and `details.rows` listing every failed row
    /// with its own (REQ-BDC-0730).
    #[must_use]
    pub fn error(&self) -> Option<ApiError> {
        let first = self.failed.first()?;
        let rows: Vec<serde_json::Value> = self
            .failed
            .iter()
            .map(|f| {
                json!({
                    "row": f.row.as_str(),
                    "run_id": f.run_id.as_str(),
                    "reason": f.reason.as_str(),
                    "exit_status": f.exit_status,
                })
            })
            .collect();
        let detail = self
            .failed
            .iter()
            .map(|f| format!("{}: {}", f.row.as_str(), cut(&f.detail, 1024)))
            .collect::<Vec<_>>()
            .join("; ");
        let e = CacheError::TickRunFailed {
            detail,
            row: first.row.as_str().to_owned(),
            run_id: first.run_id.clone(),
            reason: first.reason,
            exit_status: first.exit_status,
            stderr: first.stderr.clone(),
            restored: None,
        };
        Some(ApiError::from(&e).with_detail("rows", rows))
    }
}

impl<St, S, L, B, C, J> Schedules<St, S, L, B, C, J>
where
    St: ScheduleStore + 'static,
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    /// Runs the worker until `stop` is set or it ends, then shuts it down
    /// within `shutdown_timeout`, calling `on_expiry` if that expires (module docs).
    ///
    /// # Panics
    ///
    /// As `Scheduler::spawn_worker_with`: outside a tokio runtime, or with a
    /// worker of this scheduler already running.
    pub async fn cron_run(
        &self,
        stop: StopFlag,
        shutdown_timeout: Duration,
        on_expiry: impl FnOnce() + Send,
    ) -> CronStopped {
        self.cron_run_hurried(stop, StopFlag::new(), shutdown_timeout, on_expiry)
            .await
    }

    /// As [`cron_run`](Self::cron_run), and `hurry` ends the grace period
    /// early: once it is set while the runs in flight are being waited for,
    /// they are killed and `on_expiry` is called at once, as when
    /// `shutdown_timeout` expires (bdc sets it on a second signal). The
    /// bound of the rest of the stop is unchanged.
    ///
    /// # Panics
    ///
    /// As [`cron_run`](Self::cron_run).
    pub async fn cron_run_hurried(
        &self,
        stop: StopFlag,
        hurry: StopFlag,
        shutdown_timeout: Duration,
        on_expiry: impl FnOnce() + Send,
    ) -> CronStopped {
        let kill = StopFlag::new();
        let ctx = Arc::new(SyncContext::new(
            Arc::clone(&self.cache),
            Arc::clone(&self.clock),
            self.files.clone(),
            stop.clone(),
            kill.clone(),
        ));
        self.warn_at_startup().await;
        let worker = self
            .scheduler
            .spawn_worker_with(sync_resolver(Arc::clone(&ctx)));
        tracing::info!(action = "cron_run", "worker started");
        let worker_ended = loop {
            tokio::select! {
                () = stop.wait() => break false,
                () = tokio::time::sleep(WORKER_CHECK) => {
                    if worker.is_finished() {
                        break true;
                    }
                }
            }
        };
        if worker_ended {
            tracing::error!(
                action = "cron_run",
                "the worker ended without a signal (it panicked or failed); cron run stops"
            );
        } else {
            tracing::info!(
                action = "cron_run",
                shutdown_timeout_ms =
                    u64::try_from(shutdown_timeout.as_millis()).unwrap_or(u64::MAX),
                "stopping: no new run; waiting for the runs in flight"
            );
        }
        let mut on_expiry = Some(on_expiry);
        let shutdown = worker.shutdown(tokio::time::Instant::now() + shutdown_timeout);
        tokio::pin!(shutdown);
        let mut hurried = false;
        let mut drained = tokio::select! {
            drained = &mut shutdown => drained,
            () = hurry.wait() => {
                tracing::warn!(
                    action = "cron_run",
                    "stop requested again: the runs in flight are killed with their process trees now"
                );
                hurried = true;
                kill.stop();
                if let Some(on_expiry) = on_expiry.take() {
                    on_expiry();
                }
                // The wait for them to end stays bounded by the shutdown timeout.
                (&mut shutdown).await
            }
        };
        if !drained && !hurried {
            tracing::warn!(
                action = "cron_run",
                "runs still going at the shutdown timeout were killed with their process trees"
            );
            kill.stop();
            if let Some(on_expiry) = on_expiry.take() {
                on_expiry();
            }
        }
        // Runs killed on a second signal were not drained, whenever they ended.
        drained &= !hurried;
        let finish_by = Instant::now()
            .checked_add(shutdown_timeout)
            .unwrap_or_else(Instant::now);
        let cache = Arc::clone(&self.cache);
        let idle = tokio::task::spawn_blocking(move || join_idle(&cache, finish_by))
            .await
            .unwrap_or(false);
        tracing::info!(action = "cron_run", drained, idle, "worker stopped");
        CronStopped {
            drained,
            idle,
            worker_ended,
            finish_by,
        }
    }

    /// The checks `cron run` makes before it serves (REQ-BDC-0730): one
    /// `WARN` for no enabled sync row, and one per enabled sync row whose
    /// payload does not decode or whose cache file fails the trust check,
    /// so a row that cannot run is reported at once, not at its first tick.
    /// Returns the warnings, as logged.
    #[doc(hidden)]
    pub async fn warn_at_startup(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        match self.rows().await {
            Err(e) => warnings.push(format!("the schedule table cannot be read: {e}")),
            Ok(rows) => {
                if rows.mode() == crate::schedule::SyncMode::None {
                    warnings.push(
                        "no sync row is enabled: cron run runs no sync until one is".to_owned(),
                    );
                }
                for (kind, row) in [
                    (SyncRow::Builtin, rows.builtin.as_ref()),
                    (SyncRow::Python, rows.python.as_ref()),
                ] {
                    let Some(row) = row.filter(|r| r.enabled) else {
                        continue;
                    };
                    match crate::payload::SyncPayload::decode(&row.spec.payload) {
                        Err(e) => warnings.push(format!(
                            "the enabled row {} cannot run: its payload does not decode: {e}",
                            kind.as_str()
                        )),
                        Ok(crate::payload::SyncPayload::Python(_)) => {
                            if let Err(e) =
                                crate::trust::TrustedCacheFile::check(&self.files.cache_path)
                            {
                                warnings.push(format!(
                                    "the enabled row {} cannot run: {e}",
                                    kind.as_str()
                                ));
                            }
                        }
                        Ok(crate::payload::SyncPayload::Builtin(_)) => {}
                    }
                }
            }
        }
        for w in &warnings {
            tracing::warn!(action = "cron_run", "{w}");
        }
        warnings
    }

    /// Runs every due row once and waits for each to its own deadline
    /// (REQ-BDC-0730); a signal (`stop`) stops the runs and kills their trees.
    ///
    /// # Errors
    ///
    /// `STORE_FAILED` when the table cannot be read. A failed run is in
    /// [`CronOnce::error`].
    pub async fn cron_run_once(&self, stop: StopFlag) -> Result<CronOnce, CacheError> {
        self.cron_run_once_expiring(stop, StopFlag::new()).await
    }

    /// [`cron_run_once`](Self::cron_run_once) with a test seam: setting
    /// `expire` cuts a running built-in hook as if its deadline had come
    /// ([`RunContext::expire`](crate::RunContext::expire)). Never used in
    /// production.
    ///
    /// # Errors
    ///
    /// As [`cron_run_once`](Self::cron_run_once).
    #[doc(hidden)]
    pub async fn cron_run_once_expiring(
        &self,
        stop: StopFlag,
        expire: StopFlag,
    ) -> Result<CronOnce, CacheError> {
        let mut ctx = SyncContext::new(
            Arc::clone(&self.cache),
            Arc::clone(&self.clock),
            self.files.clone(),
            stop.clone(),
            stop,
        );
        ctx.expire = expire;
        let ctx = Arc::new(ctx);
        let runs = self
            .scheduler
            .run_due_with(sync_resolver(Arc::clone(&ctx)))
            .await
            .map_err(schedule_store_failed)?;
        let mut failed = ctx.take_failures();
        for run in &runs {
            let Some(row) = SyncRow::of(run.name.as_str()) else {
                continue;
            };
            // A run sc-cron itself ended (its deadline) has no failure of
            // the job's: its run id is the row's latest, from its record.
            if matches!(run.outcome, RunOutcome::TimedOut) && !failed.iter().any(|f| f.row == row) {
                let cache = Arc::clone(&self.cache);
                let record = blocking(move || hold(&cache)?.sync_run(row.as_str())).await?;
                if let Some(record) = record {
                    failed.push(RowFailure {
                        row,
                        run_id: record.run_id,
                        reason: TickRunReason::Timeout,
                        exit_status: None,
                        stderr: String::new(),
                        detail: "the run was killed at its deadline".to_owned(),
                    });
                }
            }
        }
        let ran = runs
            .iter()
            .map(|r| DueRunView {
                row: r.name.as_str().to_owned(),
                outcome: r.outcome.to_string(),
            })
            .collect();
        // Only failures of runs that ended failed: a skipped run is not one.
        failed.retain(|f| {
            runs.iter().any(|r| {
                r.name.as_str() == f.row.as_str()
                    && matches!(r.outcome, RunOutcome::Failed(_) | RunOutcome::TimedOut)
            })
        });
        Ok(CronOnce { ran, failed })
    }
}
