//! Worker behaviour under a manual clock: ticking, missed-tick policy, stop
//! checks, timeouts, generation fencing, jitter and shutdown. Ported from
//! atm-cron's `tests/worker.rs` where the cases apply without ATM types.
//!
//! No test polls. A test waits on one of three signals: the manual clock's
//! park at the new time (the worker has settled), the job's entry or drop
//! counters, or the recording store's write log.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::clone_on_ref_ptr,
    clippy::unused_self,
    clippy::duration_suboptimal_units,
    clippy::unchecked_time_subtraction,
    reason = "test code: a failed expectation is the test failure"
)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sc_cron::{
    Clock, CronError, Generation, Job, ManualClock, MissedTickPolicy, NewSchedule, RunOutcome,
    RunTimeout, Schedule, ScheduleId, ScheduleSpec, ScheduleStore, Scheduler, SeqJitter,
    SqliteScheduleStore, StopDecision, Timestamp, WorkerHandle,
};
use serde_json::{Value, json};
use tokio::sync::watch;

const WAIT: Duration = Duration::from_secs(10);

/// Waits on `receiver` until `condition` holds, failing the test after ten seconds.
async fn wait_on<T>(receiver: &watch::Sender<T>, what: &str, condition: impl FnMut(&T) -> bool) {
    let mut receiver = receiver.subscribe();
    let reached = tokio::time::timeout(WAIT, receiver.wait_for(condition)).await;
    assert!(matches!(reached, Ok(Ok(_))), "timed out waiting for {what}");
}

/// What the test job does on one run.
enum Behavior {
    /// Return at once.
    Return(RunOutcome),
    /// Wait for the release signal, then return.
    Hold(RunOutcome),
    /// Never return.
    Hang,
    /// Panic with this message.
    Panic(&'static str),
}

/// Counts run futures dropped before they returned.
struct DropProbe<'a> {
    dropped: &'a watch::Sender<usize>,
    armed: bool,
}

impl Drop for DropProbe<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.dropped.send_modify(|count| *count += 1);
        }
    }
}

#[derive(Default)]
struct TestJob {
    behaviors: Mutex<VecDeque<Behavior>>,
    stops: Mutex<VecDeque<StopDecision>>,
    runs: Mutex<Vec<(Value, Instant)>>,
    stop_checks: AtomicUsize,
    dropped: watch::Sender<usize>,
    entered: watch::Sender<usize>,
    release: watch::Sender<bool>,
}

impl TestJob {
    fn then(&self, behavior: Behavior) {
        self.behaviors
            .lock()
            .expect("behaviors")
            .push_back(behavior);
    }

    fn stop_with(&self, decision: StopDecision) {
        self.stops.lock().expect("stops").push_back(decision);
    }

    fn runs(&self) -> usize {
        self.runs.lock().expect("runs").len()
    }

    fn stop_checks(&self) -> usize {
        self.stop_checks.load(Ordering::SeqCst)
    }

    fn deadlines(&self) -> Vec<Instant> {
        self.runs
            .lock()
            .expect("runs")
            .iter()
            .map(|(_, deadline)| *deadline)
            .collect()
    }

    fn release(&self) {
        self.release.send_replace(true);
    }

    async fn wait_entered(&self, count: usize) {
        wait_on(&self.entered, &format!("{count} runs"), |value| {
            *value >= count
        })
        .await;
    }

    async fn wait_dropped(&self, count: usize) {
        wait_on(&self.dropped, &format!("{count} dropped runs"), |value| {
            *value >= count
        })
        .await;
    }
}

#[async_trait]
impl Job for TestJob {
    async fn run(&self, payload: &Value, deadline: Instant) -> RunOutcome {
        self.runs
            .lock()
            .expect("runs")
            .push((payload.clone(), deadline));
        let behavior = self
            .behaviors
            .lock()
            .expect("behaviors")
            .pop_front()
            .unwrap_or(Behavior::Return(RunOutcome::Ok));
        self.entered.send_modify(|count| *count += 1);
        let mut probe = DropProbe {
            dropped: &self.dropped,
            armed: true,
        };
        let outcome = match behavior {
            Behavior::Return(outcome) => outcome,
            Behavior::Hold(outcome) => {
                let mut release = self.release.subscribe();
                // The sender lives in `self`; it cannot be dropped while we wait.
                let _ = release.wait_for(|released| *released).await;
                outcome
            }
            Behavior::Hang => std::future::pending().await,
            Behavior::Panic(message) => panic!("{message}"),
        };
        probe.armed = false;
        outcome
    }

    async fn should_stop(&self, _payload: &Value) -> StopDecision {
        self.stop_checks.fetch_add(1, Ordering::SeqCst);
        self.stops
            .lock()
            .expect("stops")
            .pop_front()
            .unwrap_or(StopDecision::Continue)
    }
}

/// One write the worker made, and whether the store applied it.
#[derive(Clone, Debug, PartialEq)]
enum Write {
    Run {
        id: ScheduleId,
        outcome: RunOutcome,
        changed: bool,
    },
    Next {
        id: ScheduleId,
        changed: bool,
    },
    Stop {
        id: ScheduleId,
        reason: String,
        changed: bool,
    },
}

/// A store that logs every write after it completes, for tests to wait on.
struct RecordingStore {
    inner: SqliteScheduleStore,
    writes: watch::Sender<Vec<Write>>,
    /// When set, the next `list_active` signals `listing` and blocks until
    /// the test sends on the paired sender: a reload held mid-flight.
    list_gate: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    listing: watch::Sender<usize>,
}

impl RecordingStore {
    fn new() -> Self {
        Self {
            inner: SqliteScheduleStore::open_in_memory().expect("store"),
            writes: watch::Sender::new(Vec::new()),
            list_gate: Mutex::new(None),
            listing: watch::Sender::new(0),
        }
    }

    /// Holds the next `list_active` until the returned sender sends.
    fn gate_next_list(&self) -> std::sync::mpsc::Sender<()> {
        let (open, gate) = std::sync::mpsc::channel();
        *self.list_gate.lock().expect("gate") = Some(gate);
        open
    }

    async fn wait_listing(&self, count: usize) {
        wait_on(&self.listing, "list_active entered", |value| {
            *value >= count
        })
        .await;
    }

    fn log(&self, write: Write) {
        self.writes.send_modify(|writes| writes.push(write));
    }

    async fn wait_write(&self, what: &str, condition: impl Fn(&Write) -> bool) {
        wait_on(&self.writes, what, |writes| writes.iter().any(&condition)).await;
    }

    async fn wait_recorded(&self, schedule: &Schedule, outcome: RunOutcome) {
        let id = schedule.id;
        self.wait_write(&format!("{outcome} recorded"), |write| {
            *write
                == Write::Run {
                    id,
                    outcome: outcome.clone(),
                    changed: true,
                }
        })
        .await;
    }

    async fn wait_stopped(&self, schedule: &Schedule) {
        let id = schedule.id;
        self.wait_write("schedule stopped", |write| {
            matches!(write, Write::Stop { id: stopped, changed: true, .. } if *stopped == id)
        })
        .await;
    }
}

impl ScheduleStore for RecordingStore {
    fn create(&self, schedule: &NewSchedule, created_at: Timestamp) -> Result<Schedule, CronError> {
        self.inner.create(schedule, created_at)
    }

    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError> {
        self.inner.get(id)
    }

    fn list_active(&self) -> Result<Vec<Schedule>, CronError> {
        let gate = self.list_gate.lock().expect("gate").take();
        let rows = self.inner.list_active();
        self.listing.send_modify(|count| *count += 1);
        if let Some(gate) = gate {
            // Runs on the blocking pool; a dropped sender also opens the gate.
            let _ = gate.recv();
        }
        rows
    }

    fn record_run(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        outcome: &RunOutcome,
    ) -> Result<bool, CronError> {
        let changed = self.inner.record_run(id, generation, at, outcome)?;
        self.log(Write::Run {
            id,
            outcome: outcome.clone(),
            changed,
        });
        Ok(changed)
    }

    fn set_next(
        &self,
        id: ScheduleId,
        generation: Generation,
        next_run_at: Timestamp,
    ) -> Result<bool, CronError> {
        let changed = self.inner.set_next(id, generation, next_run_at)?;
        self.log(Write::Next { id, changed });
        Ok(changed)
    }

    fn stop(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        reason: &str,
    ) -> Result<bool, CronError> {
        let changed = self.inner.stop(id, generation, at, reason)?;
        self.log(Write::Stop {
            id,
            reason: reason.to_owned(),
            changed,
        });
        Ok(changed)
    }
}

type TestScheduler = Scheduler<RecordingStore, ManualClock, SeqJitter>;

struct Fixture {
    store: Arc<RecordingStore>,
    clock: Arc<ManualClock>,
    jitter: Arc<SeqJitter>,
    scheduler: TestScheduler,
    job: Arc<TestJob>,
}

fn at(text: &str) -> DateTime<Utc> {
    text.parse().expect("timestamp")
}

fn ts(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

fn spec(name: &str, expr: &str, timeout_secs: u64) -> ScheduleSpec {
    ScheduleSpec::new(
        name,
        expr,
        Duration::from_secs(timeout_secs),
        json!({"name": name, "goal": {"ready": true}}),
    )
    .expect("spec")
}

impl Fixture {
    fn new() -> Self {
        Self::with_policy(MissedTickPolicy::Coalesce)
    }

    fn with_policy(policy: MissedTickPolicy) -> Self {
        let store = Arc::new(RecordingStore::new());
        let clock = Arc::new(ManualClock::new(at("2026-10-19T00:00:00Z")));
        let jitter = Arc::new(SeqJitter::default());
        let scheduler = Scheduler::new(store.clone(), clock.clone(), jitter.clone())
            .with_missed_tick_policy(policy);
        Self {
            store,
            clock,
            jitter,
            scheduler,
            job: Arc::new(TestJob::default()),
        }
    }

    /// A spec that runs every minute at :00 with a 20 s timeout.
    fn spec(&self, name: &str) -> ScheduleSpec {
        spec(name, "0 * * * * *", 20)
    }

    fn start(&self, spec: ScheduleSpec) -> Schedule {
        self.scheduler.start(spec).expect("start")
    }

    async fn worker(&self) -> WorkerHandle {
        let parks = self.clock.parks();
        let handle = self.scheduler.spawn_worker(self.job.clone());
        self.clock.wait_parks_above(parks).await;
        handle
    }

    async fn step(&self, seconds: i64) {
        self.clock
            .advance_settled(chrono::Duration::seconds(seconds))
            .await;
    }

    fn row(&self, schedule: &Schedule) -> Schedule {
        self.store
            .get(schedule.id)
            .expect("get")
            .expect("row exists")
    }

    /// Ends and restarts `spec` straight in the store, as another process
    /// would: this scheduler's worker gets no reload signal.
    fn restart_externally(
        &self,
        first: &Schedule,
        spec: ScheduleSpec,
        next_run_at: &str,
    ) -> Schedule {
        let now = Timestamp::from_datetime(self.clock.now());
        assert!(
            self.store
                .stop(first.id, first.generation, now, "external")
                .expect("stop")
        );
        self.store
            .create(
                &NewSchedule {
                    spec,
                    jitter: Duration::ZERO,
                    next_run_at: ts(next_run_at),
                },
                now,
            )
            .expect("restart")
    }
}

async fn shutdown(handle: WorkerHandle) {
    let stopped = handle.shutdown(tokio::time::Instant::now() + WAIT).await;
    assert!(stopped, "worker stopped before the deadline");
}

#[tokio::test]
async fn ticks_fire_only_when_the_manual_clock_reaches_them() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("tick"));
    assert_eq!(schedule.next_run_at, Some(ts("2026-10-19T00:01:00Z")));
    let worker = fixture.worker().await;
    fixture.step(59).await;
    assert_eq!(fixture.job.runs(), 0);
    fixture.step(1).await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
    let row = fixture.row(&schedule);
    assert_eq!(row.last_outcome, Some(RunOutcome::Ok));
    assert_eq!(row.last_run_at, Some(ts("2026-10-19T00:01:00Z")));
    assert_eq!(row.next_run_at, Some(ts("2026-10-19T00:02:00Z")));
    let (payload, _) = fixture.job.runs.lock().expect("runs")[0].clone();
    assert_eq!(payload, schedule.spec.payload);
}

#[tokio::test]
async fn missed_ticks_after_a_pause_are_replayed_as_one_run() {
    let fixture = Fixture::new();
    fixture.start(fixture.spec("missed"));
    let worker = fixture.worker().await;
    fixture.step(180 * 60).await;
    fixture.job.wait_entered(1).await;
    fixture.step(0).await;
    assert_eq!(fixture.job.runs(), 1);
    fixture.step(59).await;
    assert_eq!(fixture.job.runs(), 1);
    fixture.step(1).await;
    fixture.job.wait_entered(2).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 2);
}

#[tokio::test]
async fn skip_policy_drops_missed_ticks_after_a_pause() {
    let fixture = Fixture::with_policy(MissedTickPolicy::Skip);
    fixture.start(fixture.spec("missed"));
    let worker = fixture.worker().await;
    fixture.step(180 * 60).await;
    assert_eq!(fixture.job.runs(), 0);
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
}

#[tokio::test]
async fn ticks_missed_while_down_are_replayed_once_at_worker_start() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("down"));
    fixture.clock.set(at("2026-10-19T03:00:30Z"));
    let worker = fixture.worker().await;
    fixture.job.wait_entered(1).await;
    fixture.step(0).await;
    assert_eq!(fixture.job.runs(), 1);
    assert_eq!(
        fixture.row(&schedule).next_run_at,
        Some(ts("2026-10-19T03:01:00Z"))
    );
    fixture.step(30).await;
    fixture.job.wait_entered(2).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 2);
}

#[tokio::test]
async fn skip_policy_drops_ticks_missed_while_down() {
    let fixture = Fixture::with_policy(MissedTickPolicy::Skip);
    fixture.start(fixture.spec("down"));
    fixture.clock.set(at("2026-10-19T03:00:30Z"));
    let worker = fixture.worker().await;
    assert_eq!(fixture.job.runs(), 0);
    fixture.step(30).await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
}

#[tokio::test]
async fn a_late_but_not_missed_tick_runs_under_skip() {
    let fixture = Fixture::with_policy(MissedTickPolicy::Skip);
    fixture.start(fixture.spec("late"));
    fixture.clock.set(at("2026-10-19T00:01:30Z"));
    let worker = fixture.worker().await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
}

#[tokio::test]
async fn tick_while_in_flight_is_skipped() {
    let fixture = Fixture::new();
    fixture.start(spec("held", "0 * * * * *", 300));
    fixture.job.then(Behavior::Hold(RunOutcome::Ok));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;
    fixture.step(60).await;
    fixture.step(60).await;
    assert_eq!(fixture.job.runs(), 1);
    fixture.job.release();
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
    assert_eq!(fixture.job.stop_checks(), 1);
}

#[tokio::test]
async fn stop_check_runs_before_every_run_and_can_end_the_schedule() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("stoppable"));
    fixture.job.stop_with(StopDecision::Continue);
    fixture.job.stop_with(StopDecision::Continue);
    fixture
        .job
        .stop_with(StopDecision::Stop("phase closed".into()));
    let worker = fixture.worker().await;
    for expected in 1..=2 {
        fixture.step(60).await;
        fixture.job.wait_entered(expected).await;
        assert_eq!(fixture.job.stop_checks(), expected);
    }
    fixture.step(60).await;
    fixture.store.wait_stopped(&schedule).await;
    fixture.step(120).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 2);
    assert_eq!(fixture.job.stop_checks(), 3);
    let row = fixture.row(&schedule);
    assert_eq!(row.stop_reason.as_deref(), Some("phase closed"));
    assert_eq!(row.ended_at, Some(ts("2026-10-19T00:03:00Z")));
    assert_eq!(row.next_run_at, None);
    assert!(fixture.store.list_active().expect("list").is_empty());
}

#[tokio::test]
async fn timed_out_run_is_recorded_saw_its_deadline_and_does_not_delay_the_next_tick() {
    let fixture = Fixture::new();
    let schedule = fixture.start(spec("slow", "0 * * * * *", 30));
    fixture.job.then(Behavior::Hang);
    let worker = fixture.worker().await;
    let launched_at = fixture.clock.monotonic_now() + Duration::from_secs(60);

    fixture.step(60).await;
    fixture.job.wait_entered(1).await;
    assert_eq!(
        fixture.job.deadlines(),
        [launched_at + Duration::from_secs(30)]
    );
    fixture.step(29).await;
    assert_eq!(fixture.row(&schedule).last_outcome, None);
    fixture.step(1).await;
    let row = fixture.row(&schedule);
    assert_eq!(row.last_outcome, Some(RunOutcome::TimedOut));
    assert_eq!(row.last_run_at, Some(ts("2026-10-19T00:01:30Z")));
    fixture.job.wait_dropped(1).await;

    fixture.step(30).await;
    fixture.job.wait_entered(2).await;
    shutdown(worker).await;
    let row = fixture.row(&schedule);
    assert_eq!(row.last_outcome, Some(RunOutcome::Ok));
    assert_eq!(row.last_run_at, Some(ts("2026-10-19T00:02:00Z")));
}

#[tokio::test]
async fn a_wall_clock_jump_does_not_time_a_run_out_early() {
    let fixture = Fixture::new();
    let schedule = fixture.start(spec("jump", "0 * * * * *", 30));
    fixture.job.then(Behavior::Hold(RunOutcome::Ok));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;
    // Wall time leaps an hour; monotonic time does not move.
    let parks = fixture.clock.parks();
    fixture.clock.jump_wall(at("2026-10-19T01:01:00Z"));
    fixture
        .clock
        .wait_parked_at(parks, at("2026-10-19T01:01:00Z"))
        .await;
    assert_eq!(fixture.row(&schedule).last_outcome, None, "not timed out");
    fixture.job.release();
    fixture.store.wait_recorded(&schedule, RunOutcome::Ok).await;
    shutdown(worker).await;
}

#[tokio::test]
async fn a_run_from_an_older_generation_cannot_record() {
    let fixture = Fixture::new();
    let spec = fixture.spec("fenced");
    let first = fixture.start(spec.clone());
    fixture.job.then(Behavior::Hold(RunOutcome::Ok));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;

    let restarted = fixture.restart_externally(&first, spec, "2026-10-19T00:02:00Z");
    assert_eq!(restarted.id, first.id);
    assert_eq!(Some(restarted.generation), first.generation.next());

    fixture.job.release();
    let id = first.id;
    fixture
        .store
        .wait_write("fenced record", |write| {
            *write
                == Write::Run {
                    id,
                    outcome: RunOutcome::Ok,
                    changed: false,
                }
        })
        .await;
    let row = fixture.row(&first);
    assert_eq!(row.generation, restarted.generation);
    assert_eq!(row.last_outcome, None);
    assert_eq!(row.last_run_at, None);

    // The fenced write made the worker reload: the new generation ticks.
    fixture.step(60).await;
    fixture.job.wait_entered(2).await;
    shutdown(worker).await;
    let row = fixture.row(&first);
    assert_eq!(row.last_outcome, Some(RunOutcome::Ok));
    assert_eq!(row.last_run_at, Some(ts("2026-10-19T00:02:00Z")));
}

#[tokio::test]
async fn a_fenced_launch_reloads_at_once_and_runs_the_new_generation() {
    let fixture = Fixture::new();
    let spec = fixture.spec("fenced-launch");
    let first = fixture.start(spec.clone());
    let worker = fixture.worker().await;
    fixture.restart_externally(&first, spec, "2026-10-19T00:01:30Z");

    // The stale slot's tick is fenced at `set_next`, so it does not run...
    fixture.step(60).await;
    assert_eq!(fixture.job.runs(), 0);
    // ...and the reload is immediate: the new generation's 00:01:30 run fires.
    fixture.step(30).await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    let row = fixture.row(&first);
    assert_eq!(row.generation.get(), 2);
    assert_eq!(row.last_run_at, Some(ts("2026-10-19T00:01:30Z")));
}

#[tokio::test]
async fn restart_through_the_scheduler_cancels_the_old_generation_run() {
    let fixture = Fixture::new();
    let spec = fixture.spec("restart");
    let first = fixture.start(spec.clone());
    fixture.job.then(Behavior::Hold(RunOutcome::Ok));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;
    fixture
        .scheduler
        .stop(first.id, first.generation, "manual")
        .expect("stop");
    let restarted = fixture.start(spec);
    fixture.job.wait_dropped(1).await;
    fixture.job.release();
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
    let row = fixture.row(&restarted);
    assert_eq!(row.generation.get(), 2);
    assert_eq!(row.last_outcome, None);
}

#[tokio::test]
async fn stop_ends_ticks_and_a_stale_stop_is_fenced() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("ended"));
    let worker = fixture.worker().await;
    let parks = fixture.clock.parks();
    fixture
        .scheduler
        .stop(schedule.id, schedule.generation, "done")
        .expect("stop");
    fixture.clock.wait_parks_above(parks).await;
    fixture.step(120).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 0);
    let error = fixture
        .scheduler
        .stop(schedule.id, schedule.generation, "again")
        .expect_err("stale stop");
    assert!(
        matches!(error, CronError::Fenced { id, generation }
            if id == schedule.id && generation == schedule.generation),
        "{error:?}"
    );
    assert_eq!(error.code(), "FENCED");
    let row = fixture.row(&schedule);
    assert_eq!(row.stop_reason.as_deref(), Some("done"));
    assert_eq!(row.ended_at, Some(ts("2026-10-19T00:00:00Z")));
}

#[tokio::test]
async fn start_while_running_ticks_without_restart() {
    let fixture = Fixture::new();
    let worker = fixture.worker().await;
    let parks = fixture.clock.parks();
    fixture.start(fixture.spec("added"));
    fixture.clock.wait_parks_above(parks).await;
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
}

#[tokio::test]
async fn refused_and_failed_runs_are_recorded_and_the_schedule_stays_active() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("outcomes"));
    let refused = RunOutcome::Refused("quota".into());
    let failed = RunOutcome::Failed("bd exited 1".into());
    fixture.job.then(Behavior::Return(refused.clone()));
    fixture.job.then(Behavior::Return(failed.clone()));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture.store.wait_recorded(&schedule, refused).await;
    fixture.step(60).await;
    fixture.store.wait_recorded(&schedule, failed.clone()).await;
    shutdown(worker).await;
    let row = fixture.row(&schedule);
    assert!(row.is_active());
    assert_eq!(row.last_outcome, Some(failed));
}

#[tokio::test]
async fn panicking_run_records_failed_and_the_worker_keeps_ticking() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("panics"));
    fixture.job.then(Behavior::Panic("boom"));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture
        .store
        .wait_recorded(&schedule, RunOutcome::Failed("job panicked: boom".into()))
        .await;
    fixture.step(60).await;
    fixture.job.wait_entered(2).await;
    shutdown(worker).await;
    assert_eq!(fixture.row(&schedule).last_outcome, Some(RunOutcome::Ok));
}

#[tokio::test]
async fn last_finite_occurrence_ends_the_schedule() {
    let fixture = Fixture::new();
    let schedule = fixture.start(spec("finite", "0 0 1 20 10 * 2026", 20));
    let worker = fixture.worker().await;
    fixture.step(25 * 60 * 60).await;
    fixture.job.wait_entered(1).await;
    fixture.store.wait_stopped(&schedule).await;
    shutdown(worker).await;
    let row = fixture.row(&schedule);
    assert_eq!(row.last_outcome, Some(RunOutcome::Ok));
    assert_eq!(row.stop_reason.as_deref(), Some("no future occurrence"));
}

#[tokio::test]
async fn jitter_is_half_the_period_capped_at_thirty_minutes() {
    for (expr, bound) in [
        ("0 * * * * *", Duration::from_secs(30)),
        ("@hourly", Duration::from_secs(1800)),
        ("@daily", Duration::from_secs(1800)),
    ] {
        let fixture = Fixture::new();
        fixture.jitter.push(bound - Duration::from_secs(1));
        let schedule = fixture.start(spec("jitter", expr, 20));
        assert_eq!(schedule.jitter, bound - Duration::from_secs(1), "{expr}");
        assert_eq!(fixture.jitter.calls(), [bound], "{expr}");
    }
}

#[tokio::test]
async fn worker_fires_at_occurrence_plus_jitter() {
    let fixture = Fixture::new();
    fixture.clock.advance(chrono::Duration::minutes(30));
    fixture.jitter.push(Duration::from_secs(600));
    fixture.start(spec("jitter", "@hourly", 20));
    let worker = fixture.worker().await;
    fixture.step(1800).await;
    fixture.step(599).await;
    assert_eq!(fixture.job.runs(), 0);
    fixture.step(1).await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
}

#[tokio::test]
async fn restart_inside_the_jitter_window_still_fires_that_occurrence() {
    let fixture = Fixture::new();
    fixture.jitter.push(Duration::from_secs(600));
    fixture.start(spec("window", "@hourly", 20));
    shutdown(fixture.worker().await).await;
    fixture.clock.set(at("2026-10-19T00:05:00Z"));
    let worker = fixture.worker().await;
    fixture.step(299).await;
    assert_eq!(fixture.job.runs(), 0);
    fixture.step(1).await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
    assert_eq!(fixture.jitter.calls().len(), 1, "jitter is drawn once");
}

#[tokio::test]
#[should_panic(expected = "already running")]
async fn a_second_live_worker_is_refused() {
    let fixture = Fixture::new();
    let _first = fixture.scheduler.spawn_worker(fixture.job.clone());
    let _second = fixture.scheduler.spawn_worker(fixture.job.clone());
}

#[tokio::test]
async fn a_worker_can_be_spawned_again_after_shutdown() {
    let fixture = Fixture::new();
    shutdown(fixture.worker().await).await;
    shutdown(fixture.worker().await).await;
}

#[tokio::test]
async fn shutdown_drains_a_finishing_run_and_records_it() {
    let fixture = Fixture::new();
    let schedule = fixture.start(spec("drain", "0 * * * * *", 300));
    fixture.job.then(Behavior::Hold(RunOutcome::Ok));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;
    // Handshake: the worker parks once more only when it is in the drain,
    // waiting on the held run's deadline. Release the run after that.
    let parks = fixture.clock.parks();
    let shutting = tokio::spawn(worker.shutdown(tokio::time::Instant::now() + WAIT));
    fixture.clock.wait_parks_above(parks).await;
    assert_eq!(fixture.row(&schedule).last_outcome, None, "still draining");
    fixture.job.release();
    assert!(shutting.await.expect("shutdown task"), "drained in time");
    assert_eq!(fixture.row(&schedule).last_outcome, Some(RunOutcome::Ok));
}

#[tokio::test]
async fn shutdown_during_a_reload_still_drains_and_records_the_in_flight_run() {
    let fixture = Fixture::new();
    let held = fixture.start(spec("held", "0 * * * * *", 300));
    fixture.job.then(Behavior::Hold(RunOutcome::Ok));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;

    // A second start makes the worker reload; hold that reload in list_active.
    let listed = *fixture.store.listing.borrow();
    let open = fixture.store.gate_next_list();
    fixture.start(fixture.spec("second"));
    fixture.store.wait_listing(listed + 1).await;

    // Request shutdown while the reload is held: the first poll of
    // `shutdown` sends the stop signal before it awaits anything.
    let mut shutting = Box::pin(worker.shutdown(tokio::time::Instant::now() + WAIT));
    std::future::poll_fn(|context| {
        let _ = shutting.as_mut().poll(context);
        std::task::Poll::Ready(())
    })
    .await;

    // Let the reload finish. The held run's slot must survive the merge and
    // reach the drain, which parks on the run's deadline.
    let parks = fixture.clock.parks();
    open.send(()).expect("open gate");
    fixture.clock.wait_parks_above(parks).await;
    assert_eq!(fixture.job.runs(), 1);
    assert_eq!(fixture.row(&held).last_outcome, None, "still draining");

    fixture.job.release();
    assert!(shutting.await, "shutdown waited for the held run");
    assert_eq!(fixture.row(&held).last_outcome, Some(RunOutcome::Ok));
    assert_eq!(
        *fixture.job.dropped.borrow(),
        0,
        "the held run was not cancelled"
    );
}

#[tokio::test]
async fn shutdown_deadline_aborts_a_hung_run_and_records_nothing() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("hung"));
    fixture.job.then(Behavior::Hang);
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;
    let stopped = worker
        .shutdown(tokio::time::Instant::now() + Duration::from_millis(50))
        .await;
    assert!(!stopped, "the hung run outlives the shutdown deadline");
    fixture.job.wait_dropped(1).await;
    assert_eq!(fixture.row(&schedule).last_outcome, None);
}

#[tokio::test]
async fn a_new_schedule_starts_at_the_first_generation() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("first"));
    assert_eq!(schedule.generation, Generation::FIRST);
    assert_eq!(schedule.created_at, ts("2026-10-19T00:00:00Z"));
    assert_eq!(
        schedule.spec.timeout,
        RunTimeout::new(Duration::from_secs(20)).expect("timeout")
    );
}
