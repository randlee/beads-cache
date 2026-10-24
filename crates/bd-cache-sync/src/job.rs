//! The sync task on `sc-cron` (REQ-BDC-0701, 0703, 0707; ADR-CORE-0001).
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
//! - a poisoned mutex (an earlier tick panicked) is recovered, cleared and
//!   reported once in the tick reply.

use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::Instant;

use async_trait::async_trait;
use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainLock, EscalationSender, JournalEntry, TickConfig,
};
use sc_beads::{CacheBackend, Clock, Deadline};
use sc_cron::{Job, RunOutcome, ScheduleSpec, StopDecision};

use crate::phases::{ConfigRefusal, TickSettings};
use crate::reply::{TickOutcome, TickReply};
use crate::tick::{TICK_BUDGET, stop_reason, tick};

/// The cron expression of the schedule: every 30 s (BD-CACHE.md, Cron module).
///
/// The tick interval is the retry interval of everything the drain and the
/// sync leave for later (ADR-BDC-0005).
pub const TICK_EXPR: &str = "*/30 * * * * *";

/// What one run of the job did.
#[derive(Debug)]
#[non_exhaustive]
pub enum TickRun {
    /// An earlier tick still holds the cache; this one did nothing.
    Busy,
    /// The tick ran; the reply says how it went.
    Done(Box<TickReply>),
    /// The tick failed with a store or cache error.
    Failed(CacheError),
}

/// What the job needs from its owner, shared with the blocking tasks.
struct Shared<S: CacheStore, L: DrainLock, B: CacheBackend> {
    cache: Mutex<Cache<S, L, B>>,
    config: TickConfig,
    sender: Arc<dyn EscalationSender>,
    clock: Arc<dyn Clock>,
}

/// The cache, locked for one tick.
struct Held<'a, S: CacheStore, L: DrainLock, B: CacheBackend> {
    guard: MutexGuard<'a, Cache<S, L, B>>,
    poisoned: bool,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> Shared<S, L, B> {
    /// Takes the cache without waiting; `None` when it is in use.
    fn try_hold(&self) -> Option<Held<'_, S, L, B>> {
        match self.cache.try_lock() {
            Ok(guard) => Some(Held {
                guard,
                poisoned: false,
            }),
            Err(TryLockError::Poisoned(p)) => {
                // A panicking tick left the cache between two statements of a
                // verb; every verb keeps its state in the store's
                // transactions, so the cache is usable. Clear the poison so
                // it is reported once.
                let guard = p.into_inner();
                self.cache.clear_poison();
                Some(Held {
                    guard,
                    poisoned: true,
                })
            }
            Err(TryLockError::WouldBlock) => None,
        }
    }

    fn run_tick(&self, end: Deadline) -> TickRun {
        let Some(mut held) = self.try_hold() else {
            return TickRun::Busy;
        };
        match tick(
            &mut held.guard,
            &self.config,
            &*self.sender,
            &*self.clock,
            end,
        ) {
            Ok(mut reply) => {
                if held.poisoned {
                    reply.poisoned = true;
                    reply.refused.push(
                        "the cache mutex was poisoned by an earlier panic and was recovered"
                            .to_owned(),
                    );
                    reply.outcome = TickOutcome::Refused;
                }
                TickRun::Done(Box::new(reply))
            }
            Err(e) => TickRun::Failed(e),
        }
    }

    fn run_stop(&self, end: Deadline) -> StopDecision {
        let Some(mut held) = self.try_hold() else {
            // Busy: a blocking task is still working; never stop under it.
            return StopDecision::Continue;
        };
        match stop_reason(&mut held.guard, &self.config, &*self.clock, end) {
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
/// use bd_cache_sync::{AtmSender, SyncJob, compose, schedule_spec};
/// use sc_beads::{Clock, SystemClock};
/// use sc_cron::{Scheduler, SqliteScheduleStore, SystemClock as CronClock, SystemJitter};
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let clock: Arc<dyn Clock> = Arc::new(SystemClock);
/// let root = std::path::Path::new(".");
/// let composed = compose(root, "bd", Arc::clone(&clock))?;
/// let spec = schedule_spec(&composed.tick, &composed.name)?;
/// let job = SyncJob::new(composed.cache, composed.tick, Arc::new(AtmSender::default()), clock);
/// let scheduler = Scheduler::new(
///     Arc::new(SqliteScheduleStore::open(".bd-cache/cron.db")?),
///     Arc::new(CronClock),
///     Arc::new(SystemJitter),
/// );
/// scheduler.start_async(spec).await?;
/// let worker = scheduler.spawn_worker(Arc::new(job));
/// // ... later
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
            .field("config", &self.shared.config)
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
    /// `clock` must be the clock `cache` and its backend share. Only one
    /// ticker may run per cache file: do not build a second job, or call
    /// `drain` and `sync` from another scheduler, on the same file.
    #[must_use]
    pub fn new(
        cache: Cache<S, L, B>,
        config: TickConfig,
        sender: Arc<dyn EscalationSender>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                cache: Mutex::new(cache),
                config,
                sender,
                clock,
            }),
        }
    }

    /// Runs one tick on the calling thread, which must be allowed to block.
    ///
    /// This is what the scheduler's blocking task calls; it is public for
    /// hosts that drive the tick from their own loop and for tests.
    #[must_use]
    pub fn run_blocking(&self, end: Deadline) -> TickRun {
        self.shared.run_tick(end)
    }

    /// Runs the stop check on the calling thread.
    #[must_use]
    pub fn should_stop_blocking(&self, end: Deadline) -> StopDecision {
        self.shared.run_stop(end)
    }

    /// Runs `f` on the cache, waiting for any tick in flight to finish.
    ///
    /// For the reads and writes of the host process that share this cache
    /// (the same `Cache` the ticks use); a poisoned mutex is recovered.
    pub fn with_cache<T>(&self, f: impl FnOnce(&mut Cache<S, L, B>) -> T) -> T {
        let mut guard = self
            .shared
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut guard)
    }

    /// The escalated entries not yet announced, for a host that wants to show them.
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
        TickRun::Busy => RunOutcome::Refused(
            "busy: an earlier tick still holds the cache; this tick was skipped".to_owned(),
        ),
        TickRun::Done(reply) => match reply.outcome {
            TickOutcome::PassThrough => RunOutcome::Ok,
            TickOutcome::Refused => RunOutcome::Refused(reply.refused.join("; ")),
        },
        TickRun::Failed(e) => RunOutcome::Failed(format!("{}: {}", e.code(), e.detail())),
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
            Err(e) => RunOutcome::Failed(format!("the tick task did not finish: {e}")),
        }
    }

    async fn should_stop(&self, _payload: &serde_json::Value) -> StopDecision {
        let shared = Arc::clone(&self.shared);
        // The stop check shares the schedule's budget with the run; its root
        // read is bounded by the miss-fill timeout, and by this deadline.
        let end = Deadline::after(&*self.shared.clock, TICK_BUDGET);
        match tokio::task::spawn_blocking(move || shared.run_stop(end)).await {
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
    /// `sc-cron` refused the specification.
    Cron(sc_cron::CronError),
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(c) => write!(f, "the sync schedule is refused: {c}"),
            Self::Cron(c) => write!(f, "sc-cron refused the sync schedule: {c}"),
        }
    }
}

impl std::error::Error for ScheduleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(c) => Some(c),
            Self::Cron(c) => Some(c),
        }
    }
}

/// The one schedule of a repository (REQ-BDC-0701): every 30 s with a 25 s
/// budget, named `bd-cache:<repo>`.
///
/// # Errors
///
/// [`ScheduleError::Config`] when the `[tick]` table has no team or no
/// fallback lead (or any other refusal the tick would give), so a schedule
/// that could never announce is not started; [`ScheduleError::Cron`] for a
/// name `sc-cron` does not accept.
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
/// let spec = schedule_spec(&cfg, "atm-dev").unwrap();
/// assert_eq!(spec.name.as_str(), "bd-cache:atm-dev");
/// // No team, no schedule (REQ-BDC-0701).
/// assert!(schedule_spec(&TickConfig::default(), "atm-dev").is_err());
/// ```
pub fn schedule_spec(config: &TickConfig, repo: &str) -> Result<ScheduleSpec, ScheduleError> {
    let settings = TickSettings::from_config(config).map_err(ScheduleError::Config)?;
    ScheduleSpec::new(
        &format!("bd-cache:{repo}"),
        TICK_EXPR,
        TICK_BUDGET,
        serde_json::json!({ "team": settings.team(), "fallback_lead": settings.fallback_lead() }),
    )
    .map_err(ScheduleError::Cron)
}
