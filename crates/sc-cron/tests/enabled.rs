//! The `enabled` flag (REQ-CRON-0014): `set_enabled` flips only that column,
//! and a disabled schedule never fires. Time is a manual clock.

#![expect(
    clippy::clone_on_ref_ptr,
    reason = "test code: a failed expectation is the test failure"
)]

mod common;

use std::sync::Arc;

use common::{Fixture, ScriptedJob, StopScript, name, shutdown, ts};
use sc_cron::{CronError, HookTarget, RunOutcome, Schedule, ScheduleStore};

fn rust(job: &Arc<ScriptedJob>) -> HookTarget {
    HookTarget::Rust(job.clone())
}

#[test]
fn set_enabled_keeps_expression_payload_generation() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("keep"));
    assert!(schedule.enabled, "a new schedule is enabled");

    fixture
        .scheduler
        .set_enabled(&schedule.spec.name, false)
        .expect("disable");
    let disabled = fixture.row(&schedule);
    assert!(!disabled.enabled);
    // Every column but `enabled` is as it was: expression, payload, timeout,
    // generation, id, jitter, creation time, next run and run state.
    assert_eq!(
        disabled,
        Schedule {
            enabled: false,
            ..schedule.clone()
        }
    );

    fixture
        .scheduler
        .set_enabled(&schedule.spec.name, true)
        .expect("enable");
    assert_eq!(fixture.row(&schedule), schedule);
}

#[test]
fn set_enabled_leaves_run_state_alone() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("state"));
    let at = ts("2026-10-19T00:01:30Z");
    fixture
        .store
        .record_run(schedule.id, schedule.generation, at, &RunOutcome::Ok)
        .expect("record");
    fixture
        .store
        .set_next(schedule.id, schedule.generation, ts("2026-10-19T00:02:00Z"))
        .expect("next");
    let before = fixture.row(&schedule);
    assert_eq!(before.last_run_at, Some(at));
    fixture
        .scheduler
        .set_enabled(&schedule.spec.name, false)
        .expect("disable");
    assert_eq!(
        fixture.row(&schedule),
        Schedule {
            enabled: false,
            ..before
        }
    );
}

#[test]
fn set_enabled_names_a_schedule_that_is_not_active_as_not_found() {
    let fixture = Fixture::new();
    let missing = fixture.scheduler.set_enabled(&name("nobody"), false);
    assert!(
        matches!(missing, Err(CronError::NotFound(_))),
        "{missing:?}"
    );
    assert_eq!(missing.expect_err("missing").code(), "NOT_FOUND");

    let schedule = fixture.start(fixture.spec("ended"));
    fixture
        .scheduler
        .stop(schedule.id, schedule.generation, "by hand")
        .expect("stop");
    let ended = fixture.scheduler.set_enabled(&schedule.spec.name, false);
    assert!(matches!(ended, Err(CronError::NotFound(_))), "{ended:?}");
    assert!(fixture.row(&schedule).enabled, "an ended row is untouched");
}

#[test]
fn a_restarted_schedule_is_enabled() {
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("again"));
    fixture
        .scheduler
        .set_enabled(&schedule.spec.name, false)
        .expect("disable");
    fixture
        .scheduler
        .stop(schedule.id, schedule.generation, "by hand")
        .expect("stop");
    let restarted = fixture.start(fixture.spec("again"));
    assert_eq!(restarted.generation.get(), 2);
    assert!(restarted.enabled);
    assert!(fixture.row(&restarted).enabled);
}

#[tokio::test]
async fn a_disabled_row_never_fires_and_resumes_when_enabled() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    let schedule = fixture.start(fixture.spec("quiet"));
    fixture
        .scheduler
        .set_enabled_async(&schedule.spec.name, false)
        .await
        .expect("disable");
    let worker = fixture.worker(rust(&job)).await;

    // Several ticks pass; none runs, and the stored next run is not advanced.
    fixture.step(300).await;
    assert_eq!(job.runs(), 0);
    assert_eq!(job.stop_checks(), 0);
    assert_eq!(fixture.row(&schedule).next_run_at, schedule.next_run_at);

    fixture
        .scheduler
        .set_enabled_async(&schedule.spec.name, true)
        .await
        .expect("enable");
    // The ticks that came due while it was off count as missed, and are
    // replayed as one run under the default policy.
    job.wait_runs(1).await;
    fixture.store.wait_recorded(&schedule, RunOutcome::Ok).await;
    fixture.step(0).await;
    assert_eq!(job.runs(), 1);
    fixture.step(60).await;
    job.wait_runs(2).await;
    shutdown(worker).await;
}

#[tokio::test]
async fn disabling_while_the_worker_runs_stops_the_next_tick() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    let schedule = fixture.start(fixture.spec("live"));
    let worker = fixture.worker(rust(&job)).await;
    fixture.step(60).await;
    job.wait_runs(1).await;
    fixture.store.wait_recorded(&schedule, RunOutcome::Ok).await;

    fixture
        .scheduler
        .set_enabled_async(&schedule.spec.name, false)
        .await
        .expect("disable");
    fixture.step(180).await;
    shutdown(worker).await;
    assert_eq!(job.runs(), 1);
}

#[tokio::test]
async fn a_disable_made_elsewhere_stops_the_next_tick() {
    // Another scheduler or process writes the flag straight to the store, so
    // this worker gets no reload signal. The tick finds the schedule disabled
    // when it stores the next run, and does not launch.
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    let schedule = fixture.start(fixture.spec("elsewhere"));
    let worker = fixture.worker(rust(&job)).await;
    assert!(
        fixture
            .store
            .set_enabled(&schedule.spec.name, false)
            .expect("disable")
    );
    fixture.step(60).await;
    fixture.store.wait_next_refused(&schedule).await;
    fixture.step(120).await;
    shutdown(worker).await;
    assert_eq!(job.runs(), 0);
    assert_eq!(job.stop_checks(), 0);
}

#[tokio::test]
async fn disabling_cancels_a_run_in_flight_and_records_nothing() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    job.then_stop(StopScript::Hang);
    let schedule = fixture.start(fixture.spec("cancel"));
    let worker = fixture.worker(rust(&job)).await;
    fixture.step(60).await;
    job.wait_stop_checks(1).await;

    fixture
        .scheduler
        .set_enabled_async(&schedule.spec.name, false)
        .await
        .expect("disable");
    // The run's deadline passes, but its task is gone: nothing is recorded.
    fixture.step(60).await;
    shutdown(worker).await;
    let row = fixture.row(&schedule);
    assert_eq!(row.last_outcome, None);
    assert!(row.is_active() && !row.enabled);
}
