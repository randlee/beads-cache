//! The worker task that ticks every active schedule.
//!
//! The worker keeps one slot per active schedule: its stored row, its parsed
//! expression, its next run time and at most one run in flight. Each loop
//! pass it (1) reloads the rows if signalled or a retry is due, (2) times out
//! runs past their deadline, (3) launches the runs that are due, then sleeps
//! on the clock until the next run time, the nearest run deadline, a reload
//! signal, a run finishing or shutdown. The worker is the only caller of
//! [`Clock::sleep_until`], so a manual clock's park count tells a test that
//! the worker has settled.
//!
//! Each run is its own tokio task: stop check, then the job, under one
//! deadline. A timed-out run is aborted at its deadline by the worker loop,
//! so the next tick is never delayed by it. A tick that comes due while the
//! previous run is still in flight is skipped and logged. Every store write
//! names `(id, generation)`; a fenced write makes the worker reload.

use chrono::{DateTime, Utc};
use std::future::poll_fn;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use tokio::sync::{oneshot, watch};
use tokio::task::{JoinError, JoinHandle};

use crate::{
    Clock, CronError, Job, MissedTickPolicy, RunOutcome, Schedule, ScheduleStore, StopDecision,
    expr, timestamp,
};

/// Stop reason recorded when an expression has no further occurrence.
pub(crate) const EXHAUSTED: &str = "no future occurrence";

/// What a run task reports back to the worker.
#[derive(Debug)]
enum Report {
    /// The stop check asked to end the schedule; the job did not run.
    Stop(String),
    /// The job ran with this outcome.
    Ran(RunOutcome),
}

/// One run in flight.
struct InFlight {
    handle: JoinHandle<Report>,
    deadline: DateTime<Utc>,
}

/// One active schedule as the worker tracks it.
struct Slot {
    schedule: Schedule,
    parsed: cron::Schedule,
    next: Option<DateTime<Utc>>,
    run: Option<InFlight>,
}

/// Dropping a run cancels it, wherever it is held; aborting a finished task
/// is a no-op. So a dropped slot, a reload that removes a slot and an
/// aborted worker all cancel their runs.
impl Drop for InFlight {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Backoff for a failed reload: 1 s doubling per failure, capped at 60 s.
#[derive(Debug, Default)]
struct ReloadRetry {
    failures: u32,
    due: Option<DateTime<Utc>>,
}

const RELOAD_RETRY_MAX_SECS: i64 = 60;

impl ReloadRetry {
    fn delay(failures: u32) -> chrono::Duration {
        let secs = 1i64
            .checked_shl(failures)
            .unwrap_or(RELOAD_RETRY_MAX_SECS)
            .min(RELOAD_RETRY_MAX_SECS);
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
    job: Arc<dyn Job>,
    policy: MissedTickPolicy,
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
        job: Arc<dyn Job>,
        policy: MissedTickPolicy,
    ) -> Self {
        Self {
            store,
            clock,
            job,
            policy,
            slots: Vec::new(),
            reload_wanted: true,
            retry: ReloadRetry::default(),
        }
    }

    pub(crate) async fn run(
        mut self,
        mut reload: watch::Receiver<u64>,
        mut shutdown: oneshot::Receiver<()>,
    ) {
        let mut reload_open = true;
        loop {
            if self.reload_wanted || self.retry.is_due(self.clock.now()) {
                self.reload().await;
            }
            let now = self.clock.now();
            self.expire(now).await;
            self.launch_due(now).await;
            // A fenced write asks for a reload: do it now, not at the next wake.
            let wake = if self.reload_wanted { now } else { self.wake() };
            let clock = Arc::clone(&self.clock);
            tokio::select! {
                biased;
                _ = &mut shutdown => break,
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

    /// The earliest instant the worker has something to do.
    fn wake(&self) -> DateTime<Utc> {
        let slots = self.slots.iter().flat_map(|slot| {
            slot.next
                .into_iter()
                .chain(slot.run.as_ref().map(|run| run.deadline))
        });
        slots
            .chain(self.retry.due)
            .min()
            .unwrap_or(DateTime::<Utc>::MAX_UTC)
    }

    async fn store_call<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&S) -> Result<T, CronError> + Send + 'static,
    ) -> Result<T, CronError> {
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || operation(store.as_ref()))
            .await
            .map_err(|error| CronError::Store(format!("store task failed: {error}")))?
    }

    async fn reload(&mut self) {
        self.reload_wanted = false;
        match self.store_call(|store| store.list_active()).await {
            Ok(rows) => {
                self.retry.succeeded();
                self.merge(rows).await;
            }
            Err(error) => {
                tracing::error!(subsystem = "sc_cron", action = "reload_failed", %error,
                    "schedule reload failed: {error}");
                self.retry.failed(self.clock.now());
            }
        }
    }

    /// Keeps the slots whose `(id, generation)` is unchanged, builds the rest.
    async fn merge(&mut self, rows: Vec<Schedule>) {
        let mut previous = std::mem::take(&mut self.slots);
        for schedule in rows.into_iter().filter(Schedule::is_active) {
            let kept = previous.iter().position(|slot| {
                slot.schedule.id == schedule.id && slot.schedule.generation == schedule.generation
            });
            if let Some(index) = kept {
                self.slots.push(previous.swap_remove(index));
            } else if let Some(slot) = self.new_slot(schedule).await {
                self.slots.push(slot);
            }
        }
        for slot in previous {
            if slot.run.is_some() {
                log(&slot.schedule, "cancelled", "schedule ended or restarted");
            }
            // Dropping the slot aborts its run, which records nothing.
        }
    }

    async fn new_slot(&mut self, schedule: Schedule) -> Option<Slot> {
        let parsed = match expr::parse_syntax(&schedule.spec.expr) {
            Ok(parsed) => parsed,
            Err(error) => {
                tracing::error!(subsystem = "sc_cron", action = "load_failed",
                    schedule = %schedule.id, %error, "stored schedule not loaded: {error}");
                return None;
            }
        };
        let stored = schedule
            .next_run_at
            .as_deref()
            .and_then(|text| timestamp::parse(text).ok());
        let next = match stored {
            Some(next) => Some(next),
            None => {
                let next = expr::next_run(&parsed, schedule.jitter, self.clock.now());
                if let Some(next) = next {
                    self.persist_next(&schedule, next).await;
                }
                next
            }
        };
        if next.is_none() {
            self.end(&schedule, EXHAUSTED).await;
            return None;
        }
        Some(Slot {
            schedule,
            parsed,
            next,
            run: None,
        })
    }

    /// Times out every run past its deadline; the run records `TimedOut`.
    async fn expire(&mut self, now: DateTime<Utc>) {
        let mut index = 0;
        while index < self.slots.len() {
            let expired = self.slots[index]
                .run
                .as_ref()
                .is_some_and(|run| run.deadline <= now);
            if !expired {
                index += 1;
                continue;
            }
            let Some(mut run) = self.slots[index].run.take() else {
                index += 1;
                continue;
            };
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
            let slot = &mut self.slots[index];
            let Some(due) = slot.next.filter(|next| *next <= now) else {
                index += 1;
                continue;
            };
            let jitter = slot.schedule.jitter;
            let missed =
                expr::next_run(&slot.parsed, jitter, due).is_some_and(|after| after <= now);
            let next = expr::next_run(&slot.parsed, jitter, now);
            slot.next = next;
            let schedule = slot.schedule.clone();
            if let Some(next) = next
                && !self.persist_next(&schedule, next).await
            {
                // Fenced: the schedule changed under us; the reload decides.
                index += 1;
                continue;
            }
            let slot = &mut self.slots[index];
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
                self.launch(index, now);
            }
            let slot = &self.slots[index];
            if slot.next.is_none() && slot.run.is_none() {
                let slot = self.slots.remove(index);
                self.end(&slot.schedule, EXHAUSTED).await;
            } else {
                index += 1;
            }
        }
    }

    fn launch(&mut self, index: usize, now: DateTime<Utc>) {
        let monotonic = self.clock.monotonic_now();
        let slot = &mut self.slots[index];
        let timeout = slot.schedule.spec.timeout;
        let deadline = monotonic.checked_add(timeout).unwrap_or(monotonic);
        let wall_deadline = chrono::Duration::from_std(timeout)
            .ok()
            .and_then(|timeout| now.checked_add_signed(timeout))
            .unwrap_or(DateTime::<Utc>::MAX_UTC);
        let job = Arc::clone(&self.job);
        let payload = slot.schedule.spec.payload.clone();
        let handle = tokio::spawn(async move {
            match job.should_stop(&payload).await {
                StopDecision::Stop(reason) => Report::Stop(reason),
                StopDecision::Continue => Report::Ran(job.run(&payload, deadline).await),
            }
        });
        log(&slot.schedule, "launched", "run started");
        slot.run = Some(InFlight {
            handle,
            deadline: wall_deadline,
        });
    }

    /// Records a finished run. Returns whether the slot was removed.
    async fn complete(&mut self, index: usize, result: Result<Report, JoinError>) -> bool {
        self.slots[index].run = None;
        let schedule = self.slots[index].schedule.clone();
        match result {
            Ok(Report::Stop(reason)) => {
                self.slots.remove(index);
                self.end(&schedule, &reason).await;
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
        if self.slots[index].next.is_none() {
            self.slots.remove(index);
            self.end(&schedule, EXHAUSTED).await;
            return true;
        }
        false
    }

    async fn record(&mut self, schedule: &Schedule, outcome: RunOutcome) {
        let (id, generation) = (schedule.id, schedule.generation);
        let at = timestamp::format(self.clock.now());
        let text = outcome.to_string();
        let result = self
            .store_call(move |store| store.record_run(id, generation, &at, &outcome))
            .await;
        match result {
            Ok(true) => log(schedule, "recorded", &text),
            Ok(false) => self.fenced(schedule, "record_run"),
            Err(error) => tracing::error!(subsystem = "sc_cron", action = "record_failed",
                schedule = %id, %error, "run outcome not recorded: {error}"),
        }
    }

    /// Stores the next run time; returns `false` only when fenced.
    async fn persist_next(&mut self, schedule: &Schedule, next: DateTime<Utc>) -> bool {
        let (id, generation) = (schedule.id, schedule.generation);
        let next = timestamp::format(next);
        match self
            .store_call(move |store| store.set_next(id, generation, &next))
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
                    schedule = %id, %error, "next run not stored: {error}");
                true
            }
        }
    }

    async fn end(&mut self, schedule: &Schedule, reason: &str) {
        let (id, generation) = (schedule.id, schedule.generation);
        let at = timestamp::format(self.clock.now());
        let owned = reason.to_owned();
        match self
            .store_call(move |store| store.stop(id, generation, &at, &owned))
            .await
        {
            Ok(true) => log(schedule, "stopped", reason),
            Ok(false) => self.fenced(schedule, "stop"),
            Err(error) => tracing::error!(subsystem = "sc_cron", action = "stop_failed",
                schedule = %id, %error, "schedule stop not stored: {error}"),
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
            let Some(mut run) = self.slots[index].run.take() else {
                index += 1;
                continue;
            };
            let clock = Arc::clone(&self.clock);
            let result = tokio::select! {
                result = &mut run.handle => result,
                () = clock.sleep_until(run.deadline) => {
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
