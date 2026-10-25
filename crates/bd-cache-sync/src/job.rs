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
//! - a poisoned mutex (an earlier tick panicked) is never trusted: the cache
//!   the panic left behind is dropped, the poison cleared, and the next tick
//!   builds a new cache from the same composition inputs (the recompose
//!   function of [`SyncJob::with_recompose`]) and reports it once in its
//!   reply, also when that tick fails (S55, PE-RBP-F007). A job built
//!   without one reports the cache lost ([`TickRun::Lost`]).
//!
//! # Shutdown
//!
//! Build the job with a [`StopFlag`] ([`SyncJob::with_stop`]) and give the
//! same flag to the [`AtmSender`]. To stop, in this order:
//!
//! 1. set the flag: the running tick ends between phases and before its next
//!    message, and `atm` is killed;
//! 2. `WorkerHandle::shutdown` and await it: no new run starts;
//! 3. [`SyncJob::join_idle`] with a deadline, from a thread that may block
//!    (`spawn_blocking`): the scheduler drops a run's future but never its
//!    blocking thread, so a tick may still be inside the cache; this waits
//!    for it, up to the deadline, and says whether it ended (PE-RSH-004);
//! 4. drop the job.
//!
//! # Clocks
//!
//! The scheduler hands a run its deadline as an `Instant`; `Deadline::at`
//! makes it `T`. Under `sc-cron`'s `SystemClock` and the cache's `SystemClock`
//! both are `Instant::now`. A test that drives `sc-cron` with its own manual
//! clock must start both clocks from one origin (`tests/common/real.rs`).
//!
//! [`AtmSender`]: crate::AtmSender

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainLock, EscalationSender, JournalEntry, TickConfig,
};
use sc_beads::{CacheBackend, Deadline};
use sc_cron::{Job, RunOutcome, ScheduleSpec, StopDecision};

use crate::atm::AtmSender;
use crate::compose::ComposeError;
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

/// How often [`SyncJob::join_idle`] looks at the cache mutex again.
const IDLE_POLL: Duration = Duration::from_millis(5);

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
        /// An earlier panic poisoned the cache mutex; the cache it left was
        /// dropped and this tick ran on a recomposed one.
        poisoned: bool,
    },
    /// There is no cache to tick: an earlier panic dropped it and it could not
    /// be recomposed (S55).
    Lost {
        /// Why there is no cache.
        error: CacheLost,
        /// The panic is reported by this run (once, as for the other variants).
        poisoned: bool,
    },
}

/// A panic dropped the job's cache and no new one could be built (S55).
#[derive(Debug)]
pub struct CacheLost {
    /// The composition's error; `None` when the job has no recompose function.
    cause: Option<ComposeError>,
}

/// The stable code of [`CacheLost`].
const CACHE_LOST_CODE: &str = "sync_cache_lost";

impl CacheLost {
    /// The stable code, `sync_cache_lost`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        CACHE_LOST_CODE
    }

    /// What to do about it.
    #[must_use]
    pub fn fix(&self) -> &'static str {
        match &self.cause {
            Some(e) => e.fix(),
            None => {
                "build the job with SyncJob::with_recompose, or restart the host; look for the panic in the log"
            }
        }
    }

    /// The composition's error, when recomposing was tried and failed.
    #[must_use]
    pub fn cause(&self) -> Option<&ComposeError> {
        self.cause.as_ref()
    }
}

impl fmt::Display for CacheLost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Some(_) => f.write_str("a panic dropped the cache and it could not be recomposed"),
            None => f.write_str("a panic dropped the cache and the job has no recompose function"),
        }
    }
}

impl std::error::Error for CacheLost {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause
            .as_ref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

/// A host read through the job failed: the cache is lost, or the read failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum JobCacheError {
    /// There is no cache ([`CacheLost`]).
    Lost(CacheLost),
    /// The cache answered with an error.
    Cache(CacheError),
}

impl JobCacheError {
    /// A stable code: [`CacheLost::code`] or the cache error's `BD_CACHE.*` code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Lost(e) => e.code(),
            Self::Cache(e) => e.code(),
        }
    }
}

impl fmt::Display for JobCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lost(e) => fmt::Display::fmt(e, f),
            Self::Cache(_) => f.write_str("the cache read failed"),
        }
    }
}

impl std::error::Error for JobCacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lost(e) => std::error::Error::source(e),
            Self::Cache(e) => Some(e),
        }
    }
}

/// Builds a new cache from the inputs of the first composition (S55).
type Recompose<S, L, B> =
    Box<dyn Fn() -> Result<Cache<S, L, B>, ComposeError> + Send + Sync + 'static>;

/// The job's cache: `None` once a panic dropped it, until it is recomposed.
type Slot<S, L, B> = Option<Cache<S, L, B>>;

/// What the job needs from its owner, shared with the blocking tasks.
struct Shared<S: CacheStore, L: DrainLock, B: CacheBackend> {
    cache: Mutex<Slot<S, L, B>>,
    recompose: Option<Recompose<S, L, B>>,
    settings: TickSettings,
    sender: Arc<dyn EscalationSender>,
    stop: StopFlag,
    /// A panic that dropped the cache and nobody has reported yet.
    poison_unreported: AtomicBool,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> Shared<S, L, B> {
    /// Recovers a poisoned mutex without trusting what it holds: a panic may
    /// have left the cache between two steps of a verb (S55), so it is
    /// dropped. The poison is cleared and remembered until a tick reports it;
    /// the next [`cache_in`](Self::cache_in) recomposes.
    fn recover<'a>(
        &self,
        e: PoisonError<MutexGuard<'a, Slot<S, L, B>>>,
    ) -> MutexGuard<'a, Slot<S, L, B>> {
        let mut guard = e.into_inner();
        let panicked = guard.take();
        self.cache.clear_poison();
        self.poison_unreported.store(true, Ordering::SeqCst);
        drop(panicked);
        guard
    }

    /// Takes the cache mutex without waiting; `None` when it is in use.
    fn try_hold(&self) -> Option<MutexGuard<'_, Slot<S, L, B>>> {
        match self.cache.try_lock() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(e)) => Some(self.recover(e)),
            Err(TryLockError::WouldBlock) => None,
        }
    }

    /// Takes the cache mutex, waiting for whoever holds it.
    fn hold(&self) -> MutexGuard<'_, Slot<S, L, B>> {
        self.cache.lock().unwrap_or_else(|e| self.recover(e))
    }

    /// The cache in `slot`, recomposed first when a panic dropped it.
    fn cache_in<'g>(
        &self,
        slot: &'g mut Slot<S, L, B>,
    ) -> Result<&'g mut Cache<S, L, B>, CacheLost> {
        if slot.is_none() {
            let Some(recompose) = &self.recompose else {
                return Err(CacheLost { cause: None });
            };
            let cache = recompose().map_err(|e| CacheLost { cause: Some(e) })?;
            tracing::info!("the cache dropped after a panic was recomposed");
            *slot = Some(cache);
        }
        slot.as_mut().ok_or(CacheLost { cause: None })
    }

    fn run_tick(&self, end: Deadline) -> TickRun {
        let Some(mut guard) = self.try_hold() else {
            return TickRun::Done(Box::new(TickReply::busy()));
        };
        let poisoned = self.poison_unreported.swap(false, Ordering::SeqCst);
        let cache = match self.cache_in(&mut guard) {
            Ok(cache) => cache,
            Err(error) => return TickRun::Lost { error, poisoned },
        };
        match tick_with(cache, &self.settings, &*self.sender, end, &self.stop) {
            Ok(mut reply) => {
                if poisoned {
                    reply.poisoned = true;
                    reply.refused.push(Refusal::new(
                        RefusalCode::MutexPoisoned,
                        "an earlier panic poisoned the cache mutex; that cache was dropped and this tick ran on a recomposed one",
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
        // A cache a panic dropped is rebuilt by the next tick, not here.
        let Some(cache) = guard.as_mut() else {
            return StopDecision::Continue;
        };
        let end = Deadline::after(&*cache.clock(), STOP_CHECK_BUDGET);
        match stop_reason_with(cache, &self.settings, end) {
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

    /// Waits until no one holds the cache, or `deadline` passes.
    fn join_idle(&self, deadline: Instant) -> bool {
        loop {
            if self.try_hold().is_some() {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            std::thread::sleep(IDLE_POLL.min(deadline - now));
        }
    }
}

/// The job `sc-cron` runs for the repository's one schedule.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use std::time::{Duration, Instant};
/// use bd_cache_sync::{AtmSender, StopFlag, SyncJob, compose, schedule_spec};
/// use sc_beads::{Clock, SystemClock};
/// use sc_cron::{Scheduler, SqliteScheduleStore, SystemClock as CronClock, SystemJitter};
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let clock: Arc<dyn Clock> = Arc::new(SystemClock);
/// let root = std::path::PathBuf::from(".");
/// let composed = compose(&root, "bd", Arc::clone(&clock))?;
/// let stop = StopFlag::new();
/// let atm = AtmSender::default().with_stop(stop.clone());
/// let spec = schedule_spec(&composed.tick, &composed.name, Some(&atm))?;
/// // After a panic the job drops its cache and composes a new one from the
/// // same inputs (`compose` shares one lock set across the process).
/// let job = SyncJob::with_recompose(
///     composed.cache,
///     &composed.tick,
///     Arc::new(atm),
///     stop.clone(),
///     move || compose(&root, "bd", Arc::clone(&clock)).map(|c| c.cache),
/// )?;
/// let scheduler = Scheduler::new(
///     Arc::new(SqliteScheduleStore::open(".bd-cache/cron.db")?),
///     Arc::new(CronClock),
///     Arc::new(SystemJitter),
/// );
/// scheduler.start_async(spec).await?;
/// let worker = scheduler.spawn_worker(Arc::new(job.clone()));
/// // ... later: set the flag, wait for the worker, then for the tick in flight
/// stop.stop();
/// worker.shutdown(tokio::time::Instant::now() + Duration::from_secs(30)).await;
/// let idle = tokio::task::spawn_blocking(move || {
///     job.join_idle(Instant::now() + Duration::from_secs(30))
/// })
/// .await?;
/// if !idle {
///     eprintln!("a tick was still running at the shutdown deadline");
/// }
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
            .field("recomposes", &self.shared.recompose.is_some())
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
    /// The job cannot rebuild `cache`: after a panic in a tick it reports the
    /// cache lost ([`TickRun::Lost`]). A host uses
    /// [`with_recompose`](Self::with_recompose).
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
        Self::build(cache, None, config, sender, stop)
    }

    /// As [`with_stop`](Self::with_stop), with the function that builds a new
    /// cache from the same composition inputs as `cache` (S55, PE-RBP-F007).
    ///
    /// A panic in a tick or a host call may leave the cache between two steps,
    /// so the job never reuses it: it drops it, and the next tick (or host
    /// call) calls `recompose` and runs on its cache. A failing `recompose` is
    /// [`TickRun::Lost`] and is tried again by the next tick. For the real
    /// cache, `recompose` calls [`compose`](crate::compose) (or
    /// [`compose_with`](crate::compose_with) with the same lock set) with the
    /// arguments that built `cache`.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn with_recompose(
        cache: Cache<S, L, B>,
        config: &TickConfig,
        sender: Arc<dyn EscalationSender>,
        stop: StopFlag,
        recompose: impl Fn() -> Result<Cache<S, L, B>, ComposeError> + Send + Sync + 'static,
    ) -> Result<Self, ConfigRefusal> {
        Self::build(cache, Some(Box::new(recompose)), config, sender, stop)
    }

    fn build(
        cache: Cache<S, L, B>,
        recompose: Option<Recompose<S, L, B>>,
        config: &TickConfig,
        sender: Arc<dyn EscalationSender>,
        stop: StopFlag,
    ) -> Result<Self, ConfigRefusal> {
        let settings = TickSettings::from_config(config)?;
        Ok(Self {
            shared: Arc::new(Shared {
                cache: Mutex::new(Some(cache)),
                recompose,
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

    /// Waits until no tick, stop check or host call holds the cache, for at
    /// most until `deadline`; `true` when the cache is free (PE-RSH-004).
    ///
    /// The shutdown step after `WorkerHandle::shutdown`: the scheduler drops
    /// a timed-out run's future, never its `spawn_blocking` thread, so a tick
    /// may still be running after the worker has stopped. `false` means one
    /// still held the cache at `deadline` (it is bounded by its own `T`; the
    /// host may wait again or exit). It takes the mutex and lets it go, so a
    /// new tick could start afterwards: stop the worker first.
    ///
    /// # Blocking
    ///
    /// This blocks the calling thread up to `deadline`: from async code, call
    /// it in `tokio::task::spawn_blocking`.
    #[must_use]
    pub fn join_idle(&self, deadline: Instant) -> bool {
        self.shared.join_idle(deadline)
    }

    /// Runs `f` on the cache, waiting for any tick in flight to finish.
    ///
    /// For the reads and writes of the host process that share this cache
    /// (the same `Cache` the ticks use). A cache a panic dropped is
    /// recomposed first (S55).
    ///
    /// # Blocking
    ///
    /// This waits for the mutex, for as long as a tick runs (up to its 25 s
    /// budget plus a leftover blocking task). Do not call it from an async
    /// task: use `tokio::task::spawn_blocking`, or [`try_with_cache`](Self::try_with_cache).
    ///
    /// # Errors
    ///
    /// [`CacheLost`] when a panic dropped the cache and it cannot be recomposed.
    pub fn with_cache<T>(&self, f: impl FnOnce(&mut Cache<S, L, B>) -> T) -> Result<T, CacheLost> {
        let mut guard = self.shared.hold();
        self.shared.cache_in(&mut guard).map(f)
    }

    /// Runs `f` on the cache if no tick holds it; `None` when it is busy.
    ///
    /// Never waits for the mutex, so it is safe in async code; a cache a
    /// panic dropped is recomposed first, as [`with_cache`](Self::with_cache) does.
    ///
    /// # Errors
    ///
    /// [`CacheLost`] when a panic dropped the cache and it cannot be recomposed.
    pub fn try_with_cache<T>(
        &self,
        f: impl FnOnce(&mut Cache<S, L, B>) -> T,
    ) -> Result<Option<T>, CacheLost> {
        match self.shared.try_hold() {
            Some(mut guard) => self.shared.cache_in(&mut guard).map(|c| Some(f(c))),
            None => Ok(None),
        }
    }

    /// The escalated entries not yet announced, for a host that wants to show them.
    ///
    /// # Blocking
    ///
    /// As [`with_cache`](Self::with_cache).
    ///
    /// # Errors
    ///
    /// [`JobCacheError::Cache`] for a store error, [`JobCacheError::Lost`] as
    /// [`with_cache`](Self::with_cache).
    pub fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, JobCacheError> {
        self.with_cache(|c| c.unannounced_escalations())
            .map_err(JobCacheError::Lost)?
            .map_err(JobCacheError::Cache)
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
                " (an earlier panic poisoned the cache mutex; the tick ran on a recomposed cache)"
            } else {
                ""
            };
            RunOutcome::Failed(clip(
                &format!("{}: {}{note}", error.code(), error.detail()),
                MAX_OUTCOME_BYTES,
            ))
        }
        TickRun::Lost { error, poisoned } => {
            let cause = error
                .cause()
                .map(|e| format!(" ({}: {e})", e.code()))
                .unwrap_or_default();
            let note = if poisoned {
                " (the cache mutex had been poisoned)"
            } else {
                ""
            };
            RunOutcome::Failed(clip(
                &format!("{}: {error}{cause}{note}", error.code()),
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
    /// The phases directory does not exist; its code is the tick's
    /// `phases_dir_unreadable` (PE-QA-003).
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
            // The tick's code for the same directory (PE-QA-003): one code on both surfaces.
            Self::PhasesDir(_) => RefusalCode::PhasesDirUnreadable.as_str(),
            Self::Sender(_) => "atm_not_found",
            Self::Cron(_) => "cron_refused",
        }
    }
}

impl ScheduleError {
    /// What to do about it.
    #[must_use]
    pub fn fix(&self) -> &'static str {
        match self {
            Self::Config(c) => c.code().fix(),
            Self::PhasesDir(_) => RefusalCode::PhasesDirUnreadable.fix(),
            Self::Sender(e) => e.fix(),
            Self::Cron(_) => {
                "use a repository name sc-cron accepts (nonempty, no control characters)"
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
        let lost = outcome_of(TickRun::Lost {
            error: CacheLost { cause: None },
            poisoned: true,
        });
        assert!(
            matches!(&lost, RunOutcome::Failed(m) if m.starts_with("sync_cache_lost: ") && m.contains("poisoned")),
            "{lost:?}"
        );
        let long = TickReply::refused_with(Refusal::new(
            RefusalCode::PhaseFileInvalid,
            "x".repeat(10_000),
        ));
        match outcome_of(TickRun::Done(Box::new(long))) {
            RunOutcome::Refused(m) => assert!(m.len() < 1024 + 32, "{}", m.len()),
            other => panic!("{other:?}"),
        }
    }
}
