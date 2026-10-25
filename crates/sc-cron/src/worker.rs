//! The worker task that ticks every active schedule.
//!
//! The worker keeps one slot per active schedule: its stored row (whose
//! expression is already parsed), its next run time and at most one run in
//! flight. Each loop pass it (1) reloads the rows if signalled or a retry is
//! due, (2) times out runs past their deadline, (3) launches the runs that
//! are due, then sleeps on the clock until the next run time, the nearest
//! run deadline, a reload signal, a run finishing or shutdown. The worker is
//! the only caller of [`Clock::sleep_until`], so a manual clock's parks tell
//! a test that the worker has settled. Shutdown is checked between slots,
//! so a stop request is honoured before the next store call.
//!
//! Each run is its own tokio task: stop check, then the job, under one
//! deadline on the clock's monotonic timeline. A timed-out run is aborted at
//! its deadline by the worker loop, so the next tick is never delayed by it,
//! and a wall-clock jump cannot cut it short. A tick that comes due while
//! the previous run is still in flight is skipped and logged. Every store
//! write names `(id, generation)`; a fenced write makes the worker reload.
//!
//! A run is a Rust job or a Python module ([`HookTarget`], REQ-CRON-0013), and
//! its stop check answers continue, disable or delete (REQ-CRON-0005,
//! ADR-CRON-0007). A disabled schedule has no slot, so it never fires
//! (REQ-CRON-0014).

use std::future::poll_fn;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Instant;

use chrono::{DateTime, Utc};
use tokio::sync::{oneshot, watch};
use tokio::task::{JoinError, JoinHandle};

use crate::clock::{sleep_until_monotonic, wall_after};
use crate::hook::{Step, stop_step, tick_outcome};
use crate::{
    Clock, CronError, HookCall, HookEvent, HookTarget, MissedTickPolicy, RunOutcome, Schedule,
    ScheduleStore, StopDecision, StoreError, StoreErrorKind, Timestamp,
};

/// Stop reason recorded when an expression has no further occurrence.
pub(crate) const EXHAUSTED: &str = "no future occurrence";

/// What a run task reports back to the worker.
#[derive(Debug)]
enum Report {
    /// The stop check asked to end the schedule; the job did not run.
    Delete(String),
    /// The stop check asked to disable the schedule; the job did not run.
    Disable(String),
    /// The job ran with this outcome, or the run was skipped with one.
    Ran(RunOutcome),
}

/// One run in flight.
struct InFlight {
    handle: JoinHandle<Report>,
    /// On the clock's monotonic timeline.
    deadline: Instant,
}

/// Dropping a run cancels it, wherever it is held; aborting a finished task
/// is a no-op. So a dropped slot, a reload that removes a slot and an
/// aborted worker all cancel their runs.
impl Drop for InFlight {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// One active schedule as the worker tracks it.
struct Slot {
    schedule: Schedule,
    next: Option<DateTime<Utc>>,
    run: Option<InFlight>,
}

/// Backoff for a failed reload: 1 s doubling per failure, capped at 60 s.
#[derive(Debug, Default)]
struct ReloadRetry {
    failures: u32,
    due: Option<DateTime<Utc>>,
}

const RELOAD_RETRY_MAX_SECS: i64 = 60;

/// Largest shift used for the backoff: 2^6 = 64 s is already past the cap.
const RELOAD_RETRY_MAX_SHIFT: u32 = 6;

impl ReloadRetry {
    fn delay(failures: u32) -> chrono::Duration {
        let secs = (1i64 << failures.min(RELOAD_RETRY_MAX_SHIFT)).min(RELOAD_RETRY_MAX_SECS);
        chrono::Duration::seconds(secs)
    }

    fn failed(&mut self, now: DateTime<Utc>) {
        self.due = Some(now + Self::delay(self.failures));
        self.failures = self.failures.saturating_add(1);
    }

    fn succeeded(&mut self) {
        *self = Self::default();
    }

    fn is_due(&self, now: DateTime<Utc>) -> bool {
        self.due.is_some_and(|due| due <= now)
    }
}

pub(crate) struct Worker<S, C> {
    store: Arc<S>,
    clock: Arc<C>,
    target: HookTarget,
    policy: MissedTickPolicy,
    shutdown: oneshot::Receiver<()>,
    stopping: bool,
    slots: Vec<Slot>,
    reload_wanted: bool,
    retry: ReloadRetry,
}

impl<S, C> Worker<S, C>
where
    S: ScheduleStore + 'static,
    C: Clock + 'static,
{
    pub(crate) fn new(
        store: Arc<S>,
        clock: Arc<C>,
        target: HookTarget,
        policy: MissedTickPolicy,
        shutdown: oneshot::Receiver<()>,
    ) -> Self {
        Self {
            store,
            clock,
            target,
            policy,
            shutdown,
            stopping: false,
            slots: Vec::new(),
            reload_wanted: true,
            retry: ReloadRetry::default(),
        }
    }

    /// Returns whether shutdown was requested, without waiting.
    fn stop_requested(&mut self) -> bool {
        if !self.stopping {
            // A sent value and a dropped handle both mean stop.
            self.stopping = !matches!(
                self.shutdown.try_recv(),
                Err(oneshot::error::TryRecvError::Empty)
            );
        }
        self.stopping
    }

    pub(crate) async fn run(mut self, mut reload: watch::Receiver<u64>) {
        let mut reload_open = true;
        loop {
            if self.stop_requested() {
                break;
            }
            if self.reload_wanted || self.retry.is_due(self.clock.now()) {
                self.reload().await;
            }
            self.expire().await;
            let now = self.clock.now();
            self.launch_due(now).await;
            if self.stop_requested() {
                break;
            }
            // A fenced write asks for a reload: do it now, not at the next wake.
            let wake = if self.reload_wanted { now } else { self.wake() };
            let clock = Arc::clone(&self.clock);
            tokio::select! {
                biased;
                _ = &mut self.shutdown => break,
                changed = reload.changed(), if reload_open => {
                    // A dropped scheduler cannot signal again; keep ticking.
                    reload_open = changed.is_ok();
                    self.reload_wanted |= reload_open;
                }
                (index, result) = finished(&mut self.slots) => {
                    self.complete(index, result).await;
                }
                () = clock.sleep_until(wake) => {}
            }
        }
        self.drain().await;
    }

    /// The earliest wall-clock instant the worker has something to do.
    fn wake(&self) -> DateTime<Utc> {
        let (now, monotonic) = (self.clock.now(), self.clock.monotonic_now());
        let slots = self.slots.iter().flat_map(|slot| {
            let deadline = slot
                .run
                .as_ref()
                .map(|run| wall_after(now, run.deadline.saturating_duration_since(monotonic)));
            slot.next.into_iter().chain(deadline)
        });
        slots
            .chain(self.retry.due)
            .min()
            .unwrap_or(DateTime::<Utc>::MAX_UTC)
    }

    fn now_stamp(&self) -> Timestamp {
        Timestamp::from_datetime(self.clock.now())
    }

    async fn store_call<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&S) -> Result<T, CronError> + Send + 'static,
    ) -> Result<T, CronError> {
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || operation(store.as_ref()))
            .await
            .map_err(|error| {
                CronError::Store(
                    StoreError::new(StoreErrorKind::Other, "store task failed").with_source(error),
                )
            })?
    }

    async fn reload(&mut self) {
        self.reload_wanted = false;
        match self.store_call(ScheduleStore::list_active).await {
            Ok(rows) => {
                self.retry.succeeded();
                self.merge(rows).await;
            }
            Err(error) => {
                tracing::error!(subsystem = "sc_cron", action = "reload_failed",
                    code = error.code(), %error, "schedule reload failed: {error}");
                self.retry.failed(self.clock.now());
            }
        }
    }

    /// Keeps the slots whose `(id, generation)` is unchanged, builds the rest.
    ///
    /// Once shutdown is requested no new slot is built (that needs store
    /// calls), but the keep-matching pass still runs to the end: an unchanged
    /// slot's in-flight run must reach `drain` and be recorded, not be
    /// cancelled by dropping its slot.
    async fn merge(&mut self, rows: Vec<Schedule>) {
        let mut previous = std::mem::take(&mut self.slots);
        // A disabled schedule gets no slot, so it never fires and its stored
        // next run is left alone (REQ-CRON-0014).
        for schedule in rows
            .into_iter()
            .filter(|schedule| schedule.is_active() && schedule.enabled)
        {
            let kept = previous.iter().position(|slot| {
                slot.schedule.id == schedule.id && slot.schedule.generation == schedule.generation
            });
            if let Some(index) = kept {
                self.slots.push(previous.swap_remove(index));
            } else if !self.stop_requested()
                && let Some(slot) = self.new_slot(schedule).await
            {
                self.slots.push(slot);
            }
        }
        for slot in previous {
            if slot.run.is_some() {
                log(
                    &slot.schedule,
                    "cancelled",
                    "schedule ended, disabled or restarted",
                );
            }
            // Dropping the slot aborts its run, which records nothing.
        }
    }

    async fn new_slot(&mut self, schedule: Schedule) -> Option<Slot> {
        let next = if let Some(next) = schedule.next_run_at {
            Some(next.as_datetime())
        } else {
            // The port allows an active row without a next run (a store
            // that never had one); compute it. A corrupt stored value
            // never reaches here: it fails decoding in the store.
            let next = schedule
                .spec
                .expr
                .next_run(schedule.jitter, self.clock.now());
            if let Some(next) = next {
                self.persist_next(&schedule, next).await;
            }
            next
        };
        if next.is_none() {
            self.end(&schedule, EXHAUSTED).await;
            return None;
        }
        Some(Slot {
            schedule,
            next,
            run: None,
        })
    }

    /// Times out every run past its monotonic deadline; the run records `TimedOut`.
    async fn expire(&mut self) {
        let mut index = 0;
        while index < self.slots.len() {
            if self.stop_requested() {
                return;
            }
            let monotonic = self.clock.monotonic_now();
            let Some(slot) = self.slots.get_mut(index) else {
                return;
            };
            let expired = slot
                .run
                .as_ref()
                .is_some_and(|run| run.deadline <= monotonic);
            let run = if expired { slot.run.take() } else { None };
            let Some(mut run) = run else {
                index += 1;
                continue;
            };
            log(&slot.schedule, "timed_out", "run deadline passed");
            run.handle.abort();
            let result = match (&mut run.handle).await {
                Err(error) if error.is_cancelled() => Ok(Report::Ran(RunOutcome::TimedOut)),
                other => other,
            };
            if !self.complete(index, result).await {
                index += 1;
            }
        }
    }

    /// Launches every due slot, per the missed-tick policy.
    async fn launch_due(&mut self, now: DateTime<Utc>) {
        let mut index = 0;
        while index < self.slots.len() {
            if self.stop_requested() {
                return;
            }
            let Some(slot) = self.slots.get_mut(index) else {
                return;
            };
            let Some(due) = slot.next.filter(|next| *next <= now) else {
                index += 1;
                continue;
            };
            let (expr, jitter) = (&slot.schedule.spec.expr, slot.schedule.jitter);
            let missed = expr.next_run(jitter, due).is_some_and(|after| after <= now);
            let next = expr.next_run(jitter, now);
            slot.next = next;
            let schedule = slot.schedule.clone();
            if let Some(next) = next
                && !self.persist_next(&schedule, next).await
            {
                // Fenced: the schedule changed under us; the reload decides.
                index += 1;
                continue;
            }
            let Some(slot) = self.slots.get_mut(index) else {
                return;
            };
            if slot.run.is_some() {
                log(
                    &slot.schedule,
                    "skipped_in_flight",
                    "previous run still in flight",
                );
            } else if missed && self.policy == MissedTickPolicy::Skip {
                log(
                    &slot.schedule,
                    "missed_skipped",
                    "missed occurrences dropped",
                );
            } else {
                if missed {
                    log(
                        &slot.schedule,
                        "missed_replayed",
                        "missed occurrences run once",
                    );
                }
                self.launch(index);
            }
            let Some(slot) = self.slots.get(index) else {
                return;
            };
            if slot.next.is_none() && slot.run.is_none() {
                let slot = self.slots.remove(index);
                self.end(&slot.schedule, EXHAUSTED).await;
            } else {
                index += 1;
            }
        }
    }

    fn launch(&mut self, index: usize) {
        let monotonic = self.clock.monotonic_now();
        let Some(slot) = self.slots.get_mut(index) else {
            return;
        };
        let timeout = slot.schedule.spec.timeout;
        // RunTimeout is at most 24 h, so this cannot overflow in practice.
        let deadline = monotonic.checked_add(timeout.get()).unwrap_or(monotonic);
        let payload = slot.schedule.spec.payload.clone();
        let handle = match &self.target {
            HookTarget::Rust(job) => {
                let job = Arc::clone(job);
                tokio::spawn(async move {
                    match job.should_stop(&payload).await {
                        StopDecision::Continue => Report::Ran(job.run(&payload, deadline).await),
                        StopDecision::Disable(reason) => Report::Disable(reason),
                        StopDecision::Delete(reason) | StopDecision::Stop(reason) => {
                            Report::Delete(reason)
                        }
                    }
                })
            }
            HookTarget::Python(hook) => {
                let hook = hook.clone();
                let clock = Arc::clone(&self.clock);
                let name = slot.schedule.spec.name.clone();
                tokio::spawn(async move {
                    let call = |event| HookCall {
                        event,
                        schedule: &name,
                        payload: &payload,
                        timeout,
                        deadline,
                    };
                    match stop_step(&hook, clock.as_ref(), &call(HookEvent::Stop)).await {
                        Step::Run => Report::Ran(
                            tick_outcome(&hook, clock.as_ref(), &call(HookEvent::Tick)).await,
                        ),
                        Step::Skip(outcome) => Report::Ran(outcome),
                        Step::Disable(reason) => Report::Disable(reason),
                        Step::Delete(reason) => Report::Delete(reason),
                    }
                })
            }
        };
        log(&slot.schedule, "launched", "run started");
        slot.run = Some(InFlight { handle, deadline });
    }

    /// Records a finished run. Returns whether the slot was removed.
    async fn complete(&mut self, index: usize, result: Result<Report, JoinError>) -> bool {
        let Some(slot) = self.slots.get_mut(index) else {
            return false;
        };
        slot.run = None;
        let schedule = slot.schedule.clone();
        match result {
            Ok(Report::Delete(reason)) => {
                self.slots.remove(index);
                self.end(&schedule, &reason).await;
                return true;
            }
            Ok(Report::Disable(reason)) => {
                self.slots.remove(index);
                self.disable(&schedule, &reason).await;
                return true;
            }
            Ok(Report::Ran(outcome)) => self.record(&schedule, outcome).await,
            Err(error) if error.is_panic() => {
                let message = panic_message(error);
                self.record(
                    &schedule,
                    RunOutcome::Failed(format!("job panicked: {message}")),
                )
                .await;
            }
            Err(error) => log(&schedule, "cancelled", &error.to_string()),
        }
        if self
            .slots
            .get(index)
            .is_some_and(|slot| slot.next.is_none())
        {
            self.slots.remove(index);
            self.end(&schedule, EXHAUSTED).await;
            return true;
        }
        false
    }

    async fn record(&mut self, schedule: &Schedule, outcome: RunOutcome) {
        let (id, generation) = (schedule.id, schedule.generation);
        let at = self.now_stamp();
        let text = outcome.to_string();
        let result = self
            .store_call(move |store| store.record_run(id, generation, at, &outcome))
            .await;
        match result {
            Ok(true) => log(schedule, "recorded", &text),
            Ok(false) => self.fenced(schedule, "record_run"),
            Err(error) => tracing::error!(subsystem = "sc_cron", action = "record_failed",
                schedule = %id, code = error.code(), %error, "run outcome not recorded: {error}"),
        }
    }

    /// Stores the next run time; returns `false` only when fenced.
    async fn persist_next(&mut self, schedule: &Schedule, next: DateTime<Utc>) -> bool {
        let (id, generation) = (schedule.id, schedule.generation);
        let next = Timestamp::from_datetime(next);
        match self
            .store_call(move |store| store.set_next(id, generation, next))
            .await
        {
            Ok(true) => true,
            Ok(false) => {
                self.fenced(schedule, "set_next");
                false
            }
            Err(error) => {
                // The in-memory next run still drives this process.
                tracing::error!(subsystem = "sc_cron", action = "set_next_failed",
                    schedule = %id, code = error.code(), %error, "next run not stored: {error}");
                true
            }
        }
    }

    async fn end(&mut self, schedule: &Schedule, reason: &str) {
        let (id, generation) = (schedule.id, schedule.generation);
        let at = self.now_stamp();
        let owned = reason.to_owned();
        match self
            .store_call(move |store| store.stop(id, generation, at, &owned))
            .await
        {
            Ok(true) => log(schedule, "stopped", reason),
            Ok(false) => self.fenced(schedule, "stop"),
            Err(error) => tracing::error!(subsystem = "sc_cron", action = "stop_failed",
                schedule = %id, code = error.code(), %error, "schedule stop not stored: {error}"),
        }
    }

    /// Disables the schedule on its stop check's say-so; fenced like every write.
    async fn disable(&mut self, schedule: &Schedule, reason: &str) {
        let (id, generation) = (schedule.id, schedule.generation);
        let at = self.now_stamp();
        let owned = reason.to_owned();
        match self
            .store_call(move |store| store.disable(id, generation, at, &owned))
            .await
        {
            Ok(true) => log(schedule, "disabled", reason),
            Ok(false) => self.fenced(schedule, "disable"),
            Err(error) => tracing::error!(subsystem = "sc_cron", action = "disable_failed",
                schedule = %id, code = error.code(), %error, "schedule disable not stored: {error}"),
        }
    }

    fn fenced(&mut self, schedule: &Schedule, operation: &str) {
        tracing::warn!(subsystem = "sc_cron", action = "fenced", schedule = %schedule.id,
            generation = %schedule.generation, operation,
            "{operation} fenced: schedule ended or restarted");
        self.reload_wanted = true;
    }

    /// Waits for every in-flight run up to its own deadline, recording each.
    async fn drain(&mut self) {
        let mut index = 0;
        while index < self.slots.len() {
            let Some(slot) = self.slots.get_mut(index) else {
                return;
            };
            let Some(mut run) = slot.run.take() else {
                index += 1;
                continue;
            };
            let clock = Arc::clone(&self.clock);
            let deadline = run.deadline;
            let result = tokio::select! {
                result = &mut run.handle => result,
                () = sleep_until_monotonic(clock.as_ref(), deadline) => {
                    run.handle.abort();
                    match (&mut run.handle).await {
                        Err(error) if error.is_cancelled() => Ok(Report::Ran(RunOutcome::TimedOut)),
                        other => other,
                    }
                }
            };
            if !self.complete(index, result).await {
                index += 1;
            }
        }
    }
}

/// Resolves when any slot's run finishes, with that slot's index.
async fn finished(slots: &mut [Slot]) -> (usize, Result<Report, JoinError>) {
    poll_fn(|context| {
        for (index, slot) in slots.iter_mut().enumerate() {
            if let Some(run) = &mut slot.run
                && let Poll::Ready(result) = Pin::new(&mut run.handle).poll(context)
            {
                return Poll::Ready((index, result));
            }
        }
        Poll::Pending
    })
    .await
}

fn panic_message(error: JoinError) -> String {
    let payload = error.into_panic();
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic".to_owned())
}

fn log(schedule: &Schedule, action: &str, detail: &str) {
    tracing::info!(subsystem = "sc_cron", action, schedule = %schedule.id,
        generation = %schedule.generation, name = %schedule.spec.name, detail,
        "schedule {} {action}: {detail}", schedule.spec.name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_retry_doubles_per_failure_then_caps_at_one_minute() {
        let now = DateTime::<Utc>::UNIX_EPOCH;
        let mut retry = ReloadRetry::default();
        let mut delays = Vec::new();
        for _ in 0..9 {
            retry.failed(now);
            delays.push(retry.due.map(|due| (due - now).num_seconds()));
        }
        let expected = [1, 2, 4, 8, 16, 32, 60, 60, 60].map(Some);
        assert_eq!(delays, expected);
        assert!(!retry.is_due(now));
        assert!(retry.is_due(now + chrono::Duration::seconds(60)));
    }

    #[test]
    fn reload_retry_delay_never_overflows() {
        for failures in (62..=64).chain([u32::MAX - 1, u32::MAX]) {
            assert_eq!(
                ReloadRetry::delay(failures),
                chrono::Duration::seconds(60),
                "{failures}"
            );
        }
        let now = DateTime::<Utc>::UNIX_EPOCH;
        let mut retry = ReloadRetry {
            failures: u32::MAX,
            due: None,
        };
        retry.failed(now);
        assert_eq!(retry.failures, u32::MAX);
        assert_eq!(retry.due, Some(now + chrono::Duration::seconds(60)));
    }

    #[test]
    fn reload_retry_success_clears_the_backoff() {
        let now = DateTime::<Utc>::UNIX_EPOCH;
        let mut retry = ReloadRetry::default();
        retry.failed(now);
        retry.failed(now);
        retry.succeeded();
        assert!(retry.due.is_none());
        assert!(!retry.is_due(now + chrono::Duration::days(1)));
        retry.failed(now);
        assert_eq!(retry.due, Some(now + chrono::Duration::seconds(1)));
    }
}
