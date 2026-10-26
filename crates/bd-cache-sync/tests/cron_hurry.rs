//! A second stop request ends `cron run`'s grace period at once
//! (PE1-RSH-007, REQ-BDC-0730): the runs in flight are killed with their
//! process trees and the bd calls are asked to stop, without waiting for
//! `--shutdown-timeout-ms`. The Python row runs a stub interpreter (Unix).

#![cfg(unix)]
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bd_cache_sync::{Schedules, StopFlag, UsePython};
use common::gate_file;
use common::repo::{PyStub, Repo};

/// The shutdown timeout the hurry must not wait for.
const GRACE: Duration = Duration::from_secs(20);

/// With the hurry flag set, a run that would otherwise be waited for up to
/// [`GRACE`] (a script whose grandchild ignores SIGTERM) is killed at once:
/// `cron_run_hurried` returns well inside the grace, undrained, with
/// `on_expiry` called once and the lease released. Planted negative: with
/// the hurry flag never set the same call waits the whole grace.
#[tokio::test(flavor = "multi_thread")]
async fn a_second_stop_kills_the_runs_without_waiting_for_the_grace() {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    repo.schedules()
        .use_python(
            &UsePython {
                python: stub.python.clone(),
                module: "consumer.sync".into(),
                cron: Some("* * * * * *".to_owned()),
                timeout_ms: Some(60_000),
                env: vec![],
            },
            StopFlag::new(),
        )
        .await
        .unwrap();
    stub.set("tick", "grandchild");
    let mk = |n: &str| {
        let p = stub.dir.path().join(n);
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&p)
                .status()
                .unwrap()
                .success()
        );
        p
    };
    let (ready, life) = (gate_file(mk("ready")), gate_file(mk("life")));
    let schedules = Arc::new(Schedules::new(
        Arc::clone(&repo.store),
        Arc::new(sc_cron::SystemClock),
        Arc::new(sc_cron::SystemJitter),
        Arc::clone(&repo.cache),
        repo.files(),
    ));
    let (stop, hurry) = (StopFlag::new(), StopFlag::new());
    let expired = Arc::new(AtomicBool::new(false));
    let run = tokio::spawn({
        let (schedules, stop, hurry, expired) = (
            Arc::clone(&schedules),
            stop.clone(),
            hurry.clone(),
            Arc::clone(&expired),
        );
        async move {
            schedules
                .cron_run_hurried(stop, hurry, GRACE, move || {
                    expired.store(true, Ordering::SeqCst);
                })
                .await
        }
    });
    tokio::task::spawn_blocking(move || ready.recv_timeout(Duration::from_secs(20)).unwrap())
        .await
        .unwrap();
    let asked = Instant::now();
    stop.stop();
    hurry.stop();
    let stopped = run.await.unwrap();
    assert!(
        asked.elapsed() < GRACE / 2,
        "the hurry ended the grace period: took {:?}",
        asked.elapsed()
    );
    assert!(
        !stopped.drained,
        "the run in flight was killed, not drained"
    );
    assert!(expired.load(Ordering::SeqCst), "on_expiry ran on the hurry");
    tokio::task::spawn_blocking(move || life.recv_timeout(Duration::from_secs(20)).unwrap())
        .await
        .unwrap();
    // The lease guard's drop released the lease on the blocking pool.
    for _ in 0..200 {
        if repo.cache.lock().unwrap().lease().unwrap().is_none() {
            return;
        }
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the lease was not released");
}
