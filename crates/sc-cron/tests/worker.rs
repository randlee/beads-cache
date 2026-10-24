//! Worker behaviour under a manual clock: ticking, missed-tick policy, stop
//! checks, timeouts, generation fencing, jitter and shutdown. Ported from
//! atm-cron's `tests/worker.rs` where the cases apply without ATM types.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sc_cron::{
    Clock, CronError, Generation, Job, ManualClock, MissedTickPolicy, NewSchedule, RunOutcome,
    Schedule, ScheduleSpec, ScheduleStore, Scheduler, SeqJitter, SqliteScheduleStore, StopDecision,
    WorkerHandle,
};
use serde_json::{Value, json};
use tokio::sync::watch;

const WAIT: Duration = Duration::from_secs(10);

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
    dropped: &'a AtomicUsize,
    armed: bool,
}

impl Drop for DropProbe<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[derive(Default)]
struct TestJob {
    behaviors: Mutex<VecDeque<Behavior>>,
    stops: Mutex<VecDeque<StopDecision>>,
    runs: Mutex<Vec<(Value, Instant)>>,
    stop_checks: AtomicUsize,
    dropped: AtomicUsize,
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

    fn dropped(&self) -> usize {
        self.dropped.load(Ordering::SeqCst)
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
        let mut entered = self.entered.subscribe();
        let reached = tokio::time::timeout(WAIT, entered.wait_for(|value| *value >= count)).await;
        assert!(
            matches!(reached, Ok(Ok(_))),
            "expected {count} runs, saw {}",
            *self.entered.borrow()
        );
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

type TestScheduler = Scheduler<SqliteScheduleStore, ManualClock, SeqJitter>;

struct Fixture {
    store: Arc<SqliteScheduleStore>,
    clock: Arc<ManualClock>,
    jitter: Arc<SeqJitter>,
    scheduler: TestScheduler,
    job: Arc<TestJob>,
}

fn at(text: &str) -> DateTime<Utc> {
    text.parse().expect("timestamp")
}

impl Fixture {
    fn new() -> Self {
        Self::with_policy(MissedTickPolicy::Coalesce)
    }

    fn with_policy(policy: MissedTickPolicy) -> Self {
        let store = Arc::new(SqliteScheduleStore::open_in_memory().expect("store"));
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

    fn spec(&self, name: &str) -> ScheduleSpec {
        ScheduleSpec {
            name: name.into(),
            expr: "0 * * * * *".into(),
            timeout: Duration::from_secs(20),
            payload: json!({"name": name, "goal": {"ready": true}}),
        }
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
}

/// Polls `condition` until it holds, failing the test after ten seconds.
async fn eventually(what: &str, condition: impl Fn() -> bool) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while !condition() {
        assert!(tokio::time::Instant::now() < deadline, "timed out: {what}");
        tokio::time::sleep(Duration::from_millis(2)).await;
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
    assert_eq!(
        schedule.next_run_at.as_deref(),
        Some("2026-10-19T00:01:00.000000Z")
    );
    let worker = fixture.worker().await;
    fixture.step(59).await;
    assert_eq!(fixture.job.runs(), 0);
    fixture.step(1).await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 1);
    let row = fixture.row(&schedule);
    assert_eq!(row.last_outcome, Some(RunOutcome::Ok));
    assert_eq!(
        row.last_run_at.as_deref(),
        Some("2026-10-19T00:01:00.000000Z")
    );
    assert_eq!(
        row.next_run_at.as_deref(),
        Some("2026-10-19T00:02:00.000000Z")
    );
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
        fixture.row(&schedule).next_run_at.as_deref(),
        Some("2026-10-19T03:01:00.000000Z")
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
    let mut spec = fixture.spec("held");
    spec.timeout = Duration::from_secs(300);
    fixture.start(spec);
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
    eventually("schedule ended", || !fixture.row(&schedule).is_active()).await;
    fixture.step(120).await;
    shutdown(worker).await;
    assert_eq!(fixture.job.runs(), 2);
    assert_eq!(fixture.job.stop_checks(), 3);
    let row = fixture.row(&schedule);
    assert_eq!(row.stop_reason.as_deref(), Some("phase closed"));
    assert_eq!(row.ended_at.as_deref(), Some("2026-10-19T00:03:00.000000Z"));
    assert_eq!(row.next_run_at, None);
    assert!(fixture.store.list_active().expect("list").is_empty());
}

#[tokio::test]
async fn timed_out_run_is_recorded_saw_its_deadline_and_does_not_delay_the_next_tick() {
    let fixture = Fixture::new();
    let mut spec = fixture.spec("slow");
    spec.timeout = Duration::from_secs(30);
    let schedule = fixture.start(spec);
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
    assert_eq!(
        row.last_run_at.as_deref(),
        Some("2026-10-19T00:01:30.000000Z")
    );
    eventually("hung run dropped", || fixture.job.dropped() == 1).await;

    fixture.step(30).await;
    fixture.job.wait_entered(2).await;
    shutdown(worker).await;
    let row = fixture.row(&schedule);
    assert_eq!(row.last_outcome, Some(RunOutcome::Ok));
    assert_eq!(
        row.last_run_at.as_deref(),
        Some("2026-10-19T00:02:00.000000Z")
    );
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

    // Another process ends and restarts the schedule; this worker gets no signal.
    let now = "2026-10-19T00:01:00.000000Z";
    assert!(
        fixture
            .store
            .stop(first.id, first.generation, now, "external")
            .expect("stop")
    );
    let restarted = fixture
        .store
        .create(
            &NewSchedule {
                spec,
                jitter: Duration::ZERO,
                next_run_at: "2026-10-19T00:02:00.000000Z".into(),
            },
            now,
        )
        .expect("restart");
    assert_eq!(restarted.id, first.id);
    assert_eq!(Some(restarted.generation), first.generation.next());

    let parks = fixture.clock.parks();
    fixture.job.release();
    fixture.clock.wait_parks_above(parks).await;
    let row = fixture.row(&first);
    assert_eq!(row.generation, restarted.generation);
    assert_eq!(row.last_outcome, None);
    assert_eq!(row.last_run_at, None);
    assert!(
        !fixture
            .store
            .record_run(first.id, first.generation, now, &RunOutcome::Ok)
            .expect("stale record")
    );

    // The fenced write made the worker reload: the new generation ticks.
    fixture.step(60).await;
    fixture.job.wait_entered(2).await;
    shutdown(worker).await;
    let row = fixture.row(&first);
    assert_eq!(row.last_outcome, Some(RunOutcome::Ok));
    assert_eq!(
        row.last_run_at.as_deref(),
        Some("2026-10-19T00:02:00.000000Z")
    );
}

#[tokio::test]
async fn a_fenced_launch_reloads_at_once_and_runs_the_new_generation() {
    let fixture = Fixture::new();
    let spec = fixture.spec("fenced-launch");
    let first = fixture.start(spec.clone());
    let worker = fixture.worker().await;

    // Another process restarts the schedule before its tick; no signal here.
    let now = "2026-10-19T00:00:00.000000Z";
    fixture
        .store
        .stop(first.id, first.generation, now, "external")
        .expect("stop");
    fixture
        .store
        .create(
            &NewSchedule {
                spec,
                jitter: Duration::ZERO,
                next_run_at: "2026-10-19T00:01:30.000000Z".into(),
            },
            now,
        )
        .expect("restart");

    // The stale slot's tick is fenced at `set_next`, so it does not run...
    fixture.step(60).await;
    assert_eq!(fixture.job.runs(), 0);
    // ...and the reload is immediate: the new generation's 00:01:30 run fires.
    fixture.step(30).await;
    fixture.job.wait_entered(1).await;
    shutdown(worker).await;
    let row = fixture.row(&first);
    assert_eq!(row.generation.get(), 2);
    assert_eq!(
        row.last_run_at.as_deref(),
        Some("2026-10-19T00:01:30.000000Z")
    );
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
    eventually("old run dropped", || fixture.job.dropped() == 1).await;
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
    assert_eq!(
        fixture
            .scheduler
            .stop(schedule.id, schedule.generation, "again"),
        Err(CronError::Fenced)
    );
    let row = fixture.row(&schedule);
    assert_eq!(row.stop_reason.as_deref(), Some("done"));
    assert_eq!(row.ended_at.as_deref(), Some("2026-10-19T00:00:00.000000Z"));
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
    eventually("refused recorded", || {
        fixture.row(&schedule).last_outcome.as_ref() == Some(&refused)
    })
    .await;
    fixture.step(60).await;
    eventually("failed recorded", || {
        fixture.row(&schedule).last_outcome.as_ref() == Some(&failed)
    })
    .await;
    shutdown(worker).await;
    assert!(fixture.row(&schedule).is_active());
}

#[tokio::test]
async fn panicking_run_records_failed_and_the_worker_keeps_ticking() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("panics"));
    fixture.job.then(Behavior::Panic("boom"));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    let panicked = RunOutcome::Failed("job panicked: boom".into());
    eventually("panic recorded", || {
        fixture.row(&schedule).last_outcome.as_ref() == Some(&panicked)
    })
    .await;
    fixture.step(60).await;
    fixture.job.wait_entered(2).await;
    shutdown(worker).await;
    assert_eq!(fixture.row(&schedule).last_outcome, Some(RunOutcome::Ok));
}

#[tokio::test]
async fn last_finite_occurrence_ends_the_schedule() {
    let fixture = Fixture::new();
    let mut spec = fixture.spec("finite");
    spec.expr = "0 0 1 20 10 * 2026".into();
    let schedule = fixture.start(spec);
    let worker = fixture.worker().await;
    fixture.step(25 * 60 * 60).await;
    fixture.job.wait_entered(1).await;
    eventually("schedule ended", || !fixture.row(&schedule).is_active()).await;
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
        let mut spec = fixture.spec("jitter");
        spec.expr = expr.into();
        let schedule = fixture.start(spec);
        assert_eq!(schedule.jitter, bound - Duration::from_secs(1), "{expr}");
        assert_eq!(fixture.jitter.calls(), [bound], "{expr}");
    }
}

#[tokio::test]
async fn worker_fires_at_occurrence_plus_jitter() {
    let fixture = Fixture::new();
    fixture.clock.advance(chrono::Duration::minutes(30));
    fixture.jitter.push(Duration::from_secs(600));
    let mut spec = fixture.spec("jitter");
    spec.expr = "@hourly".into();
    fixture.start(spec);
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
    let mut spec = fixture.spec("window");
    spec.expr = "@hourly".into();
    fixture.start(spec);
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
    let mut spec = fixture.spec("drain");
    spec.timeout = Duration::from_secs(300);
    let schedule = fixture.start(spec);
    fixture.job.then(Behavior::Hold(RunOutcome::Ok));
    let worker = fixture.worker().await;
    fixture.step(60).await;
    fixture.job.wait_entered(1).await;
    let job = fixture.job.clone();
    let release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        job.release();
    });
    shutdown(worker).await;
    release.await.expect("release task");
    assert_eq!(fixture.row(&schedule).last_outcome, Some(RunOutcome::Ok));
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
    eventually("hung run dropped", || fixture.job.dropped() == 1).await;
    assert_eq!(fixture.row(&schedule).last_outcome, None);
}

#[tokio::test]
async fn a_new_schedule_starts_at_the_first_generation() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("first"));
    assert_eq!(schedule.generation, Generation::FIRST);
    assert_eq!(schedule.created_at, "2026-10-19T00:00:00.000000Z");
}
