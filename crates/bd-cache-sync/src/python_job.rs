//! The Python sync row's job (REQ-BDC-0725, REQ-BDC-0722, ADR-SYNC-0008).
//!
//! [`PythonSyncJob`] runs the consumer's module with sc-cron's Python hook
//! protocol (`sc-cron.hook.v1`) through [`sc_cron::PythonHook`], configured
//! with the environment allowlist of REQ-BDC-0723: `cron.stop`, then, on
//! `continue`, `cron.tick` under the lease, each from a fresh temp cwd with
//! one JSON object on stdin whose `payload` is the row's payload plus the
//! run's `run_id` and `repo_root`, its `cache_path` the file the worker
//! opened. The trust check runs before each launch on that opened file,
//! never on a path the row holds. The run is judged by the script's exit
//! status.
//!
//! One core serves both paths: [`PythonSyncJob::stop_check`] and the tick
//! launch judge and classify each output, every child-derived text redacted
//! with the row's `--env` values, and [`outcome_of`] maps a result to
//! sc-cron's [`RunOutcome`] in one place. The scheduled path is sc-cron's
//! [`Job`]: `should_stop` (the stop check, before the lease, so two workers
//! may each launch `cron.stop`, which has no side effects) then `run` (the
//! lease, then `cron.tick`). The immediate run of `sync use-python` is
//! [`PythonSyncJob::run_under`] with the lease the command already holds;
//! the command records its outcome on the row as a worker run would.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use async_trait::async_trait;
use bd_cache_model::{CacheError, CacheStore, DrainLock, RunId, SyncRunEnd, TickRunReason};
use sc_beads::{CacheBackend, Deadline};
use sc_cron::{
    Clock as CronClock, HookCall, HookEvent, HookOutput, HookStatus, Job, RunOutcome, StopDecision,
};
use serde_json::Value;

use crate::control::StopFlag;
use crate::history::RunRecord;
use crate::job::{FailureLog, RowFailure};
use crate::lease::{LeaseTaken, TickLease, hold_within};
use crate::payload::{PYTHON_ROW, PythonPayload, SyncRow};
use crate::run::{SharedCache, blocking};
use crate::text::Redactor;
use crate::trust::TrustedCacheFile;

/// What one launch of the script gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonRun {
    /// The run.
    pub run_id: RunId,
    /// Its exit status, when it exited.
    pub exit_status: Option<i32>,
    /// Its stderr, redacted and cut to 8 KiB (REQ-BDC-0724).
    pub stderr: String,
}

/// The files a run of the Python row opens: never the row's own paths.
#[derive(Debug, Clone)]
pub(crate) struct RunFiles {
    /// The cache file the worker opened: the trust check's subject and the
    /// script's `BD_CACHE_PATH` and payload `cache_path`.
    pub(crate) cache_path: PathBuf,
    /// The beads directory the worker resolved (`BEADS_DIR`).
    pub(crate) beads_dir: Option<PathBuf>,
    /// The repository root (the payload's `repo_root`).
    pub(crate) repo_root: PathBuf,
}

/// The Python row's job, built per launch by the resolver.
///
/// sc-cron calls `should_stop` and then `run` on the one instance it got
/// from the resolver for one launch (the [`Job`] contract), through `&self`;
/// a stop check that did not decide therefore leaves its failure in
/// `undecided` (a `Mutex`, since `Job` takes `&self`) for `run` to record.
pub struct PythonSyncJob<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> {
    cache: SharedCache<S, L, B>,
    payload: PythonPayload,
    files: RunFiles,
    clock: Arc<C>,
    kill: StopFlag,
    record: RunRecord<S, L, B>,
    redactor: Redactor,
    failures: FailureLog,
    /// A stop check that did not decide: its failure, which `run` records.
    undecided: Mutex<Option<CacheError>>,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> std::fmt::Debug
    for PythonSyncJob<S, L, B, C>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PythonSyncJob")
            .field("payload", &self.payload)
            .field("files", &self.files)
            .finish_non_exhaustive()
    }
}

impl<S, L, B, C> PythonSyncJob<S, L, B, C>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
{
    /// The job of one launch, run `run_id`.
    pub(crate) fn new(
        cache: SharedCache<S, L, B>,
        payload: PythonPayload,
        files: RunFiles,
        clock: Arc<C>,
        kill: StopFlag,
        run_id: RunId,
        failures: FailureLog,
    ) -> Self {
        let redactor = Redactor::of(payload.env().names());
        Self {
            record: RunRecord::new(Arc::clone(&cache), SyncRow::Python, run_id),
            cache,
            payload,
            files,
            clock,
            kill,
            redactor,
            failures,
            undecided: Mutex::new(None),
        }
    }

    fn run_id(&self) -> &RunId {
        self.record.run_id()
    }

    fn deadline(&self) -> Instant {
        let start = self.clock.monotonic_now();
        start
            .checked_add(self.payload.timeout().get())
            .unwrap_or(start)
    }

    /// Launches `event` once: the trust check on the opened cache file, then
    /// the script, killed at its timeout capped by `deadline` or when the
    /// kill flag is set.
    async fn launch(&self, event: HookEvent, deadline: Instant) -> Result<HookOutput, CacheError> {
        let run_id = self.run_id();
        let fail = |reason, detail: String| run_failed(run_id, reason, None, String::new(), detail);
        let trusted = TrustedCacheFile::check(&self.files.cache_path)
            .map_err(|e| fail(TickRunReason::Untrusted, e.to_string()))?;
        let mut set = vec![(
            std::ffi::OsString::from("BD_CACHE_PATH"),
            trusted.path().as_os_str().to_owned(),
        )];
        if let Some(dir) = &self.files.beads_dir {
            set.push(("BEADS_DIR".into(), dir.as_os_str().to_owned()));
        }
        let hook = self
            .payload
            .interpreter()
            .python_hook(self.payload.env().child_env(set));
        let name = crate::payload::schedule_name(SyncRow::Python);
        let payload = self
            .payload
            .run_payload(run_id, trusted.path(), &self.files.repo_root);
        let call = HookCall {
            event,
            schedule: &name,
            payload: &payload,
            timeout: self.payload.timeout(),
            deadline,
        };
        let output = tokio::select! {
            out = hook.invoke(self.clock.as_ref(), &call) => out,
            () = self.kill.wait() => {
                return Err(fail(TickRunReason::Signal,
                    format!("{} was stopped by a signal; its process tree was killed", event.as_str())));
            }
        };
        drop(trusted);
        output.map_err(|e| fail(TickRunReason::Spawn, self.redactor.message(&e.to_string())))
    }

    /// The stop check (`cron.stop`), judged by sc-cron's rules.
    async fn stop_check(&self, deadline: Instant) -> Result<StopDecision, CacheError> {
        let output = self.launch(HookEvent::Stop, deadline).await?;
        output
            .stop_decision()
            .map_err(|outcome| self.classify(&output, &outcome, "cron.stop"))
    }

    /// The run (`cron.tick`) under `lease`.
    async fn tick(&self, lease: &TickLease, deadline: Instant) -> Result<PythonRun, CacheError> {
        debug_assert_eq!(lease.run_id(), self.run_id());
        let output = self.launch(HookEvent::Tick, deadline).await?;
        match output.run_outcome() {
            RunOutcome::Ok => Ok(PythonRun {
                run_id: self.run_id().clone(),
                exit_status: code(&output),
                stderr: self.redactor.stderr(&output.stderr),
            }),
            outcome => Err(self.classify(&output, &outcome, "cron.tick")),
        }
    }

    /// The `TICK_RUN_FAILED` of an output that did not succeed; its stderr
    /// and its message (sc-cron's outcome text holds the start of the raw
    /// stderr) redacted and cut.
    fn classify(&self, output: &HookOutput, outcome: &RunOutcome, event: &str) -> CacheError {
        let reason = if output.stdout_truncated || output.stderr_truncated {
            TickRunReason::Oversize
        } else if output.status == HookStatus::TimedOut {
            TickRunReason::Timeout
        } else {
            TickRunReason::Exit
        };
        run_failed(
            self.run_id(),
            reason,
            code(output),
            self.redactor.stderr(&output.stderr),
            format!("{event}: {}", self.redactor.message(&outcome.to_string())),
        )
    }

    /// `cron.stop` then `cron.tick` under `lease`: the immediate run of
    /// `sync use-python`, which already holds the lease (REQ-BDC-0724).
    ///
    /// # Errors
    ///
    /// `TICK_RUN_FAILED`: `untrusted`, `spawn`, `signal`, `stop_answer` for a
    /// stop answer other than `continue`, `exit` for a non-zero exit,
    /// `timeout` for a kill at the timeout, `oversize` for output over the cap.
    pub(crate) async fn run_under(&self, lease: &TickLease) -> Result<PythonRun, CacheError> {
        let deadline = self.deadline();
        match self.stop_check(deadline).await? {
            StopDecision::Continue => {}
            other => {
                return Err(run_failed(
                    self.run_id(),
                    TickRunReason::StopAnswer,
                    None,
                    String::new(),
                    format!("cron.stop answered {other:?}, not continue"),
                ));
            }
        }
        self.tick(lease, deadline).await
    }

    /// Records a failure where `cron run --once` reads it.
    fn note_failure(&self, e: &CacheError) {
        if let Some(failure) = RowFailure::of(SyncRow::Python, e) {
            self.failures
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(failure);
        }
    }

    /// Records the end of a scheduled run and gives its outcome.
    async fn end(&self, result: Result<PythonRun, CacheError>) -> RunOutcome {
        let end = match &result {
            Ok(_) => SyncRunEnd::Ok,
            Err(e) => {
                self.note_failure(e);
                SyncRunEnd::Failed
            }
        };
        self.record.finish(end).await;
        let outcome = outcome_of(&result);
        tracing::info!(row = PYTHON_ROW, run_id = %self.run_id(), step = "script",
            outcome = %outcome, "sync run step");
        outcome
    }
}

/// sc-cron's outcome of a Python run's result, in one place: ok, a kill at
/// the timeout, or the failure's (redacted) text.
pub(crate) fn outcome_of(result: &Result<PythonRun, CacheError>) -> RunOutcome {
    match result {
        Ok(_) => RunOutcome::Ok,
        Err(CacheError::TickRunFailed {
            reason: TickRunReason::Timeout,
            ..
        }) => RunOutcome::TimedOut,
        Err(e) => RunOutcome::Failed(e.to_string()),
    }
}

fn code(output: &HookOutput) -> Option<i32> {
    match output.status {
        HookStatus::Exited(status) => status.code(),
        HookStatus::TimedOut => None,
    }
}

/// A `TICK_RUN_FAILED` of the Python row.
pub(crate) fn run_failed(
    run_id: &RunId,
    reason: TickRunReason,
    exit_status: Option<i32>,
    stderr: String,
    detail: String,
) -> CacheError {
    CacheError::TickRunFailed {
        detail,
        row: PYTHON_ROW.to_owned(),
        run_id: run_id.clone(),
        reason,
        exit_status,
        stderr,
        restored: None,
    }
}

#[async_trait]
impl<S, L, B, C> Job for PythonSyncJob<S, L, B, C>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
{
    async fn should_stop(&self, _payload: &Value) -> StopDecision {
        self.record.start().await;
        match self.stop_check(self.deadline()).await {
            Ok(StopDecision::Continue) => StopDecision::Continue,
            Ok(decision) => {
                self.record.finish(SyncRunEnd::Skipped).await;
                decision
            }
            Err(e) => {
                *self
                    .undecided
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(e);
                StopDecision::Continue
            }
        }
    }

    async fn run(&self, _payload: &Value, deadline: Instant) -> RunOutcome {
        let undecided = self
            .undecided
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(e) = undecided {
            return self.end(Err(e)).await;
        }
        let cache = Arc::clone(&self.cache);
        let left = deadline.saturating_duration_since(self.clock.monotonic_now());
        let run_id = self.run_id().clone();
        let taken = blocking(move || {
            let Some(mut guard) = hold_within(&cache, left)? else {
                return Ok(None);
            };
            let clock = guard.clock();
            TickLease::take(&cache, &mut guard, run_id, Deadline::after(&*clock, left)).map(Some)
        })
        .await;
        let lease = match taken {
            Ok(Some(LeaseTaken::Taken(lease))) => lease,
            Ok(Some(LeaseTaken::Held(_)) | None) => {
                self.record.finish(SyncRunEnd::Skipped).await;
                return RunOutcome::Refused("in_flight".to_owned());
            }
            Err(e) => return self.end(Err(e)).await,
        };
        tracing::info!(row = PYTHON_ROW, run_id = %self.run_id(), step = "lease", outcome = "taken", "sync run step");
        let result = self.tick(&lease, deadline).await;
        let outcome = self.end(result).await;
        let cache = Arc::clone(&self.cache);
        let released = blocking(move || {
            let mut guard = crate::lease::hold(&cache)?;
            Ok(lease.release(&mut guard))
        })
        .await
        .unwrap_or(false);
        tracing::info!(row = PYTHON_ROW, run_id = %self.run_id(), step = "reply",
            outcome = if released { "lease_released" } else { "lease_left_to_expiry" }, "sync run step");
        outcome
    }
}
