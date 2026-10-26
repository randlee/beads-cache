//! Dropping `Scheduler::run_due_with`'s future aborts the runs it started
//! (as a stopped worker's are), so a caller stopped by a signal leaves no
//! run detached (RBP-F016).

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use common::Fixture;
use sc_cron::{HookTarget, Job, StopDecision, TargetResolver};
use serde_json::Value;
use tokio::sync::Notify;

/// Sets its flag when dropped: the run's future was dropped (aborted).
struct DroppedFlag(Arc<AtomicBool>);

impl Drop for DroppedFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// A run that says it started, then never ends on its own.
struct Hangs {
    started: Arc<Notify>,
    dropped: Arc<AtomicBool>,
}

#[async_trait]
impl Job for Hangs {
    async fn should_stop(&self, _payload: &Value) -> StopDecision {
        StopDecision::Continue
    }

    async fn run(&self, _payload: &Value, _deadline: Instant) -> sc_cron::RunOutcome {
        let _guard = DroppedFlag(Arc::clone(&self.dropped));
        self.started.notify_one();
        std::future::pending::<()>().await;
        sc_cron::RunOutcome::Ok
    }
}

/// The run is aborted with the dropped caller. Planted negative: the same
/// run on a bare spawned task, its handle dropped, is detached and keeps
/// going (what `run_due_with` did before RBP-F016).
#[tokio::test]
async fn dropping_run_due_with_aborts_the_runs_it_started() {
    let fixture = Fixture::new();
    let _due = fixture.start(fixture.spec("due"));
    fixture.clock.advance(chrono::Duration::seconds(60));
    let started = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let job: Arc<dyn Job> = Arc::new(Hangs {
        started: Arc::clone(&started),
        dropped: Arc::clone(&dropped),
    });
    let target = Arc::clone(&job);
    let resolver: TargetResolver = Arc::new(move |_| Ok(HookTarget::Rust(Arc::clone(&target))));
    let run = fixture.scheduler.run_due_with(resolver);
    tokio::select! {
        _ = run => panic!("the hanging run never ends by itself"),
        () = started.notified() => {}
    }
    // `run` is dropped here: its runs must be aborted.
    let until = Instant::now() + Duration::from_secs(10);
    while !dropped.load(Ordering::SeqCst) && Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(dropped.load(Ordering::SeqCst), "the run was left detached");

    let detached_flag = Arc::new(AtomicBool::new(false));
    let detached = Arc::new(Hangs {
        started: Arc::new(Notify::new()),
        dropped: Arc::clone(&detached_flag),
    });
    let handle = tokio::spawn({
        let detached = Arc::clone(&detached);
        async move { detached.run(&Value::Null, Instant::now()).await }
    });
    let abort = handle.abort_handle();
    drop(handle);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !detached_flag.load(Ordering::SeqCst),
        "a dropped bare handle detaches its task"
    );
    abort.abort();
}
