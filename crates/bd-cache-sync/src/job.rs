//! The sync job on `sc-cron` (REQ-BDC-0701, 0703; ANC-002, ANC-003, ANC-004;
//! ADR-CORE-0001; ADR-SYNC-0002).
//!
//! [`SyncJob`] is the [`Job`] a [`sc_cron::Scheduler`] runs over a cache
//! file's cron table ([`cron_db_path`](crate::cron_db_path)). It runs two kinds
//! of row ([`RowPayload`]):
//!
//! - the **built-in sync row**: one [`run_sync`](crate::run_sync) with the run's
//!   deadline less the post-sync hook's timeout as `T`, then the post-sync
//!   hook with the run's results (ANC-004, NFR-BDC-0004);
//! - a consumer's **Python row**: its module through `sc-cron`'s Python
//!   runner, `cron.tick`, with the row's payload, killed at the run's
//!   deadline (ANC-003, NFR-BDC-0004).
//!
//! Rows of other kinds are refused ([`RunOutcome::Refused`]): they belong to
//! another worker. The stop check always answers `Continue`: pausing is the
//! row's `enabled` flag (ANC-006), and REQ-BDC-0707's stop rule is retired.
//!
//! A `Cache` is `Send` but not `Sync` and its verbs block, so the job holds
//! it in an `Arc<Mutex<_>>` and runs the sync in
//! `tokio::task::spawn_blocking`:
//!
//! - a run that finds the mutex held, including by a blocking task left over
//!   from a timed-out run (the scheduler drops the future, never the
//!   thread), is skipped and reported busy;
//! - a poisoned mutex (an earlier run panicked) is never trusted: the cache
//!   the panic left behind is dropped, the poison cleared, and the next run
//!   builds a new cache from the same composition inputs (the recompose
//!   function of [`SyncJob::with_recompose`]) and reports it once (S55,
//!   PE-RBP-F007). A job built without one reports the cache lost
//!   ([`JobRun::Lost`]).
//!
//! The mutex is released before a hook starts, so a read through the job
//! ([`SyncJob::with_cache`]) never waits for a hook (REQ-BDC-0001).
//!
//! # Shutdown
//!
//! Build the job with a [`StopFlag`] ([`SyncJob::with_stop`]). To stop, in
//! this order:
//!
//! 1. set the flag: the running sync ends after the drain or between scopes;
//! 2. `WorkerHandle::shutdown` and await it: no new run starts;
//! 3. [`SyncJob::join_idle`] with a deadline, from a thread that may block
//!    (`spawn_blocking`): a sync may still be inside the cache; this waits
//!    for it, up to the deadline, and says whether it ended (PE-RSH-004);
//! 4. drop the job.
//!
//! # Clocks
//!
//! The scheduler hands a run its deadline as an `Instant`; `Deadline::at`
//! makes it the cache's `T`, and the hooks are bounded on the job's
//! `sc-cron` clock. Under the system clocks all three are `Instant::now`. A
//! test that drives `sc-cron` with its own manual clock must start both
//! clocks from one origin (`tests/common/real.rs`).

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bd_cache::Cache;
use bd_cache_model::{CacheError, CacheStore, DrainLock, JournalEntry};
use sc_beads::{CacheBackend, Deadline};
use sc_cron::{Clock, Job, PythonHook, RunOutcome, ScheduleName, StopDecision};
use serde_json::Value;

use crate::compose::ComposeError;
use crate::control::StopFlag;
use crate::hook::{HookRun, post_sync_request, run_hook};
use crate::report::SyncRun;
use crate::run::run_sync;
use crate::schedule::{BUILTIN_NAME, HookSpec, PYTHON_KIND, RowPayload};
use crate::text::clip;

/// The most bytes of a run's summary kept in the scheduler's recorded outcome
/// (it is stored on every run).
const MAX_OUTCOME_BYTES: usize = 1024;

/// How often [`SyncJob::join_idle`] looks at the cache mutex again.
const IDLE_POLL: Duration = Duration::from_millis(5);

/// What the sync part of one run did.
#[derive(Debug)]
#[non_exhaustive]
pub enum JobRun {
    /// The sync ran, or was skipped: the run says which (`busy`) and how it went.
    Done(Box<SyncRun>),
    /// The sync failed with a store or cache error.
    Failed {
        /// The error.
        error: CacheError,
        /// An earlier panic poisoned the cache mutex; the cache it left was
        /// dropped and this run used a recomposed one.
        poisoned: bool,
    },
    /// There is no cache to sync: an earlier panic dropped it and it could
    /// not be recomposed (S55).
    Lost {
        /// Why there is no cache.
        error: CacheLost,
        /// The panic is reported by this run (once, as for the other variants).
        poisoned: bool,
    },
}

/// One run of the built-in sync row: the sync, then the post-sync hook.
#[derive(Debug)]
pub struct BuiltInRun {
    /// The sync.
    pub sync: JobRun,
    /// The post-sync hook, when the row names one and the sync ran.
    pub hook: Option<HookRun>,
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

/// What the post-sync hook is told besides the run: the escalations not yet
/// marked and the pending count, read under the same lock as the run.
type HookInput = (Vec<JournalEntry>, u64);

/// What the job needs from its owner, shared with the blocking tasks.
struct Shared<S: CacheStore, L: DrainLock, B: CacheBackend> {
    cache: Mutex<Slot<S, L, B>>,
    recompose: Option<Recompose<S, L, B>>,
    stop: StopFlag,
    /// The repository root a hook is told (it runs `bdc` there).
    repo_root: PathBuf,
    /// The clock the hooks are bounded on (the scheduler's).
    clock: Arc<dyn Clock>,
    /// A panic that dropped the cache and nobody has reported yet.
    poison_unreported: AtomicBool,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> Shared<S, L, B> {
    /// Recovers a poisoned mutex without trusting what it holds: a panic may
    /// have left the cache between two steps of a verb (S55), so it is
    /// dropped. The poison is cleared and remembered until a run reports it;
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

    /// The sync part of a run, with what a hook is told when `for_hook`.
    fn run_sync(&self, end: Deadline, for_hook: bool) -> (JobRun, Option<HookInput>) {
        let Some(mut guard) = self.try_hold() else {
            return (JobRun::Done(Box::new(SyncRun::busy())), None);
        };
        let poisoned = self.poison_unreported.swap(false, Ordering::SeqCst);
        let cache = match self.cache_in(&mut guard) {
            Ok(cache) => cache,
            Err(error) => return (JobRun::Lost { error, poisoned }, None),
        };
        match run_sync(cache, end, &self.stop) {
            Ok(mut run) => {
                run.poisoned = poisoned;
                let input = if for_hook {
                    match (cache.unannounced_escalations(), cache.open_work()) {
                        (Ok(e), Ok(w)) => Some((e, w.pending)),
                        (Err(error), _) | (_, Err(error)) => {
                            return (JobRun::Failed { error, poisoned }, None);
                        }
                    }
                } else {
                    None
                };
                (JobRun::Done(Box::new(run)), input)
            }
            Err(error) => (JobRun::Failed { error, poisoned }, None),
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

/// The job `sc-cron` runs over a cache file's cron table.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
/// use std::time::{Duration, Instant};
/// use bd_cache_sync::{StopFlag, SyncJob, builtin_spec, compose, cron_db_path, register_builtin};
/// use sc_beads::{Clock, SystemClock};
/// use sc_cron::{Scheduler, SqliteScheduleStore, SystemClock as CronClock, SystemJitter};
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let clock: Arc<dyn Clock> = Arc::new(SystemClock);
/// let root = std::path::PathBuf::from(".");
/// let composed = compose(&root, "bd", Arc::clone(&clock))?;
/// let scheduler = Scheduler::new(
///     Arc::new(SqliteScheduleStore::open(cron_db_path(&composed.cache_path))?),
///     Arc::new(CronClock),
///     Arc::new(SystemJitter),
/// );
/// register_builtin(&scheduler, builtin_spec(None, None, None)?)?;
/// let stop = StopFlag::new();
/// // After a panic the job drops its cache and composes a new one from the
/// // same inputs (`compose` shares one lock set across the process).
/// let job = SyncJob::new(composed.cache, root.clone(), Arc::new(CronClock))
///     .with_stop(stop.clone())
///     .with_recompose(move || compose(&root, "bd", Arc::clone(&clock)).map(|c| c.cache));
/// let worker = scheduler.spawn_worker(Arc::new(job.clone()));
/// // ... later: set the flag, wait for the worker, then for the sync in flight
/// stop.stop();
/// worker.shutdown(tokio::time::Instant::now() + Duration::from_secs(30)).await;
/// let idle = tokio::task::spawn_blocking(move || {
///     job.join_idle(Instant::now() + Duration::from_secs(30))
/// })
/// .await?;
/// if !idle {
///     eprintln!("a sync was still running at the shutdown deadline");
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
            .field("repo_root", &self.shared.repo_root)
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
    /// A job over `cache` for the repository at `repo_root`, its hooks
    /// bounded on `clock` (the scheduler's).
    ///
    /// Only one runner may drive a cache file: do not build a second job, or
    /// call `drain` and `sync` from another scheduler, on the same file.
    ///
    /// The job cannot rebuild `cache`: after a panic in a run it reports the
    /// cache lost ([`JobRun::Lost`]). A host uses
    /// [`with_recompose`](Self::with_recompose).
    #[must_use]
    pub fn new(cache: Cache<S, L, B>, repo_root: PathBuf, clock: Arc<dyn Clock>) -> Self {
        Self {
            shared: Arc::new(Shared {
                cache: Mutex::new(Some(cache)),
                recompose: None,
                stop: StopFlag::new(),
                repo_root,
                clock,
                poison_unreported: AtomicBool::new(false),
            }),
        }
    }

    /// The job with the stop flag the host will set to shut down.
    ///
    /// # Panics
    ///
    /// When the job was already cloned (set the flag before sharing the job).
    #[must_use]
    pub fn with_stop(self, stop: StopFlag) -> Self {
        self.rebuild(|shared| shared.stop = stop)
    }

    /// The job with the function that builds a new cache from the same
    /// composition inputs as its cache (S55, PE-RBP-F007).
    ///
    /// A panic in a run or a host call may leave the cache between two steps,
    /// so the job never reuses it: it drops it, and the next run (or host
    /// call) calls `recompose` and runs on its cache. A failing `recompose` is
    /// [`JobRun::Lost`] and is tried again by the next run. For the real
    /// cache, `recompose` calls [`compose`](crate::compose) (or
    /// [`compose_with`](crate::compose_with) with the same lock set) with the
    /// arguments that built the cache.
    ///
    /// # Panics
    ///
    /// When the job was already cloned (set it before sharing the job).
    #[must_use]
    pub fn with_recompose(
        self,
        recompose: impl Fn() -> Result<Cache<S, L, B>, ComposeError> + Send + Sync + 'static,
    ) -> Self {
        self.rebuild(|shared| shared.recompose = Some(Box::new(recompose)))
    }

    #[expect(
        clippy::expect_used,
        reason = "a builder called on a shared job is a host bug (documented)"
    )]
    fn rebuild(self, f: impl FnOnce(&mut Shared<S, L, B>)) -> Self {
        let mut shared = Arc::try_unwrap(self.shared)
            .ok()
            .expect("configure a SyncJob before cloning it");
        f(&mut shared);
        Self {
            shared: Arc::new(shared),
        }
    }

    /// The stop flag (a clone): set it to end the running sync early.
    #[must_use]
    pub fn stop_flag(&self) -> StopFlag {
        self.shared.stop.clone()
    }

    /// The repository root the post-sync hook is told.
    #[must_use]
    pub fn repo_root(&self) -> &Path {
        &self.shared.repo_root
    }

    /// Runs the sync part of a run once on the calling thread, which must be
    /// allowed to block; `end` is its `T`.
    ///
    /// It does not wait for the cache: a held cache gives a busy run.
    #[must_use]
    pub fn run_blocking(&self, end: Deadline) -> JobRun {
        self.shared.run_sync(end, false).0
    }

    /// Waits until no sync or host call holds the cache, for at most until
    /// `deadline`; `true` when the cache is free (PE-RSH-004).
    ///
    /// The shutdown step after `WorkerHandle::shutdown`: the scheduler drops
    /// a timed-out run's future, never its `spawn_blocking` thread, so a sync
    /// may still be running after the worker has stopped. `false` means one
    /// still held the cache at `deadline` (it is bounded by its own `T`; the
    /// host may wait again or exit). It takes the mutex and lets it go, so a
    /// new run could start afterwards: stop the worker first.
    ///
    /// # Blocking
    ///
    /// This blocks the calling thread up to `deadline`: from async code, call
    /// it in `tokio::task::spawn_blocking`.
    #[must_use]
    pub fn join_idle(&self, deadline: Instant) -> bool {
        self.shared.join_idle(deadline)
    }

    /// Runs `f` on the cache, waiting for any sync in flight to finish.
    ///
    /// For the reads and writes of the host process that share this cache.
    /// A cache a panic dropped is recomposed first (S55). It never waits for
    /// a hook: the cache is free while one runs.
    ///
    /// # Blocking
    ///
    /// This waits for the mutex, for as long as a sync runs (up to its
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

    /// Runs `f` on the cache if no sync holds it; `None` when it is busy.
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

/// The scheduler's outcome of a run of the built-in row.
///
/// `Ok` when every scope synced (or was deferred) and the hook, if any, ran
/// clean; `Refused` when the run was skipped as busy; `Failed` with each
/// scope error and the hook's failure otherwise.
#[must_use]
pub fn builtin_outcome(run: &BuiltInRun) -> RunOutcome {
    let mut problems: Vec<String> = Vec::new();
    let mut busy = false;
    match &run.sync {
        JobRun::Done(sync) => {
            busy = sync.busy;
            if sync.poisoned {
                problems.push(
                    "an earlier panic poisoned the cache mutex; this run used a recomposed cache"
                        .into(),
                );
            }
            for s in sync.failures() {
                if let Some(e) = &s.error {
                    problems.push(format!("{}: {}: {}", s.scope, e.code, e.message));
                }
            }
        }
        JobRun::Failed { error, poisoned } => {
            let note = if *poisoned {
                " (an earlier panic poisoned the cache mutex; the run used a recomposed cache)"
            } else {
                ""
            };
            problems.push(format!("{}: {}{note}", error.code(), error.detail()));
        }
        JobRun::Lost { error, poisoned } => {
            let cause = error
                .cause()
                .map(|e| format!(" ({}: {e})", e.code()))
                .unwrap_or_default();
            let note = if *poisoned {
                " (the cache mutex had been poisoned)"
            } else {
                ""
            };
            problems.push(format!("{}: {error}{cause}{note}", error.code()));
        }
    }
    match &run.hook {
        Some(HookRun::Failed { detail }) => problems.push(format!("post-sync hook: {detail}")),
        Some(HookRun::TimedOut) => {
            problems.push("post-sync hook: killed at its timeout".into());
        }
        Some(HookRun::Ok) | None => {}
    }
    if problems.is_empty() {
        if busy {
            return RunOutcome::Refused("busy: an earlier run still holds the cache".into());
        }
        return RunOutcome::Ok;
    }
    RunOutcome::Failed(clip(&problems.join("; "), MAX_OUTCOME_BYTES))
}

/// The scheduler's outcome of a Python row's run.
fn python_outcome(run: &HookRun) -> RunOutcome {
    match run {
        HookRun::Ok => RunOutcome::Ok,
        HookRun::TimedOut => RunOutcome::TimedOut,
        HookRun::Failed { detail } => RunOutcome::Failed(clip(detail, MAX_OUTCOME_BYTES)),
    }
}

#[async_trait]
impl<S, L, B> Job for SyncJob<S, L, B>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    async fn run(&self, payload: &Value, deadline: Instant) -> RunOutcome {
        match RowPayload::of(payload) {
            Err(e) => RunOutcome::Failed(clip(&e.to_string(), MAX_OUTCOME_BYTES)),
            Ok(None) => RunOutcome::Refused(format!(
                "not a row this job runs: its kind is neither {BUILTIN_KIND_TEXT} nor {PYTHON_KIND}"
            )),
            Ok(Some(RowPayload::BuiltIn { post_sync_hook })) => {
                builtin_outcome(&self.run_builtin(post_sync_hook.as_ref(), deadline).await)
            }
            Ok(Some(RowPayload::Python { hook, name })) => {
                python_outcome(&self.run_python(&hook, &name, payload, deadline).await)
            }
        }
    }

    /// Always `Continue`: a row is paused with its `enabled` flag (ANC-006),
    /// and the stop rule of REQ-BDC-0707 is retired.
    async fn should_stop(&self, _payload: &Value) -> StopDecision {
        StopDecision::Continue
    }
}

/// The built-in kind, for messages.
const BUILTIN_KIND_TEXT: &str = crate::schedule::BUILTIN_KIND;

/// The runs that cross threads: the sync goes to tokio's blocking pool.
impl<S, L, B> SyncJob<S, L, B>
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    /// Runs the built-in row once: the sync with `T` the `deadline` less the
    /// hook's timeout, then the post-sync hook `hook` names, killed at its
    /// timeout capped by the time left before `deadline` (ANC-004,
    /// NFR-BDC-0004).
    ///
    /// The sync runs on tokio's blocking pool; the cache is released before
    /// the hook starts, so a read never waits for the hook (REQ-BDC-0001).
    pub async fn run_builtin(&self, hook: Option<&HookSpec>, deadline: Instant) -> BuiltInRun {
        let budget = hook.map_or(Duration::ZERO, HookSpec::timeout);
        let sync_end = deadline.checked_sub(budget).unwrap_or(deadline);
        let shared = Arc::clone(&self.shared);
        let wants_hook = hook.is_some();
        let joined = tokio::task::spawn_blocking(move || {
            shared.run_sync(Deadline::at(sync_end), wants_hook)
        })
        .await;
        let (sync, input) = match joined {
            Ok(done) => done,
            Err(e) => {
                return BuiltInRun {
                    sync: JobRun::Failed {
                        error: CacheError::store_failed(
                            format!("the sync task did not finish ({e}); look for a panic in the log"),
                            std::io::Error::other(e.to_string()),
                        ),
                        poisoned: false,
                    },
                    hook: None,
                };
            }
        };
        let hook_run = match (hook, input, &sync) {
            (Some(spec), Some((escalations, pending)), JobRun::Done(run)) => {
                let request =
                    post_sync_request(run, &escalations, pending, &self.shared.repo_root);
                Some(self.post_sync(spec, &request, deadline).await)
            }
            _ => None,
        };
        BuiltInRun {
            sync,
            hook: hook_run,
        }
    }

    /// [`run_builtin`](Self::run_builtin) from code that is not async (`bdc
    /// cache tick`): on a current-thread runtime of its own.
    ///
    /// # Errors
    ///
    /// The I/O error when the runtime cannot be built.
    pub fn run_builtin_blocking(
        &self,
        hook: Option<&HookSpec>,
        deadline: Instant,
    ) -> std::io::Result<BuiltInRun> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok(runtime.block_on(self.run_builtin(hook, deadline)))
    }

    async fn post_sync(&self, spec: &HookSpec, request: &Value, deadline: Instant) -> HookRun {
        let (hook, name) = match (spec.hook(), ScheduleName::new(BUILTIN_NAME)) {
            (Ok(h), Ok(n)) => (h, n),
            (Err(e), _) | (_, Err(e)) => {
                return HookRun::Failed {
                    detail: e.to_string(),
                };
            }
        };
        run_hook(
            &hook,
            &*self.shared.clock,
            &name,
            request,
            spec.timeout(),
            deadline,
        )
        .await
    }

    /// Runs a consumer's Python row: its module with the row's payload,
    /// killed at the run's deadline (ANC-003, NFR-BDC-0004). The job's cache
    /// is not touched; the module feeds it through `ingest_scope`.
    pub async fn run_python(
        &self,
        hook: &PythonHook,
        name: &ScheduleName,
        payload: &Value,
        deadline: Instant,
    ) -> HookRun {
        let left = deadline.saturating_duration_since(self.shared.clock.monotonic_now());
        run_hook(hook, &*self.shared.clock, name, payload, left, deadline).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{ScopeError, ScopeResult};

    fn run_with(sync: SyncRun, hook: Option<HookRun>) -> BuiltInRun {
        BuiltInRun {
            sync: JobRun::Done(Box::new(sync)),
            hook,
        }
    }

    /// A run's outcome names every scope error and a hook that failed, and
    /// is bounded (RSH-008); a clean run is `Ok`, a busy one `Refused`.
    #[test]
    fn the_outcome_names_scope_errors_and_the_hook_and_is_clipped() {
        assert_eq!(builtin_outcome(&run_with(SyncRun::started(), None)), RunOutcome::Ok);
        assert!(matches!(
            builtin_outcome(&run_with(SyncRun::busy(), None)),
            RunOutcome::Refused(m) if m.starts_with("busy")
        ));
        let mut failed = SyncRun::started();
        failed.scopes.push(ScopeResult {
            scope: bd_cache_model::ScopeId::new("x").unwrap(),
            result: None,
            error: Some(ScopeError {
                code: "BD_CACHE.SYNC_FAILED".into(),
                message: "m".repeat(10_000),
                outage: false,
            }),
            deferred: false,
            failing_since: None,
            notified: false,
        });
        match builtin_outcome(&run_with(failed, Some(HookRun::TimedOut))) {
            RunOutcome::Failed(m) => {
                assert!(m.starts_with("x: BD_CACHE.SYNC_FAILED: "), "{m}");
                assert!(m.len() < MAX_OUTCOME_BYTES + 32, "{}", m.len());
            }
            other => panic!("{other:?}"),
        }
        match builtin_outcome(&run_with(
            SyncRun::started(),
            Some(HookRun::Failed {
                detail: "exited with 3".into(),
            }),
        )) {
            RunOutcome::Failed(m) => assert_eq!(m, "post-sync hook: exited with 3"),
            other => panic!("{other:?}"),
        }
        let lost = BuiltInRun {
            sync: JobRun::Lost {
                error: CacheLost { cause: None },
                poisoned: true,
            },
            hook: None,
        };
        assert!(matches!(
            builtin_outcome(&lost),
            RunOutcome::Failed(m) if m.starts_with("sync_cache_lost: ") && m.contains("poisoned")
        ));
    }
}
