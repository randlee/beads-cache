//! The Python hook runner (REQ-CRON-0013, ADR-CRON-0008) through the public
//! API, with a stub interpreter in place of Python and a manual clock in place
//! of time. The stub is a shell script, so these tests run on Unix; the
//! Windows job-object path is checked by compiling it.
//!
//! No test measures real time. The kill tests move the manual clock, or run
//! on tokio's paused clock, whose elapsed time is exactly what was slept.

#![cfg(unix)]
#![expect(
    clippy::expect_used,
    reason = "test code: a failed expectation is the test failure"
)]

mod common;

use std::time::Duration;

use common::{Fixture, Stub, at, name, read_pipe, shutdown, wait_pipe};
use sc_cron::{
    Clock, CronError, HookCall, HookEvent, HookStatus, HookTarget, MAX_HOOK_OUTPUT_BYTES,
    ManualClock, PythonHook, RunOutcome, RunTimeout,
};
use serde_json::{Value, json};

fn call<'a>(
    clock: &ManualClock,
    schedule: &'a sc_cron::ScheduleName,
    payload: &'a Value,
    timeout_secs: u64,
    left_secs: u64,
) -> HookCall<'a> {
    HookCall {
        event: HookEvent::Tick,
        schedule,
        payload,
        timeout: RunTimeout::new(Duration::from_secs(timeout_secs)).expect("timeout"),
        deadline: clock.monotonic_now() + Duration::from_secs(left_secs),
    }
}

#[tokio::test]
async fn a_python_hook_runs_isolated_from_a_temp_dir_with_one_json_object() {
    let stub = Stub::new();
    let clock = ManualClock::new(at("2026-10-25T00:00:00Z"));
    let schedule = name("sync");
    let payload = json!({"scope": "a", "n": [1, 2]});
    let output = stub
        .hook("my_pkg.cron_hook")
        .invoke(&clock, &call(&clock, &schedule, &payload, 20, 20))
        .await
        .expect("invoke");
    assert!(matches!(output.status, HookStatus::Exited(status) if status.success()));

    let calls = stub.calls();
    assert_eq!(calls.len(), 1);
    let recorded = &calls[0];
    // `<interpreter> -I -m <module>`, nothing else.
    assert_eq!(recorded.argv, ["-I", "-m", "my_pkg.cron_hook"]);
    // One JSON object on stdin; the deadline is the run's, as a UTC instant.
    let stdin: Value = serde_json::from_str(&recorded.stdin).expect("stdin is JSON");
    assert_eq!(
        stdin,
        json!({
            "protocol": "sc-cron.hook.v1",
            "event": "cron.tick",
            "schedule": "sync",
            "payload": {"scope": "a", "n": [1, 2]},
            "deadline": "2026-10-25T00:00:20.000000Z",
        })
    );
    // A fresh temporary directory, not the caller's, and gone afterwards.
    let here = std::env::current_dir().expect("cwd");
    assert_ne!(recorded.cwd, here.canonicalize().expect("canonical cwd"));
    assert!(
        !recorded
            .cwd
            .starts_with(stub.dir().canonicalize().expect("stub dir"))
    );
    assert!(
        recorded
            .cwd
            .file_name()
            .is_some_and(|dir| dir.to_string_lossy().starts_with("sc-cron-hook-"))
    );
    assert!(!recorded.cwd.exists(), "the temporary directory is removed");
}

#[tokio::test]
async fn the_event_name_is_cron_stop_for_the_stop_check() {
    let stub = Stub::new();
    let clock = ManualClock::new(at("2026-10-25T00:00:00Z"));
    let (schedule, payload) = (name("sync"), json!({}));
    let mut stop = call(&clock, &schedule, &payload, 20, 20);
    stop.event = HookEvent::Stop;
    stub.hook("m").invoke(&clock, &stop).await.expect("invoke");
    let stdin: Value = serde_json::from_str(&stub.calls()[0].stdin).expect("json");
    assert_eq!(stdin["event"], "cron.stop");
}

/// A hook killed at its timeout, or at the time left before the run's
/// deadline when that is sooner. Paused time makes the elapsed time exactly
/// the budget; the hook hangs, so only the kill can end it.
#[tokio::test(start_paused = true)]
async fn a_hook_is_killed_at_the_timeout_capped_by_the_deadline() {
    let clock = ManualClock::new(at("2026-10-25T00:00:00Z"));
    let (schedule, payload) = (name("sync"), json!({}));
    // (row timeout, time left before the run's deadline, expected kill time), in seconds.
    for (timeout, left, killed_at) in [(3600, 5, 5), (7, 3600, 7), (9, 9, 9)] {
        let stub = Stub::new();
        stub.mode("tick", "hang");
        let started = tokio::time::Instant::now();
        let output = stub
            .hook("m")
            .invoke(&clock, &call(&clock, &schedule, &payload, timeout, left))
            .await
            .expect("invoke");
        assert_eq!(output.status, HookStatus::TimedOut, "{timeout}/{left}");
        assert_eq!(
            started.elapsed(),
            Duration::from_secs(killed_at),
            "timeout {timeout} s, {left} s left"
        );
    }
}

/// Nothing is started once the deadline has passed, not even the interpreter:
/// one that does not exist would otherwise be a start failure.
#[tokio::test]
async fn a_hook_whose_deadline_has_passed_is_not_started() {
    let stub = Stub::new();
    let missing = PythonHook::new(stub.dir().join("no-such-python"), "m").expect("hook");
    let clock = ManualClock::new(at("2026-10-25T00:00:00Z"));
    let (schedule, payload) = (name("sync"), json!({}));
    let due = call(&clock, &schedule, &payload, 20, 10);
    clock.advance(chrono::Duration::seconds(10));
    let output = missing.invoke(&clock, &due).await.expect("not started");
    assert_eq!(output.status, HookStatus::TimedOut);
    // The same through the stub: no call was recorded.
    let output = stub.hook("m").invoke(&clock, &due).await.expect("invoke");
    assert_eq!(output.status, HookStatus::TimedOut);
    assert!(stub.calls().is_empty(), "nothing was started");
}

#[tokio::test]
async fn output_is_capped_and_a_flooding_hook_is_not_blocked() {
    let stub = Stub::new();
    stub.mode("tick", "flood");
    let clock = ManualClock::new(at("2026-10-25T00:00:00Z"));
    let (schedule, payload) = (name("sync"), json!({}));
    let output = stub
        .hook("m")
        .invoke(&clock, &call(&clock, &schedule, &payload, 20, 20))
        .await
        .expect("invoke");
    assert!(matches!(output.status, HookStatus::Exited(status) if status.success()));
    assert_eq!(output.stdout.len(), MAX_HOOK_OUTPUT_BYTES);
    assert_eq!(output.stderr.len(), MAX_HOOK_OUTPUT_BYTES);
    assert!(output.stdout.bytes().all(|byte| byte == b'x'));
    assert!(output.stderr.bytes().all(|byte| byte == b'y'));
    assert!(output.stdout_truncated && output.stderr_truncated);
}

#[tokio::test]
async fn output_under_the_cap_is_kept_whole_and_not_flagged() {
    let stub = Stub::new();
    stub.reply("tick", "hello");
    stub.exit("tick", 3);
    let clock = ManualClock::new(at("2026-10-25T00:00:00Z"));
    let (schedule, payload) = (name("sync"), json!({}));
    let output = stub
        .hook("m")
        .invoke(&clock, &call(&clock, &schedule, &payload, 20, 20))
        .await
        .expect("invoke");
    assert_eq!(output.stdout, "hello");
    assert!(!output.stdout_truncated && !output.stderr_truncated);
    assert!(matches!(output.status, HookStatus::Exited(status) if status.code() == Some(3)));
}

#[tokio::test]
async fn an_interpreter_that_cannot_start_is_a_hook_failure() {
    let stub = Stub::new();
    let missing = PythonHook::new(stub.dir().join("no-such-python"), "m").expect("hook");
    let clock = ManualClock::new(at("2026-10-25T00:00:00Z"));
    let (schedule, payload) = (name("sync"), json!({}));
    let error = missing
        .invoke(&clock, &call(&clock, &schedule, &payload, 20, 20))
        .await
        .expect_err("cannot start");
    assert!(matches!(error, CronError::HookFailed(_)), "{error}");
    assert_eq!(error.code(), "HOOK_FAILED");
}

/// The run's deadline kills the hook, and everything the hook started: the
/// worker aborts the run at the deadline on the manual clock, which drops the
/// supervisor, which kills the process group. The stub holds the write end of
/// a named pipe in the hook and in a grandchild; the pipe reaches its end only
/// when both are dead.
#[tokio::test]
async fn hanging_hook_is_killed() {
    let stub = Stub::new();
    stub.reply("stop", r#"{"action": "continue"}"#);
    stub.mode("tick", "fork");
    let (ready, life) = (stub.fifo("ready"), stub.fifo("life"));
    let (ready, life) = (read_pipe(ready), read_pipe(life));

    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("hang"));
    let worker = fixture
        .worker(HookTarget::Python(stub.hook("my_pkg.hook")))
        .await;
    fixture.step(60).await;
    wait_pipe(ready, "the hook to start its grandchild").await;

    // The run's deadline is 20 s after it launched.
    fixture.step(20).await;
    fixture
        .store
        .wait_recorded(&schedule, RunOutcome::TimedOut)
        .await;
    wait_pipe(life, "the hook and its grandchild to die").await;
    shutdown(worker).await;

    let row = fixture.row(&schedule);
    assert!(
        row.is_active() && row.enabled,
        "a timed-out run keeps the schedule"
    );
    // The stop check and the tick both ran, in that order.
    let events: Vec<String> = stub
        .calls()
        .iter()
        .map(|call| serde_json::from_str::<Value>(&call.stdin).expect("json")["event"].to_string())
        .collect();
    assert_eq!(events, [r#""cron.stop""#, r#""cron.tick""#]);
}

#[tokio::test]
async fn a_python_target_ticks_with_the_stop_check_first_and_one_deadline() {
    let stub = Stub::new();
    stub.reply("stop", r#"{"action": "continue", "reason": "go on"}"#);
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("ticks"));
    let worker = fixture.worker(HookTarget::Python(stub.hook("m"))).await;
    fixture.step(60).await;
    fixture.store.wait_recorded(&schedule, RunOutcome::Ok).await;
    shutdown(worker).await;

    let calls = stub.calls();
    let stdin = |n: usize| serde_json::from_str::<Value>(&calls[n].stdin).expect("json");
    assert_eq!(calls.len(), 2);
    assert_eq!(stdin(0)["event"], "cron.stop");
    assert_eq!(stdin(1)["event"], "cron.tick");
    // Both hooks are given the run's deadline, 20 s after the launch at 00:01:00.
    assert_eq!(stdin(0)["deadline"], "2026-10-19T00:01:20.000000Z");
    assert_eq!(stdin(0)["deadline"], stdin(1)["deadline"]);
    assert_eq!(stdin(1)["schedule"], "ticks");
    assert_eq!(stdin(1)["payload"], schedule.spec.payload);
}

#[tokio::test]
async fn a_failing_tick_hook_is_recorded_failed_and_the_schedule_goes_on() {
    let stub = Stub::new();
    stub.reply("stop", r#"{"action": "continue"}"#);
    stub.exit("tick", 3);
    let fixture = Fixture::new();
    let schedule = fixture.start(fixture.spec("fails"));
    let worker = fixture.worker(HookTarget::Python(stub.hook("m"))).await;
    fixture.step(60).await;
    fixture
        .store
        .wait_write("a failed run", |write| {
            matches!(write, common::Write::Run { outcome: RunOutcome::Failed(why), changed: true, .. }
                if why.contains("exit status: 3"))
        })
        .await;
    shutdown(worker).await;
    assert!(fixture.row(&schedule).is_active());
}
