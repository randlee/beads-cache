//! `SyncJob`'s threading (ADR-CORE-0001; REQ-BDC-0703): the cache in an
//! `Arc<Mutex<_>>` under `spawn_blocking`, a busy tick skipped, a poisoned
//! mutex recovered and reported.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use bd_cache_sync::{SyncJob, TICK_BUDGET, TickOutcome, TickRun};
use common::World;
use sc_cron::{Job, RunOutcome, StopDecision};

type TestJob = SyncJob<
    bd_cache_model::testing::FakeStore,
    bd_cache_model::testing::FakeDrainLock,
    common::blocking::Controlled,
>;

fn job(w: &World) -> TestJob {
    SyncJob::new(
        w.cache(),
        &w.config(),
        Arc::clone(&w.sender) as Arc<dyn bd_cache_model::EscalationSender>,
    )
    .unwrap()
}

fn done(run: TickRun) -> bd_cache_sync::TickReply {
    match run {
        TickRun::Done(reply) => *reply,
        other => panic!("expected a finished tick, got {other:?}"),
    }
}

/// A second tick while a blocking sync is still running is skipped and
/// reported busy; once the sync ends the next tick runs.
#[test]
fn a_second_tick_while_a_blocking_sync_is_running_is_skipped_and_reported_busy() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let job = job(&w);
    w.switch.gate.arm();
    let running = {
        let job = job.clone();
        let end = w.after(TICK_BUDGET);
        thread::spawn(move || job.run_blocking(end))
    };
    w.switch.gate.wait_entered(1);

    let second = done(job.run_blocking(w.after(TICK_BUDGET)));
    assert!(second.busy, "{second:?}");
    assert_eq!(second.outcome, TickOutcome::Refused);
    assert!(second.synced.is_empty() && second.drain.is_none());
    // The skipped tick made no bd call of its own.
    assert_eq!(
        w.bd.calls()
            .iter()
            .filter(|c| c.op() == sc_beads::testing::FakeOp::Fingerprint)
            .count(),
        1,
        "only the running tick's backend check"
    );

    w.switch.gate.open();
    let first = done(running.join().unwrap());
    assert_eq!(first.outcome, TickOutcome::PassThrough, "{first:?}");
    assert!(!first.busy);
    assert_eq!(done(job.run_blocking(w.after(TICK_BUDGET))).synced.len(), 1);
}

/// The scheduler drops a run that times out, but never its blocking thread:
/// the next tick finds the mutex held, is skipped, and `should_stop` keeps going.
#[tokio::test(flavor = "multi_thread")]
async fn a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let job = job(&w);
    w.switch.gate.arm();
    let payload = serde_json::json!({});
    let deadline = std::time::Instant::now() + Duration::from_secs(25);

    // The run times out (the scheduler would record TimedOut and drop it).
    let timed_out =
        tokio::time::timeout(Duration::from_millis(300), job.run(&payload, deadline)).await;
    assert!(timed_out.is_err(), "the run is blocked in bd");
    w.switch.gate.wait_entered(1);

    // The next scheduled run is skipped...
    let next = job.run(&payload, deadline).await;
    assert!(
        matches!(&next, RunOutcome::Refused(why) if why.contains("busy")),
        "{next:?}"
    );
    // ...and the stop check never stops under a running task.
    assert_eq!(job.should_stop(&payload).await, StopDecision::Continue);

    w.switch.gate.open();
    // The left-over task ends; ticks run again. (Wait for it via the mutex.)
    job.with_cache(|_| ());
    assert_eq!(job.run(&payload, deadline).await, RunOutcome::Ok);
}

/// With nothing to do the stop check stops, unless a task holds the cache.
#[test]
fn should_stop_returns_continue_when_busy() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.close_in_bd("rx");
    let job = job(&w);
    assert!(matches!(job.should_stop_blocking(), StopDecision::Stop(_)));

    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let holder = {
        let job = job.clone();
        thread::spawn(move || {
            job.with_cache(|_| {
                held_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            });
        })
    };
    held_rx.recv().unwrap();
    assert_eq!(job.should_stop_blocking(), StopDecision::Continue);
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    assert!(matches!(job.should_stop_blocking(), StopDecision::Stop(_)));
}

/// A poisoned mutex is recovered, reported once in the tick reply, and cleared.
#[test]
fn a_poisoned_mutex_is_recovered_and_reported_in_the_tick_reply() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let job = job(&w);
    let poisoner = {
        let job = job.clone();
        thread::spawn(move || job.with_cache(|_| panic!("a tick died")))
    };
    assert!(poisoner.join().is_err());

    let first = done(job.run_blocking(w.after(TICK_BUDGET)));
    assert!(first.poisoned, "{first:?}");
    assert_eq!(first.outcome, TickOutcome::Refused);
    assert!(
        first
            .refused
            .iter()
            .any(|r| r.code == bd_cache_sync::RefusalCode::MutexPoisoned)
    );
    assert_eq!(first.synced.len(), 1, "the tick still ran");

    w.next_tick();
    let second = done(job.run_blocking(w.after(TICK_BUDGET)));
    assert!(!second.poisoned, "reported once: {second:?}");
    assert_eq!(second.outcome, TickOutcome::PassThrough);
}

/// The scheduler asks `should_stop` before every run, so the stop check is
/// the first to meet a poisoned mutex; the report still reaches the run's reply.
#[test]
fn a_poison_met_by_the_stop_check_is_still_reported_by_the_next_tick() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let job = job(&w);
    let poisoner = {
        let job = job.clone();
        thread::spawn(move || job.with_cache(|_| panic!("a tick died")))
    };
    assert!(poisoner.join().is_err());

    // should_stop first (as sc-cron does), then run.
    assert_eq!(job.should_stop_blocking(), StopDecision::Continue);
    let run = done(job.run_blocking(w.after(TICK_BUDGET)));
    assert!(run.poisoned, "{run:?}");
    assert_eq!(
        run.refused[0].code,
        bd_cache_sync::RefusalCode::MutexPoisoned
    );
    // Reported once.
    w.next_tick();
    assert!(!done(job.run_blocking(w.after(TICK_BUDGET))).poisoned);
}

/// `try_with_cache` never waits: a held cache is `None`.
#[test]
fn try_with_cache_does_not_wait_for_a_running_tick() {
    let w = World::new();
    let job = job(&w);
    assert_eq!(job.try_with_cache(|_| 7), Some(7));
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let holder = {
        let job = job.clone();
        thread::spawn(move || {
            job.with_cache(|_| {
                held_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            });
        })
    };
    held_rx.recv().unwrap();
    assert_eq!(job.try_with_cache(|_| 7), None);
    release_tx.send(()).unwrap();
    holder.join().unwrap();
}

/// A job that could never run is not built.
#[test]
fn a_job_with_an_unusable_config_is_refused_at_construction() {
    let w = World::new();
    let mut cfg = w.config();
    cfg.fallback_lead = None;
    let built = SyncJob::new(
        w.cache(),
        &cfg,
        Arc::clone(&w.sender) as Arc<dyn bd_cache_model::EscalationSender>,
    );
    assert!(built.is_err());
}

/// The stop flag ends a running tick between phases.
#[test]
fn the_stop_flag_ends_a_tick_between_phases() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.phase("y", Some("lead-y"));
    let stop = bd_cache_sync::StopFlag::new();
    let job = SyncJob::with_stop(
        w.cache(),
        &w.config(),
        Arc::clone(&w.sender) as Arc<dyn bd_cache_model::EscalationSender>,
        stop.clone(),
    )
    .unwrap();
    stop.stop();
    let run = done(job.run_blocking(w.after(TICK_BUDGET)));
    assert!(run.stopped, "{run:?}");
    assert!(run.synced.is_empty(), "no phase was started");
    assert!(job.stop_flag().is_stopped());
}
