//! The Python sync row's job (REQ-BDC-0725, REQ-BDC-0722, ADR-SYNC-0008).
//!
//! [`PythonSyncJob`] runs the consumer's module with sc-cron's Python hook
//! protocol (`sc-cron.hook.v1`) through [`sc_cron::PythonHook`], configured
//! with the environment allowlist of REQ-BDC-0723: `cron.stop` (its
//! `should_stop`), then, on `continue`, `cron.tick` (its `run`, under the
//! lease), each from a fresh temp cwd with one JSON object on stdin whose
//! `payload` is the row's payload plus the run's `run_id`. The trust check
//! runs before each launch. The stop check runs before the lease is taken,
//! so two workers may each launch `cron.stop` (a stop answer has no side
//! effects); only the lease holder launches `cron.tick`. The run is judged
//! by the script's exit status.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use async_trait::async_trait;
use bd_cache_model::{CacheError, CacheStore, DrainLock, RunId, TickRunReason};
use sc_beads::{CacheBackend, Deadline};
use sc_cron::{
    Clock as CronClock, HookCall, HookEvent, HookOutput, HookStatus, Job, RunOutcome, RunTimeout,
    ScheduleName, StopDecision,
};
use serde_json::Value;

use crate::control::StopFlag;
use crate::history::RowHistory;
use crate::lease::{LeaseTaken, TickLease, log_release, new_run_id};
use crate::payload::{PYTHON_ROW, PythonPayload};
use crate::run::{SharedCache, blocking, hold};
use crate::text::redact_and_cut;
use crate::trust::TrustedCacheFile;

/// What one launch of the script gave.
#[derive(Debug)]
pub struct PythonRun {
    /// The run.
    pub run_id: RunId,
    /// Its exit status, when it exited.
    pub exit_status: Option<i32>,
    /// Its stderr, redacted and cut to 8 KiB (REQ-BDC-0724).
    pub stderr: String,
}

/// The Python row's job, built per launch by the resolver.
pub struct PythonSyncJob<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> {
    cache: SharedCache<S, L, B>,
    payload: PythonPayload,
    clock: Arc<C>,
    stop: StopFlag,
    history: RowHistory,
    /// A stop check that did not decide: its outcome, which `run` records.
    undecided: Mutex<Option<RunOutcome>>,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend, C: CronClock> std::fmt::Debug
    for PythonSyncJob<S, L, B, C>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PythonSyncJob")
            .field("payload", &self.payload)
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
    /// The job of one launch of the row.
    #[must_use]
    pub fn new(
        cache: SharedCache<S, L, B>,
        payload: PythonPayload,
        clock: Arc<C>,
        stop: StopFlag,
        history: RowHistory,
    ) -> Self {
        Self {
            cache,
            payload,
            clock,
            stop,
            history,
            undecided: Mutex::new(None),
        }
    }

    fn timeout(&self) -> RunTimeout {
        RunTimeout::new(self.payload.timeout).unwrap_or_else(|_| fallback_timeout())
    }

    /// Launches `event` once: the trust check, then the script, killed at
    /// its timeout capped by `deadline` or when the stop flag is set.
    ///
    /// # Errors
    ///
    /// `TICK_RUN_FAILED`: `untrusted`, `spawn`, `timeout` or `signal`.
    pub async fn launch(
        &self,
        event: HookEvent,
        run_id: &RunId,
        deadline: Instant,
    ) -> Result<HookOutput, CacheError> {
        let fail = |reason, detail: String| run_failed(run_id, reason, None, String::new(), detail);
        let trusted = TrustedCacheFile::check(&self.payload.cache_path)
            .map_err(|e| fail(TickRunReason::Untrusted, e.to_string()))?;
        let mut set = vec![(
            std::ffi::OsString::from("BD_CACHE_PATH"),
            trusted.path().as_os_str().to_owned(),
        )];
        if let Some(dir) = &self.payload.beads_dir {
            set.push(("BEADS_DIR".into(), dir.as_os_str().to_owned()));
        }
        let hook = self
            .payload
            .interpreter
            .python_hook(self.payload.env.child_env(set));
        let name =
            ScheduleName::new(PYTHON_ROW).map_err(|e| fail(TickRunReason::Spawn, e.to_string()))?;
        let payload = self.payload.run_payload(run_id.as_str());
        let call = HookCall {
            event,
            schedule: &name,
            payload: &payload,
            timeout: self.timeout(),
            deadline,
        };
        let output = tokio::select! {
            out = hook.invoke(self.clock.as_ref(), &call) => out,
            () = self.stop.wait() => {
                return Err(fail(TickRunReason::Signal,
                    format!("{} was stopped by a signal; its process tree was killed", event.as_str())));
            }
        };
        drop(trusted);
        output.map_err(|e| fail(TickRunReason::Spawn, e.to_string()))
    }

    /// `cron.stop` then `cron.tick` under `lease` (the immediate run of
    /// `sync use-python`, which already holds the lease, REQ-BDC-0724).
    ///
    /// # Errors
    ///
    /// `TICK_RUN_FAILED`: as [`launch`](Self::launch), `stop_answer` for a
    /// stop answer other than `continue`, `exit` for a non-zero exit,
    /// `timeout` for a kill at the timeout, `oversize` for output over the cap.
    pub async fn run_under(&self, lease: &TickLease) -> Result<PythonRun, CacheError> {
        let run_id = lease.run_id();
        let start = self.clock.monotonic_now();
        let deadline = start.checked_add(self.payload.timeout).unwrap_or(start);
        let stop = self.launch(HookEvent::Stop, run_id, deadline).await?;
        let names = self.payload.env.names();
        match stop.stop_decision() {
            Ok(StopDecision::Continue) => {}
            Ok(other) => {
                return Err(run_failed(
                    run_id,
                    TickRunReason::StopAnswer,
                    code(&stop),
                    redact_and_cut(&stop.stderr, names),
                    format!("cron.stop answered {other:?}, not continue"),
                ));
            }
            Err(outcome) => {
                return Err(output_failed(run_id, &stop, &outcome, names, "cron.stop"));
            }
        }
        let tick = self.launch(HookEvent::Tick, run_id, deadline).await?;
        match tick.run_outcome() {
            RunOutcome::Ok => Ok(PythonRun {
                run_id: run_id.clone(),
                exit_status: code(&tick),
                stderr: redact_and_cut(&tick.stderr, names),
            }),
            outcome => Err(output_failed(run_id, &tick, &outcome, names, "cron.tick")),
        }
    }

    fn failed(&self, run_id: &RunId, message: &str) -> RunOutcome {
        let now = sc_cron::Timestamp::from_datetime(self.clock.now()).to_string();
        self.history.failed(run_id, &now, message)
    }
}

#[expect(clippy::expect_used, reason = "1 s is inside RunTimeout's range")]
fn fallback_timeout() -> RunTimeout {
    RunTimeout::new(std::time::Duration::from_secs(1)).expect("in range")
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

/// The `TICK_RUN_FAILED` of an output that did not succeed.
fn output_failed(
    run_id: &RunId,
    output: &HookOutput,
    outcome: &RunOutcome,
    names: &[String],
    event: &str,
) -> CacheError {
    let reason = if output.stdout_truncated || output.stderr_truncated {
        TickRunReason::Oversize
    } else if output.status == HookStatus::TimedOut {
        TickRunReason::Timeout
    } else {
        TickRunReason::Exit
    };
    run_failed(
        run_id,
        reason,
        code(output),
        redact_and_cut(&output.stderr, names),
        format!("{event}: {outcome}"),
    )
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
        let start = self.clock.monotonic_now();
        let deadline = start.checked_add(self.payload.timeout).unwrap_or(start);
        let run_id = new_run_id(&sc_beads::SystemClock);
        let decided = match self.launch(HookEvent::Stop, &run_id, deadline).await {
            Ok(output) => output.stop_decision(),
            Err(e) => Err(self.failed(&run_id, &e.to_string())),
        };
        match decided {
            Ok(decision) => decision,
            Err(outcome) => {
                *self
                    .undecided
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(outcome);
                StopDecision::Continue
            }
        }
    }

    async fn run(&self, _payload: &Value, deadline: Instant) -> RunOutcome {
        if let Some(outcome) = self
            .undecided
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            return outcome;
        }
        let cache = Arc::clone(&self.cache);
        let left = deadline.saturating_duration_since(self.clock.monotonic_now());
        let taken = blocking(move || {
            let mut guard = hold(&cache)?;
            let clock = guard.clock();
            let run_id = new_run_id(&*clock);
            TickLease::take(&mut guard, run_id, Deadline::after(&*clock, left))
        })
        .await;
        let lease = match taken {
            Ok(LeaseTaken::Taken(lease)) => lease,
            Ok(LeaseTaken::Held(_)) => return RunOutcome::Refused("in_flight".to_owned()),
            Err(e) => return RunOutcome::Failed(format!("lease: {e}")),
        };
        let fallback = Arc::clone(&self.cache);
        let lease = lease.with_fallback(move |run_id| {
            let mut guard = fallback.lock().unwrap_or_else(PoisonError::into_inner);
            log_release(&run_id, guard.lease_release(&run_id));
        });
        let run_id = lease.run_id().clone();
        tracing::info!(row = PYTHON_ROW, run_id = %run_id, step = "lease", outcome = "taken", "sync run step");
        let outcome = match self.launch(HookEvent::Tick, &run_id, deadline).await {
            Ok(output) => match output.run_outcome() {
                RunOutcome::Ok => RunOutcome::Ok,
                other => self.failed(&run_id, &other.to_string()),
            },
            Err(e) => self.failed(&run_id, &e.to_string()),
        };
        tracing::info!(row = PYTHON_ROW, run_id = %run_id, step = "script", outcome = %outcome, "sync run step");
        let cache = Arc::clone(&self.cache);
        let released = blocking(move || {
            let mut guard = hold(&cache)?;
            Ok(lease.release(&mut guard))
        })
        .await
        .unwrap_or(false);
        tracing::info!(row = PYTHON_ROW, run_id = %run_id, step = "reply",
            outcome = if released { "lease_released" } else { "lease_left_to_expiry" }, "sync run step");
        outcome
    }
}
