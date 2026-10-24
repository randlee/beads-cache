//! The scheduler: starts and stops schedules and spawns the worker that runs them.
//!
//! [`Scheduler::start`] validates the spec and parses the expression before
//! anything is written, draws the schedule's jitter, computes its first run
//! and stores it. [`Scheduler::spawn_worker`] starts the one tokio task that
//! ticks every active schedule; `start` and `stop` signal it to reload, so a
//! new schedule ticks without a restart.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use crate::worker::Worker;
use crate::{
    Clock, CronError, Generation, JitterSource, Job, NewSchedule, Schedule, ScheduleId,
    ScheduleSpec, ScheduleStore, expr, jitter, timestamp,
};

/// Longest timeout a schedule may carry.
pub const MAX_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

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
/// let schedule = scheduler.start(ScheduleSpec {
///     name: "sync".into(),
///     expr: "*/30 * * * * *".into(),
///     timeout: Duration::from_secs(25),
///     payload: serde_json::json!({"scope": "phase-a"}),
/// })?;
/// assert!(schedule.is_active());
///
/// // Five-field expressions are refused before anything is stored.
/// let five_field = ScheduleSpec { name: "other".into(), expr: "* * * * *".into(), ..schedule.spec };
/// assert!(matches!(scheduler.start(five_field), Err(CronError::BadExpression(_))));
/// assert_eq!(scheduler.store().list_active()?.len(), 1);
/// # Ok::<(), sc_cron::CronError>(())
/// ```
pub struct Scheduler<S: ScheduleStore, C: Clock, J: JitterSource> {
    store: Arc<S>,
    clock: Arc<C>,
    jitter: Arc<J>,
    policy: MissedTickPolicy,
    reload: watch::Sender<u64>,
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
            store,
            clock,
            jitter,
            policy: MissedTickPolicy::default(),
            reload: watch::channel(0).0,
            worker_live: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Sets the missed-tick policy for workers spawned after this call.
    #[must_use]
    pub fn with_missed_tick_policy(mut self, policy: MissedTickPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Returns the store, for reading schedules and their run state.
    #[must_use]
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    /// Validates, parses and stores a new schedule; parses before any write.
    ///
    /// Restarting an ended name keeps its id and raises its generation, so
    /// any run still in flight from the old generation is fenced. The store
    /// call is synchronous; from async code, call this on the blocking pool.
    ///
    /// # Errors
    ///
    /// [`CronError::InvalidSpec`] for an empty name or a timeout outside
    /// 1 ms..=[`MAX_TIMEOUT`], [`CronError::BadExpression`] for an expression
    /// that does not parse, has five fields or has no future occurrence, both
    /// before anything is stored; then the store's errors, including
    /// [`CronError::AlreadyActive`].
    pub fn start(&self, spec: ScheduleSpec) -> Result<Schedule, CronError> {
        if spec.name.trim().is_empty() {
            return Err(CronError::InvalidSpec("name is empty".into()));
        }
        if spec.timeout < Duration::from_millis(1) || spec.timeout > MAX_TIMEOUT {
            return Err(CronError::InvalidSpec(format!(
                "timeout must be between 1 ms and {} s",
                MAX_TIMEOUT.as_secs()
            )));
        }
        let now = self.clock.now();
        let parsed = expr::parse(&spec.expr, now)?;
        let jitter = jitter::draw(&parsed, now, self.jitter.as_ref());
        let next = expr::next_run(&parsed, jitter, now).ok_or_else(|| {
            CronError::BadExpression("expression has no future occurrence".into())
        })?;
        let schedule = NewSchedule {
            spec,
            jitter,
            next_run_at: timestamp::format(next),
        };
        let created = self.store.create(&schedule, &timestamp::format(now))?;
        self.signal_reload();
        Ok(created)
    }

    /// Ends schedule `(id, generation)` with `reason`; its ticks stop.
    ///
    /// A run already in flight is cancelled and records nothing. Like
    /// [`Scheduler::start`], the store call is synchronous.
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
        let at = timestamp::format(self.clock.now());
        if !self.store.stop(id, generation, &at, reason)? {
            return Err(CronError::Fenced);
        }
        self.signal_reload();
        Ok(())
    }

    /// Spawns the worker that runs `job` for every active schedule.
    ///
    /// The worker loads the active schedules from the store, then sleeps on
    /// the clock until the next run, run deadline or reload signal. A failed
    /// load is retried with a backoff of 1 s doubling to 60 s. Dropping or
    /// shutting down the returned handle stops it.
    ///
    /// # Panics
    ///
    /// Panics outside a tokio runtime, or if a worker from this scheduler is
    /// still running: two workers would run every schedule twice.
    pub fn spawn_worker(&self, job: Arc<dyn Job>) -> WorkerHandle {
        let claimed = self
            .worker_live
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        assert!(
            claimed,
            "sc-cron: a worker for this scheduler is already running"
        );
        let live = LiveGuard(Arc::clone(&self.worker_live));
        let worker = Worker::new(
            Arc::clone(&self.store),
            Arc::clone(&self.clock),
            job,
            self.policy,
        );
        let reload = self.reload.subscribe();
        let (shutdown, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _live = live;
            worker.run(reload, shutdown_rx).await;
        });
        WorkerHandle {
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }

    fn signal_reload(&self) {
        self.reload
            .send_modify(|version| *version = version.wrapping_add(1));
    }
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

/// Owner of a running worker; dropping it asks the worker to stop.
#[derive(Debug)]
pub struct WorkerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    /// Stops the worker, waiting for in-flight runs until `deadline`.
    ///
    /// The worker stops ticking and waits for each in-flight run until that
    /// run's own deadline, recording its outcome. If `deadline` passes first,
    /// the worker and its runs are aborted and record nothing. Returns
    /// whether the worker finished before `deadline`.
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
