//! The scheduler: starts and stops schedules and spawns the worker that runs them.
//!
//! [`Scheduler::start`] checks the payload size and that the expression has
//! a future occurrence before anything is written (name, expression and
//! timeout are already valid by type), draws the schedule's jitter, computes
//! its first run and stores it. [`Scheduler::spawn_worker`] starts the one
//! tokio task that ticks every active schedule. `start` and `stop` signal it
//! to reload, so a new schedule ticks without a restart.
//!
//! `start` and `stop` call the synchronous store directly. From async code
//! use [`Scheduler::start_async`] and [`Scheduler::stop_async`], which run
//! the same calls on tokio's blocking pool so a busy database never blocks
//! an executor thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use crate::limits::encode_payload;
use crate::worker::{DEFAULT_RELOAD_POLL, MIN_RELOAD_POLL, Worker};
use crate::{
    Clock, CronError, Generation, HookTarget, JitterSource, Job, NewSchedule, Schedule, ScheduleId,
    ScheduleName, ScheduleSpec, ScheduleStore, StoreError, StoreErrorKind, Timestamp, jitter,
};

/// What the worker does with occurrences that came due while it could not run them.
///
/// Occurrences are missed while the worker is not running (process down,
/// before [`Scheduler::spawn_worker`]) or cannot wake (host suspended). An
/// occurrence counts as missed once the occurrence after it is also due.
/// A run that is merely late, with the following occurrence still in the
/// future, always runs. Either way the schedule then continues from its next
/// future occurrence, and a backlog is never run one by one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MissedTickPolicy {
    /// Replay all missed occurrences as one catch-up run as soon as the worker sees them.
    #[default]
    Coalesce,
    /// Drop missed occurrences; run again at the next future occurrence.
    Skip,
}

/// What `start`, `stop` and their async forms share, so the async forms can
/// move it onto the blocking pool.
struct Core<S, C, J> {
    store: Arc<S>,
    clock: Arc<C>,
    jitter: Arc<J>,
    reload: watch::Sender<u64>,
}

impl<S: ScheduleStore, C: Clock, J: JitterSource> Core<S, C, J> {
    fn start(&self, spec: ScheduleSpec) -> Result<Schedule, CronError> {
        encode_payload(&spec.payload)?;
        let now = self.clock.now();
        spec.expr.require_future(now)?;
        let jitter = jitter::draw(&spec.expr, now, self.jitter.as_ref());
        let next = spec.expr.next_run(jitter, now).ok_or_else(|| {
            CronError::BadExpression("expression has no future occurrence".into())
        })?;
        let schedule = NewSchedule {
            spec,
            jitter,
            next_run_at: Timestamp::from_datetime(next),
        };
        let created = self
            .store
            .create(&schedule, Timestamp::from_datetime(now))?;
        self.signal_reload();
        Ok(created)
    }

    fn stop(&self, id: ScheduleId, generation: Generation, reason: &str) -> Result<(), CronError> {
        let at = Timestamp::from_datetime(self.clock.now());
        if !self.store.stop(id, generation, at, reason)? {
            return Err(CronError::Fenced { id, generation });
        }
        self.signal_reload();
        Ok(())
    }

    fn set_enabled(&self, name: &ScheduleName, enabled: bool) -> Result<(), CronError> {
        if !self.store.set_enabled(name, enabled)? {
            return Err(CronError::NotFound(name.clone()));
        }
        self.signal_reload();
        Ok(())
    }

    fn set_enabled_many(&self, flags: &[(ScheduleName, bool)]) -> Result<(), CronError> {
        if !self.store.set_enabled_many(flags)? {
            return Err(self.first_missing(flags.iter().map(|(name, _)| name))?);
        }
        self.signal_reload();
        Ok(())
    }

    fn replace_and_set_enabled(
        &self,
        spec: ScheduleSpec,
        enabled: bool,
        flags: &[(ScheduleName, bool)],
    ) -> Result<Schedule, CronError> {
        encode_payload(&spec.payload)?;
        let now = self.clock.now();
        spec.expr.require_future(now)?;
        let jitter = jitter::draw(&spec.expr, now, self.jitter.as_ref());
        let next = spec.expr.next_run(jitter, now).ok_or_else(|| {
            CronError::BadExpression("expression has no future occurrence".into())
        })?;
        let schedule = NewSchedule {
            spec,
            jitter,
            next_run_at: Timestamp::from_datetime(next),
        };
        let Some(written) = self.store.replace_and_set_enabled(
            &schedule,
            enabled,
            Timestamp::from_datetime(now),
            flags,
        )?
        else {
            return Err(self.first_missing(flags.iter().map(|(name, _)| name))?);
        };
        self.signal_reload();
        Ok(written)
    }

    /// The `NotFound` of the first of `names` with no active schedule, after
    /// a store call refused the list for one; a name gone by the time of this
    /// read is reported as the first name.
    fn first_missing<'n>(
        &self,
        mut names: impl Iterator<Item = &'n ScheduleName> + Clone,
    ) -> Result<CronError, CronError> {
        let active = self.store.list_active()?;
        let first = names.clone().next().cloned();
        let missing = names
            .find(|name| !active.iter().any(|s| s.spec.name == **name))
            .cloned()
            .or(first);
        Ok(match missing {
            Some(name) => CronError::NotFound(name),
            None => CronError::InvalidSpec("an empty list of schedules changed nothing".into()),
        })
    }

    fn signal_reload(&self) {
        self.reload
            .send_modify(|version| *version = version.wrapping_add(1));
    }
}

/// Starts, stops and runs schedules kept in a [`ScheduleStore`].
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// use sc_cron::{
///     CronError, ScheduleSpec, ScheduleStore, Scheduler, SqliteScheduleStore, SystemClock,
///     SystemJitter,
/// };
///
/// let scheduler = Scheduler::new(
///     Arc::new(SqliteScheduleStore::open_in_memory()?),
///     Arc::new(SystemClock),
///     Arc::new(SystemJitter),
/// );
/// let spec = ScheduleSpec::new("sync", "*/30 * * * * *", Duration::from_secs(25), serde_json::json!({}))?;
/// let schedule = scheduler.start(spec.clone())?;
/// assert!(schedule.is_active());
///
/// // A second start of an active name is refused and stores nothing.
/// assert!(matches!(scheduler.start(spec), Err(CronError::AlreadyActive(_))));
/// assert_eq!(scheduler.store().list_active()?.len(), 1);
/// # Ok::<(), CronError>(())
/// ```
pub struct Scheduler<S: ScheduleStore, C: Clock, J: JitterSource> {
    core: Arc<Core<S, C, J>>,
    policy: MissedTickPolicy,
    reload_poll: Duration,
    worker_live: Arc<AtomicBool>,
}

impl<S, C, J> Scheduler<S, C, J>
where
    S: ScheduleStore + 'static,
    C: Clock + 'static,
    J: JitterSource + 'static,
{
    /// Creates a scheduler over `store`, reading time from `clock`.
    ///
    /// The missed-tick policy is [`MissedTickPolicy::Coalesce`].
    #[must_use]
    pub fn new(store: Arc<S>, clock: Arc<C>, jitter: Arc<J>) -> Self {
        Self {
            core: Arc::new(Core {
                store,
                clock,
                jitter,
                reload: watch::channel(0).0,
            }),
            policy: MissedTickPolicy::default(),
            reload_poll: DEFAULT_RELOAD_POLL,
            worker_live: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Sets the missed-tick policy for workers spawned after this call.
    #[must_use]
    pub fn with_missed_tick_policy(mut self, policy: MissedTickPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Sets the longest a worker spawned after this call goes between
    /// reloads of the active schedules (REQ-CRON-0014).
    ///
    /// A schedule started, restarted, enabled, disabled or stopped by another
    /// process or scheduler sends this scheduler's worker no signal, so the
    /// worker reloads at least once per `interval` on the scheduler's clock,
    /// whatever the rows are. Such a change is therefore noticed at most
    /// `interval` after it is stored (a reload that fails is retried with a
    /// backoff, so the bound holds while the store can be read). A disable or
    /// stop is also noticed earlier, at the schedule's next tick. Each reload
    /// is one `list_active` query, so the poll costs one query per interval.
    /// The default is [`DEFAULT_RELOAD_POLL`] (30 s); an interval under one
    /// second is raised to one second.
    #[must_use]
    pub fn with_reload_poll(mut self, interval: Duration) -> Self {
        self.reload_poll = interval.max(MIN_RELOAD_POLL);
        self
    }

    /// Returns the store, for reading schedules and their run state.
    #[must_use]
    pub fn store(&self) -> &Arc<S> {
        &self.core.store
    }

    /// Stores a new schedule, checking it before any write.
    ///
    /// Restarting an ended name keeps its id and raises its generation, so
    /// any run still in flight from the old generation is fenced. This
    /// calls the store synchronously; from async code use
    /// [`Scheduler::start_async`].
    ///
    /// # Errors
    ///
    /// [`CronError::InvalidSpec`] for a payload over
    /// [`MAX_PAYLOAD_BYTES`](crate::MAX_PAYLOAD_BYTES) and
    /// [`CronError::BadExpression`] for an expression with no future
    /// occurrence, both before anything is stored; then the store's errors,
    /// including [`CronError::AlreadyActive`].
    pub fn start(&self, spec: ScheduleSpec) -> Result<Schedule, CronError> {
        self.core.start(spec)
    }

    /// [`Scheduler::start`] on tokio's blocking pool.
    ///
    /// # Errors
    ///
    /// As [`Scheduler::start`], or [`CronError::Store`] if the blocking task
    /// panics.
    pub async fn start_async(&self, spec: ScheduleSpec) -> Result<Schedule, CronError> {
        let core = Arc::clone(&self.core);
        blocking(move || core.start(spec)).await
    }

    /// Ends schedule `(id, generation)` with `reason`; its ticks stop.
    ///
    /// A run already in flight is cancelled and records nothing. This calls
    /// the store synchronously; from async code use [`Scheduler::stop_async`].
    ///
    /// # Errors
    ///
    /// [`CronError::Fenced`] if `(id, generation)` is not the active
    /// schedule, or the store's errors.
    pub fn stop(
        &self,
        id: ScheduleId,
        generation: Generation,
        reason: &str,
    ) -> Result<(), CronError> {
        self.core.stop(id, generation, reason)
    }

    /// [`Scheduler::stop`] on tokio's blocking pool.
    ///
    /// # Errors
    ///
    /// As [`Scheduler::stop`], or [`CronError::Store`] if the blocking task
    /// panics.
    pub async fn stop_async(
        &self,
        id: ScheduleId,
        generation: Generation,
        reason: &str,
    ) -> Result<(), CronError> {
        let core = Arc::clone(&self.core);
        let reason = reason.to_owned();
        blocking(move || core.stop(id, generation, &reason)).await
    }

    /// Enables or disables the active schedule `name`; only its `enabled`
    /// flag changes (REQ-CRON-0014).
    ///
    /// A disabled schedule never fires. It keeps its id, generation,
    /// expression, payload and stored next run, so enabling it again resumes
    /// it: a run that came due while it was disabled counts as missed and is
    /// handled per the [`MissedTickPolicy`]. A run in flight when it is
    /// disabled is cancelled and records nothing, as with
    /// [`Scheduler::stop`]. This scheduler's worker is told at once. A worker
    /// in another process or on another scheduler is not told: it notices the
    /// change at its next reload, at most [`Scheduler::with_reload_poll`]
    /// (default 30 s) after the write, or for a disable when its next tick
    /// finds the schedule disabled, if that is sooner. This calls the store synchronously; from
    /// async code use [`Scheduler::set_enabled_async`].
    ///
    /// # Errors
    ///
    /// [`CronError::NotFound`] if no active schedule has this name, or the
    /// store's errors.
    pub fn set_enabled(&self, name: &ScheduleName, enabled: bool) -> Result<(), CronError> {
        self.core.set_enabled(name, enabled)
    }

    /// [`Scheduler::set_enabled`] on tokio's blocking pool.
    ///
    /// # Errors
    ///
    /// As [`Scheduler::set_enabled`], or [`CronError::Store`] if the
    /// blocking task panics.
    pub async fn set_enabled_async(
        &self,
        name: &ScheduleName,
        enabled: bool,
    ) -> Result<(), CronError> {
        let core = Arc::clone(&self.core);
        let name = name.clone();
        blocking(move || core.set_enabled(&name, enabled)).await
    }

    /// Sets the `enabled` flag of every named active schedule in one store
    /// transaction (REQ-CRON-0017), and tells this scheduler's worker as
    /// [`Scheduler::set_enabled`] does (REQ-CRON-0014).
    ///
    /// Another connection sees every flag of the list change at once, or
    /// none. A worker in another process notices at its next reload. This
    /// calls the store synchronously; from async code use
    /// [`Scheduler::set_enabled_many_async`].
    ///
    /// # Errors
    ///
    /// [`CronError::NotFound`] naming a listed schedule that is not active,
    /// with nothing changed, or the store's errors.
    pub fn set_enabled_many(&self, flags: &[(ScheduleName, bool)]) -> Result<(), CronError> {
        self.core.set_enabled_many(flags)
    }

    /// [`Scheduler::set_enabled_many`] on tokio's blocking pool.
    ///
    /// # Errors
    ///
    /// As [`Scheduler::set_enabled_many`], or [`CronError::Store`] if the
    /// blocking task panics.
    pub async fn set_enabled_many_async(
        &self,
        flags: Vec<(ScheduleName, bool)>,
    ) -> Result<(), CronError> {
        let core = Arc::clone(&self.core);
        blocking(move || core.set_enabled_many(&flags)).await
    }

    /// Writes the schedule `spec` with its `enabled` flag and sets each
    /// listed other schedule's flag, in one store transaction (REQ-CRON-0017).
    ///
    /// The jitter, the next run and the creation time come from the injected
    /// sources, as [`Scheduler::start`] does, and the same checks run before
    /// anything is written. A new name is created; an ended name is restarted
    /// in place; an active name is replaced in place with its generation
    /// raised, so a run of the old generation is fenced. The worker is told as
    /// [`Scheduler::set_enabled`] does. This lets a caller keep "at most one
    /// of these schedules enabled" across a create or an update, which
    /// [`Scheduler::start`] (always enabled, `AlreadyActive` for an active
    /// name) cannot. This calls the store synchronously; from async code use
    /// [`Scheduler::replace_and_set_enabled_async`].
    ///
    /// # Errors
    ///
    /// As [`Scheduler::start`] for the spec; [`CronError::InvalidSpec`] if
    /// `flags` names `spec`'s schedule; [`CronError::NotFound`] naming a
    /// listed schedule that is not active, with nothing written; or the
    /// store's errors.
    pub fn replace_and_set_enabled(
        &self,
        spec: ScheduleSpec,
        enabled: bool,
        flags: &[(ScheduleName, bool)],
    ) -> Result<Schedule, CronError> {
        self.core.replace_and_set_enabled(spec, enabled, flags)
    }

    /// [`Scheduler::replace_and_set_enabled`] on tokio's blocking pool.
    ///
    /// # Errors
    ///
    /// As [`Scheduler::replace_and_set_enabled`], or [`CronError::Store`] if
    /// the blocking task panics.
    pub async fn replace_and_set_enabled_async(
        &self,
        spec: ScheduleSpec,
        enabled: bool,
        flags: Vec<(ScheduleName, bool)>,
    ) -> Result<Schedule, CronError> {
        let core = Arc::clone(&self.core);
        blocking(move || core.replace_and_set_enabled(spec, enabled, &flags)).await
    }

    /// Spawns the worker that runs the Rust `job` for every active schedule.
    ///
    /// This is [`Scheduler::spawn_worker_for`] with [`HookTarget::Rust`].
    ///
    /// # Panics
    ///
    /// As [`Scheduler::spawn_worker_for`].
    pub fn spawn_worker(&self, job: Arc<dyn Job>) -> WorkerHandle {
        self.spawn_worker_for(HookTarget::Rust(job))
    }

    /// Spawns the worker that runs `target` for every active, enabled schedule
    /// (REQ-CRON-0013, REQ-CRON-0014).
    ///
    /// The worker loads the active schedules from the store, then sleeps on
    /// the clock until the next run, run deadline or reload signal. A failed
    /// load is retried with a backoff of 1 s doubling to 60 s. It also
    /// reloads at least once per [`Scheduler::with_reload_poll`] (default
    /// 30 s), so a schedule started, restarted, enabled or stopped by another
    /// process is noticed within that interval.
    ///
    /// Stop the worker with [`WorkerHandle::shutdown`] and await it: that is
    /// the only way to know in-flight outcomes were recorded before the
    /// process exits. Dropping the handle only signals the worker, which
    /// then drains detached.
    ///
    /// Run timeouts cancel a run at its next `.await`. A job that blocks a
    /// runtime thread without awaiting cannot be cancelled, and on a
    /// current-thread runtime it would also stall the worker; use a
    /// multi-thread runtime and `spawn_blocking` for blocking work (see
    /// [`Job`]).
    ///
    /// # Panics
    ///
    /// Panics outside a tokio runtime, or if a worker from this scheduler is
    /// still running: two workers would run every schedule twice.
    #[must_use]
    pub fn spawn_worker_for(&self, target: HookTarget) -> WorkerHandle {
        let claimed = self
            .worker_live
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        assert!(
            claimed,
            "sc-cron: a worker for this scheduler is already running"
        );
        let live = LiveGuard(Arc::clone(&self.worker_live));
        let reload = self.core.reload.subscribe();
        let (shutdown, shutdown_rx) = oneshot::channel();
        let worker = Worker::new(
            Arc::clone(&self.core.store),
            Arc::clone(&self.core.clock),
            target,
            self.policy,
            self.reload_poll,
            shutdown_rx,
        );
        let task = tokio::spawn(async move {
            let _live = live;
            worker.run(reload).await;
        });
        WorkerHandle {
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }
}

/// Runs `operation` on the blocking pool, mapping a panic to a store error.
async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, CronError> + Send + 'static,
) -> Result<T, CronError> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| {
            CronError::Store(
                StoreError::new(StoreErrorKind::Other, "blocking store task failed")
                    .with_source(error),
            )
        })?
}

impl<S: ScheduleStore, C: Clock, J: JitterSource> std::fmt::Debug for Scheduler<S, C, J> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler")
            .field("policy", &self.policy)
            .field("worker_live", &self.worker_live.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

/// Clears the scheduler's one-worker flag when the worker task ends or is dropped.
struct LiveGuard(Arc<AtomicBool>);

impl Drop for LiveGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Owner of a running worker; await [`WorkerHandle::shutdown`] to stop it.
///
/// Dropping the handle signals the worker to stop, but the worker then
/// drains its in-flight runs detached, and their outcomes are lost if the
/// process exits first.
#[derive(Debug)]
pub struct WorkerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    /// Stops the worker, waiting for in-flight runs until `deadline`.
    ///
    /// The worker stops launching runs, then waits for each in-flight run
    /// until that run's own deadline, recording its outcome. If `deadline`
    /// passes first, the worker and its runs are aborted and record nothing.
    /// Returns whether the worker finished before `deadline`.
    pub async fn shutdown(mut self, deadline: tokio::time::Instant) -> bool {
        if let Some(shutdown) = self.shutdown.take() {
            // The worker may already have exited; nothing to signal then.
            let _ = shutdown.send(());
        }
        let Some(mut task) = self.task.take() else {
            return true;
        };
        if tokio::time::timeout_at(deadline, &mut task).await.is_ok() {
            return true;
        }
        tracing::error!(
            subsystem = "sc_cron",
            action = "shutdown_deadline",
            "worker did not stop before the shutdown deadline; aborting"
        );
        task.abort();
        // The task was just aborted; its result is cancellation.
        let _ = task.await;
        false
    }

    /// Returns whether the worker task has ended.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            // The worker may already have exited; nothing to signal then.
            let _ = shutdown.send(());
        }
    }
}
