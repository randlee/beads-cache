//! The sync task on `sc-cron` (REQ-BDC-0701, 0703, 0707; ADR-CORE-0001; ADR-SYNC-0002).
//!
//! [`SyncJob`] is the [`Job`] a [`sc_cron::Scheduler`] runs. A `Cache` is
//! `Send` but not `Sync` and its verbs block, so the job holds it in an
//! `Arc<Mutex<_>>` and runs every tick and every stop check in
//! `tokio::task::spawn_blocking`, with the run's deadline as `T`:
//!
//! - a tick that finds the mutex held, including by a blocking task left over
//!   from a timed-out tick (the scheduler drops the future, never the
//!   thread), is skipped and reported busy; `should_stop` then answers
//!   `Continue`;
//! - a poisoned mutex (an earlier tick panicked) is recovered and cleared; the
//!   next tick, whichever of the stop check and the run recovered it, reports
//!   it once in its reply, also when that tick fails.
//!
//! # Shutdown
//!
//! Build the job with a [`StopFlag`] ([`SyncJob::with_stop`]) and give the
//! same flag to the [`AtmSender`]. To stop: set the flag (the running tick ends
//! between phases and before its next message, and `atm` is killed), then
//! `WorkerHandle::shutdown` and await it, then drop the job.
//!
//! # Clocks
//!
//! The scheduler hands a run its deadline as an `Instant`; `Deadline::at`
//! makes it `T`. Under `sc-cron`'s `SystemClock` and the cache's `SystemClock`
//! both are `Instant::now`. A test that drives `sc-cron` with its own manual
//! clock must start both clocks from one origin (`tests/common/real.rs`).
//!
//! [`AtmSender`]: crate::AtmSender

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainLock, EscalationSender, JournalEntry, TickConfig,
};
use sc_beads::{CacheBackend, Deadline};
use sc_cron::{Job, RunOutcome, ScheduleSpec, StopDecision};

use crate::atm::AtmSender;
use crate::control::StopFlag;
use crate::phases::{ConfigRefusal, TickSettings};
use crate::reply::{Refusal, RefusalCode, TickOutcome, TickReply};
use crate::text::clip;
use crate::tick::{TICK_BUDGET, stop_reason_with, tick_with};

/// The cron expression of the schedule: every 30 s (BD-CACHE.md, Cron module).
///
/// The tick interval is the retry interval of everything the drain and the
/// sync leave for later (ADR-BDC-0005).
pub const TICK_EXPR: &str = "*/30 * * * * *";

/// How long the stop check may spend (its one `bd list --id` of uncached roots
/// is further bounded by the miss-fill timeout).
///
/// The check runs before every run and shares the schedule's budget with it;
/// 3 s keeps it a small part of the 25 s a run needs.
pub const STOP_CHECK_BUDGET: Duration = Duration::from_secs(3);

/// The most bytes of a refusal list kept in the scheduler's recorded outcome
/// (it is stored on every run).
const MAX_OUTCOME_BYTES: usize = 1024;

/// What one run of the job did.
#[derive(Debug)]
#[non_exhaustive]
pub enum TickRun {
    /// The tick ran, or was skipped: the reply says which (`busy`) and how it went.
    Done(Box<TickReply>),
    /// The tick failed with a store or cache error.
    Failed {
        /// The error.
        error: CacheError,
        /// The cache mutex had been poisoned and was recovered for this tick.
        poisoned: bool,
    },
}

/// What the job needs from its owner, shared with the blocking tasks.
struct Shared<S: CacheStore, L: DrainLock, B: CacheBackend> {
    cache: Mutex<Cache<S, L, B>>,
    settings: TickSettings,
    sender: Arc<dyn EscalationSender>,
    stop: StopFlag,
    /// A recovered poison nobody has reported yet.
    poison_unreported: AtomicBool,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> Shared<S, L, B> {
    /// Recovers a poisoned mutex: the cache keeps its state in the store's
    /// transactions, so it is usable; the poison is cleared and remembered
    /// until a tick reports it.
    fn recover<'a>(
        &self,
        e: std::sync::PoisonError<MutexGuard<'a, Cache<S, L, B>>>,
    ) -> MutexGuard<'a, Cache<S, L, B>> {
        self.cache.clear_poison();
        self.poison_unreported.store(true, Ordering::SeqCst);
        e.into_inner()
    }

    /// Takes the cache without waiting; `None` when it is in use.
    fn try_hold(&self) -> Option<MutexGuard<'_, Cache<S, L, B>>> {
        match self.cache.try_lock() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(e)) => Some(self.recover(e)),
            Err(TryLockError::WouldBlock) => None,
        }
    }

    /// Takes the cache, waiting for whoever holds it.
    fn hold(&self) -> MutexGuard<'_, Cache<S, L, B>> {
        self.cache.lock().unwrap_or_else(|e| self.recover(e))
    }

    fn run_tick(&self, end: Deadline) -> TickRun {
        let Some(mut guard) = self.try_hold() else {
            return TickRun::Done(Box::new(TickReply::busy()));
        };
        let poisoned = self.poison_unreported.swap(false, Ordering::SeqCst);
        match tick_with(&mut guard, &self.settings, &*self.sender, end, &self.stop) {
            Ok(mut reply) => {
                if poisoned {
                    reply.poisoned = true;
                    reply.refused.push(Refusal::new(
                        RefusalCode::MutexPoisoned,
                        "the cache mutex was poisoned by an earlier panic and was recovered",
                    ));
                    reply.outcome = TickOutcome::Refused;
                }
                TickRun::Done(Box::new(reply))
            }
            Err(error) => TickRun::Failed { error, poisoned },
        }
    }

    fn run_stop(&self) -> StopDecision {
        let Some(mut guard) = self.try_hold() else {
            // Busy: a blocking task is still working; never stop under it.
            return StopDecision::Continue;
        };
        let end = Deadline::after(&*guard.clock(), STOP_CHECK_BUDGET);
        match stop_reason_with(&mut guard, &self.settings, end) {
            Ok(Some(reason)) => StopDecision::Stop(reason),
            Ok(None) => StopDecision::Continue,
            Err(e) => {
                tracing::warn!(
                    code = e.code(),
                    detail = e.detail(),
                    "stop check failed; the schedule keeps going"
                );
                StopDecision::Continue
            }
        }
    }
}

/// The job `sc-cron` runs for the repository's one schedule.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use bd_cache_sync::{AtmSender, StopFlag, SyncJob, compose, schedule_spec};
/// use sc_beads::{Clock, SystemClock};
/// use sc_cron::{Scheduler, SqliteScheduleStore, SystemClock as CronClock, SystemJitter};
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let clock: Arc<dyn Clock> = Arc::new(SystemClock);
/// let root = std::path::Path::new(".");
/// let composed = compose(root, "bd", clock)?;
/// let stop = StopFlag::new();
/// let atm = AtmSender::default().with_stop(stop.clone());
/// let spec = schedule_spec(&composed.tick, &composed.name, Some(&atm))?;
/// let job = SyncJob::with_stop(composed.cache, &composed.tick, Arc::new(atm), stop.clone())?;
/// let scheduler = Scheduler::new(
///     Arc::new(SqliteScheduleStore::open(".bd-cache/cron.db")?),
///     Arc::new(CronClock),
///     Arc::new(SystemJitter),
/// );
/// scheduler.start_async(spec).await?;
/// let worker = scheduler.spawn_worker(Arc::new(job));
/// // ... later: set the flag, then wait for the worker
/// stop.stop();
/// worker.shutdown(tokio::time::Instant::now() + std::time::Duration::from_secs(30)).await;
/// # Ok(())
/// # }
/// ```
pub struct SyncJob<S: CacheStore, L: DrainLock, B: CacheBackend> {
    shared: Arc<Shared<S, L, B>>,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> std::fmt::Debug for SyncJob<S, L, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncJob")
            .field("settings", &self.shared.settings)
            .finish_non_exhaustive()
    }
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> Clone for SyncJob<S, L, B> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> SyncJob<S, L, B> {
    /// A job over `cache` that announces through `sender`.
    ///
    /// Only one ticker may run per cache file: do not build a second job, or
    /// call `drain` and `sync` from another scheduler, on the same file.
    ///
    /// # Errors
    ///
    /// [`ConfigRefusal`] when the `[tick]` table would be refused by every
    /// tick (no team, no fallback lead, a bad name, a bad phase list), so a
    /// job that could never run is not built.
    pub fn new(
        cache: Cache<S, L, B>,
        config: &TickConfig,
        sender: Arc<dyn EscalationSender>,
    ) -> Result<Self, ConfigRefusal> {
        Self::with_stop(cache, config, sender, StopFlag::new())
    }

    /// As [`new`](Self::new), with the stop flag the host will set to shut down.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn with_stop(
        cache: Cache<S, L, B>,
        config: &TickConfig,
        sender: Arc<dyn EscalationSender>,
        stop: StopFlag,
    ) -> Result<Self, ConfigRefusal> {
        let settings = TickSettings::from_config(config)?;
        Ok(Self {
            shared: Arc::new(Shared {
                cache: Mutex::new(cache),
                settings,
                sender,
                stop,
                poison_unreported: AtomicBool::new(false),
            }),
        })
    }

    /// The stop flag (a clone): set it to end the running tick early.
    #[must_use]
    pub fn stop_flag(&self) -> StopFlag {
        self.shared.stop.clone()
    }

    /// Runs one tick on the calling thread, which must be allowed to block.
    ///
    /// This is what the scheduler's blocking task calls; it is public for
    /// hosts that drive the tick from their own loop and for tests. It does
    /// not wait for the cache: a held cache gives a busy reply.
    #[must_use]
    pub fn run_blocking(&self, end: Deadline) -> TickRun {
        self.shared.run_tick(end)
    }

    /// Runs the stop check on the calling thread; `Continue` when the cache is busy.
    #[must_use]
    pub fn should_stop_blocking(&self) -> StopDecision {
        self.shared.run_stop()
    }

    /// Runs `f` on the cache, waiting for any tick in flight to finish.
    ///
    /// For the reads and writes of the host process that share this cache
    /// (the same `Cache` the ticks use). A poisoned mutex is recovered.
    ///
    /// # Blocking
    ///
    /// This waits for the mutex, for as long as a tick runs (up to its 25 s
    /// budget plus a leftover blocking task). Do not call it from an async
    /// task: use `tokio::task::spawn_blocking`, or [`try_with_cache`](Self::try_with_cache).
    pub fn with_cache<T>(&self, f: impl FnOnce(&mut Cache<S, L, B>) -> T) -> T {
        f(&mut self.shared.hold())
    }

    /// Runs `f` on the cache if no tick holds it; `None` when it is busy.
    ///
    /// Never waits, so it is safe in async code.
    pub fn try_with_cache<T>(&self, f: impl FnOnce(&mut Cache<S, L, B>) -> T) -> Option<T> {
        self.shared.try_hold().map(|mut guard| f(&mut guard))
    }

    /// The escalated entries not yet announced, for a host that wants to show them.
    ///
    /// # Blocking
    ///
    /// As [`with_cache`](Self::with_cache).
    ///
    /// # Errors
    ///
    /// Store errors.
    pub fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.with_cache(|c| c.unannounced_escalations())
    }
}

/// Maps a finished run to the scheduler's outcome.
fn outcome_of(run: TickRun) -> RunOutcome {
    match run {
        TickRun::Done(reply) => match reply.outcome {
            TickOutcome::PassThrough => RunOutcome::Ok,
            TickOutcome::Refused => {
                let list: Vec<String> = reply.refused.iter().map(ToString::to_string).collect();
                RunOutcome::Refused(clip(&list.join("; "), MAX_OUTCOME_BYTES))
            }
        },
        TickRun::Failed { error, poisoned } => {
            let note = if poisoned {
                " (the cache mutex had been poisoned and was recovered)"
            } else {
                ""
            };
            RunOutcome::Failed(clip(
                &format!("{}: {}{note}", error.code(), error.detail()),
                MAX_OUTCOME_BYTES,
            ))
        }
    }
}

#[async_trait]
impl<S, L, B> Job for SyncJob<S, L, B>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    async fn run(&self, _payload: &serde_json::Value, deadline: Instant) -> RunOutcome {
        let shared = Arc::clone(&self.shared);
        let end = Deadline::at(deadline);
        match tokio::task::spawn_blocking(move || shared.run_tick(end)).await {
            Ok(run) => outcome_of(run),
            Err(e) => RunOutcome::Failed(format!(
                "tick_task_failed: the tick task did not finish ({e}); look for a panic in the log"
            )),
        }
    }

    async fn should_stop(&self, _payload: &serde_json::Value) -> StopDecision {
        let shared = Arc::clone(&self.shared);
        match tokio::task::spawn_blocking(move || shared.run_stop()).await {
            Ok(decision) => decision,
            Err(e) => {
                tracing::warn!(error = %e, "the stop check did not finish; the schedule keeps going");
                StopDecision::Continue
            }
        }
    }
}

/// The schedule of a repository could not be built.
#[derive(Debug)]
#[non_exhaustive]
pub enum ScheduleError {
    /// The `[tick]` table is refused (REQ-BDC-0701).
    Config(ConfigRefusal),
    /// The phases directory does not exist.
    PhasesDir(std::path::PathBuf),
    /// `atm` cannot be found.
    Sender(bd_cache_model::SendError),
    /// `sc-cron` refused the specification.
    Cron(sc_cron::CronError),
}

impl ScheduleError {
    /// A stable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config(c) => c.code().as_str(),
            Self::PhasesDir(_) => "phases_dir_missing",
            Self::Sender(_) => "atm_not_found",
            Self::Cron(_) => "cron_refused",
        }
    }
}

impl ScheduleError {
    /// What to do about it.
    #[must_use]
    pub fn fix(&self) -> String {
        match self {
            Self::Config(c) => c.code().fix().to_owned(),
            Self::PhasesDir(_) => RefusalCode::PhasesDirUnreadable.fix().to_owned(),
            Self::Sender(e) => e.fix().to_owned(),
            Self::Cron(_) => {
                "use a repository name sc-cron accepts (1 to 128 characters, no control characters)"
                    .to_owned()
            }
        }
    }
}

/// The message says what failed; the cause is the `source`, so a reporter that
/// walks the chain prints it once.
impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(_) => f.write_str("the sync schedule is refused"),
            Self::PhasesDir(d) => write!(f, "the phases directory {} does not exist", d.display()),
            Self::Sender(_) => f.write_str("the sync schedule cannot send messages"),
            Self::Cron(_) => f.write_str("sc-cron refused the sync schedule"),
        }
    }
}

impl std::error::Error for ScheduleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(c) => Some(c),
            Self::PhasesDir(_) => None,
            Self::Sender(e) => Some(e),
            Self::Cron(c) => Some(c),
        }
    }
}

impl From<ConfigRefusal> for ScheduleError {
    fn from(c: ConfigRefusal) -> Self {
        Self::Config(c)
    }
}

/// The one schedule of a repository (REQ-BDC-0701): every 30 s with a 25 s
/// budget, named `bd-cache:<repo>`.
///
/// With `atm` given, it also checks that the phases directory exists and that
/// the `atm` program is found, so a schedule that could never read a phase
/// or send a message is not started.
///
/// # Errors
///
/// [`ScheduleError::Config`] when the `[tick]` table has no team or no
/// fallback lead (or any other refusal the tick would give);
/// [`ScheduleError::PhasesDir`] and [`ScheduleError::Sender`] from the checks
/// above; [`ScheduleError::Cron`] for a name `sc-cron` does not accept.
///
/// # Examples
///
/// ```
/// use bd_cache_model::TickConfig;
/// use bd_cache_sync::schedule_spec;
///
/// let cfg = TickConfig {
///     team: Some("atm-dev".into()),
///     fallback_lead: Some("team-lead".into()),
///     ..TickConfig::default()
/// };
/// let spec = schedule_spec(&cfg, "atm-dev", None).unwrap();
/// assert_eq!(spec.name.as_str(), "bd-cache:atm-dev");
/// // No team, no schedule (REQ-BDC-0701).
/// assert!(schedule_spec(&TickConfig::default(), "atm-dev", None).is_err());
/// ```
pub fn schedule_spec(
    config: &TickConfig,
    repo: &str,
    atm: Option<&AtmSender>,
) -> Result<ScheduleSpec, ScheduleError> {
    let settings = TickSettings::from_config(config)?;
    if let Some(atm) = atm {
        if !settings.phases_dir().is_dir() {
            return Err(ScheduleError::PhasesDir(
                settings.phases_dir().to_path_buf(),
            ));
        }
        atm.resolve().map_err(ScheduleError::Sender)?;
    }
    ScheduleSpec::new(
        &format!("bd-cache:{repo}"),
        TICK_EXPR,
        TICK_BUDGET,
        serde_json::json!({ "team": settings.team().as_str(), "fallback_lead": settings.fallback_lead().as_str() }),
    )
    .map_err(ScheduleError::Cron)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The poison report survives a tick that fails (QA-007), and the recorded
    /// outcome is bounded (RSH-008).
    #[test]
    fn a_failed_run_carries_the_poison_note_and_outcomes_are_clipped() {
        let failed = outcome_of(TickRun::Failed {
            error: CacheError::NotFound {
                detail: "gone".into(),
            },
            poisoned: true,
        });
        assert!(
            matches!(&failed, RunOutcome::Failed(m) if m.contains("poisoned") && m.contains("NOT_FOUND")),
            "{failed:?}"
        );
        let long = TickReply::refused_with(Refusal::new(
            RefusalCode::PhaseFileInvalid,
            "x".repeat(10_000),
        ));
        match outcome_of(TickRun::Done(Box::new(long))) {
            RunOutcome::Refused(m) => assert!(m.len() < MAX_OUTCOME_BYTES + 32, "{}", m.len()),
            other => panic!("{other:?}"),
        }
    }
}
