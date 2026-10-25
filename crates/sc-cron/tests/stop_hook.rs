//! The stop hook: before every run it answers `continue`, `disable` or
//! `delete` with a reason (REQ-CRON-0005, ADR-CRON-0007). Each answer is
//! tested for a Rust hook and for a Python hook. A stop check that cannot
//! answer skips the run and leaves the schedule active and enabled.
//!
//! Time is a manual clock; the Python hook is a stub interpreter.

#![cfg_attr(
    unix,
    expect(
        clippy::expect_used,
        reason = "test code: a failed expectation is the test failure"
    )
)]
#![expect(
    clippy::clone_on_ref_ptr,
    reason = "test code: a failed expectation is the test failure"
)]

mod common;

use std::sync::Arc;

use common::{Fixture, ScriptedJob, StopScript, shutdown};
use sc_cron::{HookTarget, RunOutcome, StopDecision};

fn rust(job: &Arc<ScriptedJob>) -> HookTarget {
    HookTarget::Rust(job.clone())
}

#[tokio::test]
async fn stop_hook_continue() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    job.then_stop(StopScript::Decide(StopDecision::Continue));
    let schedule = fixture.start(fixture.spec("go"));
    let worker = fixture.worker(rust(&job)).await;
    fixture.step(60).await;
    job.wait_runs(1).await;
    fixture.store.wait_recorded(&schedule, RunOutcome::Ok).await;
    shutdown(worker).await;
    let row = fixture.row(&schedule);
    assert!(row.is_active() && row.enabled);
    assert_eq!(job.stop_checks(), 1);
}

#[tokio::test]
async fn stop_hook_disable() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    job.then_stop(StopScript::Decide(StopDecision::Disable("quiet".into())));
    let schedule = fixture.start(fixture.spec("quiet"));
    let worker = fixture.worker(rust(&job)).await;
    fixture.step(60).await;
    fixture.store.wait_disabled(&schedule, "quiet").await;

    // The run did not happen, the row is kept as it was and no longer fires.
    let row = fixture.row(&schedule);
    assert!(row.is_active(), "disable does not end the schedule");
    assert!(!row.enabled);
    assert_eq!(row.generation, schedule.generation);
    assert_eq!(row.spec, schedule.spec);
    assert_eq!(row.last_outcome, Some(RunOutcome::Refused("quiet".into())));
    fixture.step(180).await;
    assert_eq!(job.runs(), 0);
    assert_eq!(
        job.stop_checks(),
        1,
        "a disabled schedule is not even asked"
    );
    assert!(
        !fixture
            .store
            .writes()
            .iter()
            .any(|write| matches!(write, common::Write::Run { .. })),
        "a skipped run records no outcome of its own"
    );

    // Enabling it again resumes it.
    let name = schedule.spec.name.clone();
    fixture
        .scheduler
        .set_enabled_async(&name, true)
        .await
        .expect("enable");
    job.wait_runs(1).await;
    shutdown(worker).await;
}

#[tokio::test]
async fn stop_hook_delete() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    job.then_stop(StopScript::Decide(StopDecision::Delete("done".into())));
    let schedule = fixture.start(fixture.spec("done"));
    let worker = fixture.worker(rust(&job)).await;
    fixture.step(60).await;
    fixture.store.wait_stopped(&schedule, "done").await;
    fixture.step(180).await;
    shutdown(worker).await;

    let row = fixture.row(&schedule);
    assert!(!row.is_active());
    assert_eq!(row.stop_reason.as_deref(), Some("done"));
    assert_eq!(job.runs(), 0);
    assert_eq!(job.stop_checks(), 1);
    // The ended name can be started again, with a higher generation.
    let again = fixture.start(fixture.spec("done"));
    assert_eq!(again.id, schedule.id);
    assert_eq!(again.generation.get(), 2);
}

/// `StopDecision::Stop` is the earlier contract's one stopping variant.
#[tokio::test]
async fn the_earlier_stop_variant_still_ends_the_schedule() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    job.then_stop(StopScript::Decide(StopDecision::Stop(
        "phase closed".into(),
    )));
    let schedule = fixture.start(fixture.spec("legacy"));
    let worker = fixture.worker(rust(&job)).await;
    fixture.step(60).await;
    fixture.store.wait_stopped(&schedule, "phase closed").await;
    shutdown(worker).await;
    assert!(!fixture.row(&schedule).is_active());
    assert_eq!(job.runs(), 0);
}

/// A stop check that never answers uses up the run's timeout: the run is
/// skipped and recorded timed out, and the schedule goes on.
#[tokio::test]
async fn a_rust_stop_check_that_times_out_skips_the_run_and_keeps_the_row_active() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    job.then_stop(StopScript::Hang);
    let schedule = fixture.start(fixture.spec("slow"));
    let worker = fixture.worker(rust(&job)).await;
    fixture.step(60).await;
    fixture.step(20).await;
    fixture
        .store
        .wait_recorded(&schedule, RunOutcome::TimedOut)
        .await;
    let row = fixture.row(&schedule);
    assert!(row.is_active() && row.enabled);
    assert_eq!(job.runs(), 0);
    // The next tick asks again and, now answering, runs.
    fixture.step(40).await;
    job.wait_runs(1).await;
    shutdown(worker).await;
}

#[cfg(unix)]
mod python {
    use common::{Stub, shutdown};
    use sc_cron::{HookTarget, RunOutcome};

    use super::Fixture;
    use crate::common;

    /// The kinds of event the stub saw, in order.
    fn events(stub: &Stub) -> Vec<String> {
        stub.calls()
            .iter()
            .map(|call| {
                serde_json::from_str::<serde_json::Value>(&call.stdin)
                    .expect("json")
                    .get("event")
                    .and_then(serde_json::Value::as_str)
                    .expect("event")
                    .to_owned()
            })
            .collect()
    }

    async fn failed_stop_check(fixture: &Fixture, schedule: &sc_cron::Schedule, needle: &str) {
        let id = schedule.id;
        fixture
            .store
            .wait_write("a failed stop check", |write| {
                matches!(write, common::Write::Run { id: run, outcome: RunOutcome::Failed(why), changed: true }
                    if *run == id && why.starts_with("stop check: ") && why.contains(needle))
            })
            .await;
    }

    #[tokio::test]
    async fn python_stop_hook_continue() {
        let stub = Stub::new();
        stub.reply("stop", r#"{"action": "continue"}"#);
        let fixture = Fixture::new();
        let schedule = fixture.start(fixture.spec("go"));
        let worker = fixture.worker(HookTarget::Python(stub.hook("m"))).await;
        fixture.step(60).await;
        fixture.store.wait_recorded(&schedule, RunOutcome::Ok).await;
        shutdown(worker).await;
        assert_eq!(events(&stub), ["cron.stop", "cron.tick"]);
        let row = fixture.row(&schedule);
        assert!(row.is_active() && row.enabled);
    }

    #[tokio::test]
    async fn python_stop_hook_disable() {
        let stub = Stub::new();
        stub.reply("stop", r#"{"action": "disable", "reason": "quiet"}"#);
        let fixture = Fixture::new();
        let schedule = fixture.start(fixture.spec("quiet"));
        let worker = fixture.worker(HookTarget::Python(stub.hook("m"))).await;
        fixture.step(60).await;
        fixture.store.wait_disabled(&schedule, "quiet").await;
        fixture.step(180).await;
        shutdown(worker).await;

        let row = fixture.row(&schedule);
        assert!(row.is_active() && !row.enabled);
        assert_eq!(row.spec, schedule.spec);
        assert_eq!(row.last_outcome, Some(RunOutcome::Refused("quiet".into())));
        assert_eq!(events(&stub), ["cron.stop"], "no tick, and no more checks");
    }

    #[tokio::test]
    async fn python_stop_hook_delete() {
        let stub = Stub::new();
        stub.reply("stop", r#"{"action": "delete", "reason": "done"}"#);
        let fixture = Fixture::new();
        let schedule = fixture.start(fixture.spec("done"));
        let worker = fixture.worker(HookTarget::Python(stub.hook("m"))).await;
        fixture.step(60).await;
        fixture.store.wait_stopped(&schedule, "done").await;
        fixture.step(180).await;
        shutdown(worker).await;

        let row = fixture.row(&schedule);
        assert!(!row.is_active());
        assert_eq!(row.stop_reason.as_deref(), Some("done"));
        assert_eq!(events(&stub), ["cron.stop"]);
    }

    #[tokio::test]
    async fn a_failed_python_stop_check_skips_the_run_and_keeps_the_row_active() {
        let stub = Stub::new();
        stub.reply("stop", r#"{"action": "delete", "reason": "ignored"}"#);
        stub.exit("stop", 2);
        let fixture = Fixture::new();
        let schedule = fixture.start(fixture.spec("fails"));
        let worker = fixture.worker(HookTarget::Python(stub.hook("m"))).await;
        fixture.step(60).await;
        failed_stop_check(&fixture, &schedule, "exit status: 2").await;
        shutdown(worker).await;
        // A non-zero exit is no decision, whatever it printed.
        let row = fixture.row(&schedule);
        assert!(row.is_active() && row.enabled);
        assert_eq!(events(&stub), ["cron.stop"]);
    }

    #[tokio::test]
    async fn a_malformed_python_stop_reply_skips_the_run_and_keeps_the_row_active() {
        for (reply, needle) in [
            (r#"{"action": "stop", "reason": "x"}"#, "unknown action"),
            (r#"{"action": "delete"}"#, "needs a \"reason\""),
            ("{not json", "not JSON"),
            ("", "not JSON"),
        ] {
            let stub = Stub::new();
            stub.reply("stop", reply);
            let fixture = Fixture::new();
            let schedule = fixture.start(fixture.spec("garbled"));
            let worker = fixture.worker(HookTarget::Python(stub.hook("m"))).await;
            fixture.step(60).await;
            failed_stop_check(&fixture, &schedule, needle).await;
            shutdown(worker).await;
            let row = fixture.row(&schedule);
            assert!(row.is_active() && row.enabled, "{reply:?}");
            assert_eq!(
                events(&stub),
                ["cron.stop"],
                "{reply:?}: the tick did not run"
            );
        }
    }

    /// A stop hook that hangs is killed with the run, at the deadline.
    #[tokio::test]
    async fn a_python_stop_check_that_times_out_skips_the_run_and_keeps_the_row_active() {
        let stub = Stub::new();
        stub.mode("stop", "hang");
        let ready = common::read_pipe(stub.fifo("ready"));
        let fixture = Fixture::new();
        let schedule = fixture.start(fixture.spec("slow"));
        let worker = fixture.worker(HookTarget::Python(stub.hook("m"))).await;
        fixture.step(60).await;
        common::wait_pipe(ready, "the stop hook to start").await;
        fixture.step(20).await;
        fixture
            .store
            .wait_recorded(&schedule, RunOutcome::TimedOut)
            .await;
        shutdown(worker).await;
        let row = fixture.row(&schedule);
        assert!(row.is_active() && row.enabled);
        assert_eq!(events(&stub), ["cron.stop"]);
    }
}

/// After a planted store failure the worker must rebuild the slot: steps the
/// clock one second at a time until the worker has reloaded the rows again.
#[expect(
    clippy::panic,
    reason = "test helper: a failed expectation is the test failure"
)]
async fn step_until_reloaded(fixture: &Fixture, lists_before: usize) {
    for _ in 0..10 {
        if fixture.store.lists() > lists_before {
            return;
        }
        fixture.step(1).await;
    }
    panic!("the worker never reloaded after the failed store call");
}

#[tokio::test]
async fn a_failed_disable_is_retried_and_the_schedule_keeps_firing() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    job.then_stop(StopScript::Decide(StopDecision::Disable("quiet".into())));
    fixture.store.fail_next_disable();
    let schedule = fixture.start(fixture.spec("flaky"));
    let worker = fixture.worker(rust(&job)).await;
    let lists = fixture.store.lists();
    fixture.step(60).await;
    fixture.store.wait_failures(1).await;
    step_until_reloaded(&fixture, lists).await;

    // The row is still enabled, so the rebuilt slot fires at the next tick.
    assert!(fixture.row(&schedule).enabled);
    fixture.step(60).await;
    job.wait_runs(1).await;
    shutdown(worker).await;
    assert_eq!(job.stop_checks(), 2);
    assert!(fixture.row(&schedule).enabled);
}

#[tokio::test]
async fn a_failed_delete_is_retried_and_the_schedule_keeps_firing() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    job.then_stop(StopScript::Decide(StopDecision::Delete("done".into())));
    fixture.store.fail_next_stop();
    let schedule = fixture.start(fixture.spec("flaky-end"));
    let worker = fixture.worker(rust(&job)).await;
    let lists = fixture.store.lists();
    fixture.step(60).await;
    fixture.store.wait_failures(1).await;
    step_until_reloaded(&fixture, lists).await;

    assert!(fixture.row(&schedule).is_active());
    fixture.step(60).await;
    job.wait_runs(1).await;
    shutdown(worker).await;
    assert_eq!(job.stop_checks(), 2);
    assert!(fixture.row(&schedule).is_active());
}

/// A schedule whose only occurrence is 00:01:00 on the fixture's first day.
fn once(name: &str) -> sc_cron::ScheduleSpec {
    sc_cron::ScheduleSpec::new(
        name,
        "0 1 0 19 10 * 2026",
        std::time::Duration::from_secs(20),
        serde_json::json!({}),
    )
    .expect("spec")
}

const EXHAUSTED: &str = "no future occurrence";

#[tokio::test]
async fn a_failed_stop_of_an_exhausted_schedule_never_runs_its_last_occurrence_again() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    fixture.store.fail_next_stop();
    let schedule = fixture.start(once("once"));
    let worker = fixture.worker(rust(&job)).await;
    fixture.step(60).await;
    job.wait_runs(1).await;
    fixture.store.wait_failures(1).await;

    // Time passes through the backoff retry and several poll intervals.
    for _ in 0..8 {
        fixture.step(15).await;
    }
    fixture.store.wait_stopped(&schedule, EXHAUSTED).await;
    for _ in 0..4 {
        fixture.step(30).await;
    }
    shutdown(worker).await;
    assert_eq!(job.runs(), 1, "the last occurrence ran exactly once");
    assert!(!fixture.row(&schedule).is_active());
}

#[tokio::test]
async fn a_stop_of_an_exhausted_schedule_that_keeps_failing_backs_off_and_runs_it_once() {
    let fixture = Fixture::new();
    let job = Arc::new(ScriptedJob::new());
    fixture.store.fail_every_stop(true);
    let schedule = fixture.start(once("stuck"));
    let worker = fixture.worker(rust(&job)).await;
    fixture.step(60).await;
    job.wait_runs(1).await;
    fixture.store.wait_failures(1).await;

    for _ in 0..100 {
        fixture.step(1).await;
    }
    // Attempts at +0, 1, 3, 7, 15, 31 and 63 s: 1 s, 2 s ... doubling. A retry
    // that reset to one second would have tried about a hundred times.
    let attempts = fixture.store.failure_count();
    assert!(
        (6..=10).contains(&attempts),
        "{attempts} stop attempts in 100 s"
    );
    assert_eq!(job.runs(), 1, "the last occurrence did not run again");
    assert!(fixture.row(&schedule).is_active(), "the stop never landed");

    // The store recovers: the pending stop lands within the 60 s cap.
    fixture.store.fail_every_stop(false);
    for _ in 0..7 {
        fixture.step(10).await;
    }
    fixture.store.wait_stopped(&schedule, EXHAUSTED).await;
    shutdown(worker).await;
    assert_eq!(job.runs(), 1);
    assert!(!fixture.row(&schedule).is_active());
}
