//! The reload poll (REQ-CRON-0014): a worker reloads the active schedules at
//! least once per interval whatever the rows are, so a schedule started,
//! restarted, enabled or stopped by another process is noticed with no signal.
//! Time is a manual clock; the "other process" is a second `Scheduler` over the
//! same store, whose signals reach only its own (absent) worker.

#![expect(
    clippy::clone_on_ref_ptr,
    clippy::expect_used,
    reason = "test code: a failed expectation is the test failure"
)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{Fixture, ScriptedJob, TestScheduler, shutdown};
use sc_cron::{DEFAULT_RELOAD_POLL, HookTarget, RunOutcome, ScheduleSpec, Scheduler, SeqJitter};

fn rust(job: &Arc<ScriptedJob>) -> HookTarget {
    HookTarget::Rust(job.clone())
}

fn secs(interval: Duration) -> i64 {
    i64::try_from(interval.as_secs()).expect("interval")
}

/// A second scheduler over the fixture's store and clock: another process.
fn elsewhere(fixture: &Fixture) -> TestScheduler {
    Scheduler::new(
        Arc::clone(&fixture.store),
        Arc::clone(&fixture.clock),
        Arc::new(SeqJitter::default()),
    )
}

/// A schedule that fires on 1 January only, so it never fires in a test.
fn unrelated(name: &str) -> ScheduleSpec {
    ScheduleSpec::new(
        name,
        "0 0 0 1 1 *",
        Duration::from_secs(20),
        serde_json::json!({}),
    )
    .expect("spec")
}

/// Starts "late" from another scheduler while the worker runs, then checks it
/// fires once the clock has moved one `interval` plus the time to its first
/// run. The worker gets no signal from the start.
async fn start_made_elsewhere_fires(fixture: &Fixture, interval: Duration) {
    let job = Arc::new(ScriptedJob::new());
    let worker = fixture.worker(rust(&job)).await;

    let late = elsewhere(fixture)
        .start(fixture.spec("late"))
        .expect("start");
    // The first run is at 00:01:00. The poll finds the row by the end of the
    // first interval; the rest of the minute passes with the slot in place.
    fixture.step(secs(interval)).await;
    assert_eq!(job.runs(), 0, "not due yet");
    fixture.step(60 - secs(interval)).await;
    job.wait_runs(1).await;
    fixture.store.wait_recorded(&late, RunOutcome::Ok).await;
    shutdown(worker).await;
}

#[tokio::test]
async fn a_start_made_elsewhere_is_noticed_by_a_worker_with_no_rows() {
    start_made_elsewhere_fires(&Fixture::new(), DEFAULT_RELOAD_POLL).await;
}

#[tokio::test]
async fn a_start_made_elsewhere_is_noticed_by_a_worker_with_only_unrelated_rows() {
    let fixture = Fixture::new();
    elsewhere(&fixture)
        .start(unrelated("january"))
        .expect("start");
    start_made_elsewhere_fires(&fixture, DEFAULT_RELOAD_POLL).await;
}

#[tokio::test]
async fn a_start_made_elsewhere_is_noticed_within_a_configured_poll() {
    let interval = Duration::from_secs(10);
    start_made_elsewhere_fires(&Fixture::with_reload_poll(interval), interval).await;
}

#[tokio::test]
async fn a_restart_made_elsewhere_is_noticed_within_the_poll() {
    // Another process stops the schedule and starts it again: generation 2,
    // with its first run at 00:01:00. The old slot (generation 1) has the same
    // time, but its writes are fenced; the poll swaps in the new slot first.
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    let first = fixture.start(fixture.spec("again"));
    let worker = fixture.worker(rust(&job)).await;
    let other = elsewhere(&fixture);
    other
        .stop(first.id, first.generation, "by hand")
        .expect("stop");
    let second = other.start(fixture.spec("again")).expect("start");
    assert_eq!(second.generation.get(), 2);

    fixture.step(secs(DEFAULT_RELOAD_POLL)).await;
    // The generation-1 slot is gone: its tick would be fenced, and the
    // generation-2 slot's record is accepted.
    fixture.step(60 - secs(DEFAULT_RELOAD_POLL)).await;
    job.wait_runs(1).await;
    fixture.store.wait_recorded(&second, RunOutcome::Ok).await;
    shutdown(worker).await;
    assert!(
        !fixture
            .store
            .writes()
            .iter()
            .any(|write| matches!(write, common::Write::Next { changed: false, .. })),
        "no write was fenced: the poll replaced the old slot before its tick"
    );
}

#[tokio::test]
async fn the_worker_reloads_at_least_once_per_interval_whatever_the_rows() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    elsewhere(&fixture)
        .start(unrelated("january"))
        .expect("start");
    let worker = fixture.worker(rust(&job)).await;
    for round in 1..=5 {
        let before = fixture.store.lists();
        fixture.step(secs(DEFAULT_RELOAD_POLL)).await;
        assert!(
            fixture.store.lists() > before,
            "no reload in interval {round}"
        );
    }
    shutdown(worker).await;
    assert_eq!(job.runs(), 0);
}
