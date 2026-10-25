//! `SyncJob`'s threading (ADR-CORE-0001; REQ-BDC-0703): the cache in an
//! `Arc<Mutex<_>>` under `spawn_blocking`, a busy tick skipped, a poisoned
//! mutex's cache dropped and recomposed (S55), and the shutdown wait for the
//! tick in flight (PE-RSH-004).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use bd_cache_sync::{ComposeError, StopFlag, SyncJob, TICK_BUDGET, TickOutcome, TickRun};
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

/// A job that recomposes its cache over the world's parts, counting each build in `built`.
fn recomposing(w: &World, built: &Arc<AtomicUsize>) -> TestJob {
    SyncJob::with_recompose(
        w.cache(),
        &w.config(),
        Arc::clone(&w.sender) as Arc<dyn bd_cache_model::EscalationSender>,
        StopFlag::new(),
        w.recompose(built),
    )
    .unwrap()
}

/// How many caches over the world's backend are alive besides the world's
/// own handle and the one the recompose function keeps: the job's cache.
fn caches_alive(w: &World) -> usize {
    Arc::strong_count(&w.switch.gate) - 2
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
    job.with_cache(|_| ()).unwrap();
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
            })
            .unwrap();
        })
    };
    held_rx.recv().unwrap();
    assert_eq!(job.should_stop_blocking(), StopDecision::Continue);
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    assert!(matches!(job.should_stop_blocking(), StopDecision::Stop(_)));
}

/// A host call that panics leaves a poisoned mutex: its cache is not reused
/// but dropped, the next tick runs on a recomposed one and reports it once
/// (S55, PE-RBP-F007).
#[test]
fn a_poisoned_mutex_drops_the_cache_and_the_next_tick_recomposes() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let built = Arc::new(AtomicUsize::new(0));
    let job = recomposing(&w, &built);
    let poisoner = {
        let job = job.clone();
        thread::spawn(move || job.with_cache(|_| panic!("a tick died")))
    };
    assert!(poisoner.join().is_err());
    assert_eq!(
        built.load(Ordering::SeqCst),
        0,
        "nothing is rebuilt eagerly"
    );

    let first = done(job.run_blocking(w.after(TICK_BUDGET)));
    assert_eq!(built.load(Ordering::SeqCst), 1, "the tick recomposed");
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
    assert_eq!(built.load(Ordering::SeqCst), 1, "recomposed once");
}

/// A tick whose backend panics, under the scheduler's `spawn_blocking`:
/// the stop check that comes next drops the panicked cache without building
/// one, and the next run recomposes from the same inputs (S55, PE-RBP-F007).
#[tokio::test(flavor = "multi_thread")]
async fn a_tick_that_panics_in_the_backend_is_not_reused_and_the_next_run_recomposes() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let built = Arc::new(AtomicUsize::new(0));
    let job = recomposing(&w, &built);
    let payload = serde_json::json!({});
    let deadline = Instant::now() + TICK_BUDGET;
    assert_eq!(caches_alive(&w), 1);

    w.switch.set_panicking(true);
    let died = job.run(&payload, deadline).await;
    assert!(
        matches!(&died, RunOutcome::Failed(m) if m.starts_with("tick_task_failed")),
        "{died:?}"
    );
    w.switch.set_panicking(false);

    // sc-cron asks should_stop before every run: it meets the poison first.
    assert_eq!(job.should_stop(&payload).await, StopDecision::Continue);
    assert_eq!(caches_alive(&w), 0, "the panicked cache was dropped");
    assert_eq!(built.load(Ordering::SeqCst), 0);

    let next = job.run(&payload, deadline).await;
    assert!(
        matches!(&next, RunOutcome::Refused(m) if m.contains("mutex_poisoned")),
        "the poison is reported: {next:?}"
    );
    assert_eq!(built.load(Ordering::SeqCst), 1, "one recomposed cache");
    assert_eq!(caches_alive(&w), 1);

    w.next_tick();
    assert_eq!(job.run(&payload, deadline).await, RunOutcome::Ok);
    assert_eq!(built.load(Ordering::SeqCst), 1);
}

/// The scheduler asks `should_stop` before every run, so the stop check is
/// the first to meet a poisoned mutex; the report still reaches the run's reply.
#[test]
fn a_poison_met_by_the_stop_check_is_still_reported_by_the_next_tick() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let built = Arc::new(AtomicUsize::new(0));
    let job = recomposing(&w, &built);
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
    assert_eq!(built.load(Ordering::SeqCst), 1);
}

/// A job with no recompose function never reuses a panicked cache either: it
/// reports the cache lost, to the tick and to the host (S55).
#[test]
fn a_job_without_a_recompose_function_reports_the_cache_lost_after_a_panic() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let job = job(&w);
    let poisoner = {
        let job = job.clone();
        thread::spawn(move || job.with_cache(|_| panic!("a tick died")))
    };
    assert!(poisoner.join().is_err());

    match job.run_blocking(w.after(TICK_BUDGET)) {
        TickRun::Lost { error, poisoned } => {
            assert!(poisoned);
            assert_eq!(error.code(), "sync_cache_lost");
            assert!(error.cause().is_none());
        }
        other => panic!("expected a lost cache, got {other:?}"),
    }
    let err = job.with_cache(|_| ()).unwrap_err();
    assert_eq!(err.code(), "sync_cache_lost");
    assert!(matches!(job.try_with_cache(|_| ()), Err(e) if e.code() == "sync_cache_lost"));
    assert!(matches!(
        job.run_blocking(w.after(TICK_BUDGET)),
        TickRun::Lost {
            poisoned: false,
            ..
        }
    ));
}

/// A recompose that fails is a lost cache with its cause; the next tick tries
/// again (S55).
#[test]
fn a_recompose_that_fails_is_tried_again_by_the_next_tick() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let attempts = Arc::new(AtomicUsize::new(0));
    let built = Arc::new(AtomicUsize::new(0));
    let recompose = w.recompose(&built);
    let tries = Arc::clone(&attempts);
    let job: TestJob = SyncJob::with_recompose(
        w.cache(),
        &w.config(),
        Arc::clone(&w.sender) as Arc<dyn bd_cache_model::EscalationSender>,
        StopFlag::new(),
        move || {
            if tries.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(ComposeError::Cache(
                    bd_cache_model::CacheError::NoDatabase {
                        detail: "metadata.json is gone".into(),
                        source: None,
                    },
                ))
            } else {
                recompose()
            }
        },
    )
    .unwrap();
    let poisoner = {
        let job = job.clone();
        thread::spawn(move || job.with_cache(|_| panic!("a tick died")))
    };
    assert!(poisoner.join().is_err());

    match job.run_blocking(w.after(TICK_BUDGET)) {
        TickRun::Lost { error, poisoned } => {
            assert!(poisoned);
            assert_eq!(
                error.cause().map(ComposeError::code),
                Some("BD_CACHE.NO_DATABASE")
            );
        }
        other => panic!("expected a lost cache, got {other:?}"),
    }
    let run = done(job.run_blocking(w.after(TICK_BUDGET)));
    assert_eq!(run.synced.len(), 1, "{run:?}");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(built.load(Ordering::SeqCst), 1);
}

/// The shutdown wait (PE-RSH-004): `join_idle` does not return `true` while a
/// tick holds the cache, answers `false` at a deadline that has passed, and
/// returns `true` once the tick ends.
///
/// Causal, with no sleep and no elapsed-time assertion: the waiter announces
/// that it is about to wait, the test thread then opens the gate, and the
/// waiter's `true` must come after the open (`opened` is set just before it).
#[test]
fn join_idle_waits_for_the_tick_in_flight_up_to_its_deadline() {
    /// A bound so a broken `join_idle` fails the test instead of hanging; the
    /// waiter returns when the gate opens, long before it.
    const SAFETY: Duration = Duration::from_secs(60);
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let job = job(&w);
    assert!(job.join_idle(Instant::now()), "an idle job is idle at once");

    w.switch.gate.arm();
    let running = {
        let job = job.clone();
        let end = w.after(TICK_BUDGET);
        thread::spawn(move || job.run_blocking(end))
    };
    w.switch.gate.wait_entered(1);

    // A deadline that has passed is answered at once: the tick still holds the cache.
    assert!(!job.join_idle(Instant::now()), "the tick holds the cache");
    assert!(!running.is_finished());

    let opened = Arc::new(AtomicBool::new(false));
    let (waiting_tx, waiting_rx) = mpsc::channel::<()>();
    let waiter = {
        let job = job.clone();
        let opened = Arc::clone(&opened);
        thread::spawn(move || {
            waiting_tx.send(()).unwrap();
            let idle = job.join_idle(Instant::now() + SAFETY);
            (idle, opened.load(Ordering::SeqCst))
        })
    };
    waiting_rx.recv().unwrap();
    opened.store(true, Ordering::SeqCst);
    w.switch.gate.open();

    let (idle, opened_first) = waiter.join().unwrap();
    assert!(idle, "the tick ended before the deadline");
    assert!(
        opened_first,
        "join_idle said idle while the tick still held the cache"
    );
    let reply = done(running.join().unwrap());
    assert_eq!(reply.outcome, TickOutcome::PassThrough, "{reply:?}");
}

/// `try_with_cache` never waits: a held cache is `None`.
#[test]
fn try_with_cache_does_not_wait_for_a_running_tick() {
    let w = World::new();
    let job = job(&w);
    assert_eq!(job.try_with_cache(|_| 7).unwrap(), Some(7));
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let holder = {
        let job = job.clone();
        thread::spawn(move || {
            job.with_cache(|_| {
                held_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
            .unwrap();
        })
    };
    held_rx.recv().unwrap();
    assert_eq!(job.try_with_cache(|_| 7).unwrap(), None);
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

/// A stop that arrives while a phase is syncing ends the tick before the next
/// phase starts, but lets the phase in progress finish.
#[test]
fn a_stop_that_arrives_mid_tick_ends_it_between_phases() {
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
    w.switch.gate.arm();
    let running = {
        let job = job.clone();
        let end = w.after(TICK_BUDGET);
        thread::spawn(move || job.run_blocking(end))
    };
    // The first phase is inside its `bd list`; ask for the stop, then let it go.
    w.switch.gate.wait_entered(1);
    stop.stop();
    w.switch.gate.open();
    let reply = done(running.join().unwrap());
    assert!(reply.stopped, "{reply:?}");
    assert_eq!(
        reply.synced.len(),
        1,
        "the phase in progress finished, the next did not start"
    );
}
