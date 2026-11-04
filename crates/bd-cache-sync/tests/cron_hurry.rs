//! A second stop request ends `cron run`'s grace period at once
//! (PE1-RSH-007, REQ-BDC-0730): the runs in flight are killed with their
//! process trees and the bd calls are asked to stop, without waiting for
//! `--shutdown-timeout-ms`. The Python row runs a stub interpreter (Unix).

#![cfg(unix)]
mod common;
#[path = "../../bd-cache-store/tests/support/fixture_child.rs"]
mod fixture_child;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bd_cache_sync::{Schedules, StopFlag, UsePython};
use common::gate_file;
use common::repo::{PyStub, Repo};
use sc_cron::ScheduleStore as _;

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
    let schedules = Arc::new(Schedules::new_with_bounded_store(
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

/// Production's worker must leave an actual SQLite write wait and join while
/// another connection still owns the writer lock. Expiry is a timed kill event,
/// not a consequence delayed until the upstream blocking-store future returns.
#[test]
fn ordinary_expiry_joins_the_contended_production_worker_before_writer_release()
-> Result<(), Box<dyn std::error::Error>> {
    lifecycle_fixture(
        "ordinary_expiry_joins_the_contended_production_worker_before_writer_release",
        |mode| bounded_store_stop(false, false, mode),
    )
}

/// A second signal shortens the same already-running store wait immediately.
#[test]
fn hurried_expiry_joins_the_contended_production_worker_before_writer_release()
-> Result<(), Box<dyn std::error::Error>> {
    lifecycle_fixture(
        "hurried_expiry_joins_the_contended_production_worker_before_writer_release",
        |mode| bounded_store_stop(true, false, mode),
    )
}

/// Zero timeout grants no grace and still owns the real store/worker join.
/// The 100ms observation watchdog below is test scheduling tolerance only.
#[test]
fn zero_timeout_joins_the_contended_worker_without_granting_a_grace_period()
-> Result<(), Box<dyn std::error::Error>> {
    lifecycle_fixture(
        "zero_timeout_joins_the_contended_worker_without_granting_a_grace_period",
        |mode| bounded_store_stop(false, true, mode),
    )
}

async fn bounded_store_stop(
    hurried: bool,
    zero: bool,
    mode: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = Repo::new(vec![]);
    repo.schedules().set(&every_second()).await?;
    let schedules = Arc::new(bd_cache_sync::SystemSchedules::system(
        Arc::clone(&repo.store),
        Arc::clone(&repo.cache),
        repo.files(),
    ));
    let control = schedules
        .worker_store_control_for_tests()
        .ok_or("production controller missing")?;
    for _cycle in 0..2 {
        schedules.enable(true).await?;
        stopped_write_cycle(&schedules, &repo, &control, hurried, zero, &mode).await?;
    }
    Ok(())
}

async fn stopped_write_cycle(
    schedules: &Arc<bd_cache_sync::SystemSchedules>,
    repo: &Repo,
    control: &bd_cache_store::ScheduleShutdownControl,
    hurried: bool,
    zero: bool,
    mode: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let attempted = control.wait_attempt_for_tests();
    let other = rusqlite::Connection::open(&repo.cache_path)?;
    other.execute_batch("BEGIN IMMEDIATE")?;
    let stop = StopFlag::new();
    let hurry = StopFlag::new();
    let (expired_send, expired_receive) = std::sync::mpsc::channel();
    let grace = if zero {
        Duration::ZERO
    } else if hurried {
        Duration::from_secs(10)
    } else {
        Duration::from_millis(250)
    };
    let run = spawn_stopping_worker(
        schedules,
        stop.clone(),
        hurry.clone(),
        grace,
        expired_send,
        mode == "stalled-worker-join",
    );
    let entered =
        tokio::task::spawn_blocking(move || attempted.recv_timeout(Duration::from_secs(5))).await?;
    let management_attempt = control.management_attempt_for_tests();
    let stall_management = mode == "stalled-management";
    let management = tokio::spawn({
        let schedules = Arc::clone(schedules);
        async move {
            let result = schedules.enable(false).await;
            if stall_management {
                eprintln!("stalled management join after real writer release");
                std::future::pending::<()>().await;
            }
            result
        }
    });
    let asked = Instant::now();
    stop.stop();
    if hurried {
        hurry.stop();
    }
    let management_entered = tokio::task::spawn_blocking(move || {
        management_attempt.recv_timeout(Duration::from_secs(2))
    })
    .await?;
    // This timeout only observes the owned task. Both success and failure paths
    // release the external lock and await the real join below.
    let mut run = run;
    let bounded = tokio::time::timeout(Duration::from_millis(500), &mut run).await;
    if bounded.is_err() {
        eprintln!("lifecycle shutdown observation expired after 500ms");
    }
    let completion_elapsed = asked.elapsed();
    let expiry = expired_receive.try_recv();
    let management_still_owned = !management.is_finished();
    other.execute_batch("ROLLBACK")?;
    let management_result = management.await?;
    let (finished, stopped) = match bounded {
        Ok(stopped) => (true, stopped?),
        Err(_) => (false, run.await?),
    };
    entered?;
    management_entered?;
    assert!(
        management_still_owned,
        "management call should retain its ordinary wait policy"
    );
    assert!(
        !management_result?.enabled,
        "management must complete after writer release"
    );
    assert!(
        finished
            && completion_elapsed
                < if zero {
                    Duration::from_millis(100)
                } else {
                    Duration::from_millis(500)
                },
        "worker waited for external writer release: {completion_elapsed:?}"
    );
    assert!(
        expiry?.duration_since(asked)
            < if hurried || zero {
                Duration::from_millis(100)
            } else {
                Duration::from_millis(300)
            }
    );
    assert!(!stopped.drained && stopped.idle, "{stopped:?}");
    assert!(
        repo.store.list_active().is_ok(),
        "public view inherits no expired worker deadline"
    );
    assert!(
        control.deadline_for_tests().is_none(),
        "joined worker leaves no stale deadline"
    );
    Ok(())
}

/// Same-core management must notify/cancel immediately, well before the 30s
/// reload poll, retaining the prior persisted outcome for the cancelled run.
#[test]
fn same_scheduler_management_cancels_the_running_row_without_a_new_outcome()
-> Result<(), Box<dyn std::error::Error>> {
    lifecycle_fixture(
        "same_scheduler_management_cancels_the_running_row_without_a_new_outcome",
        |mode| python_lifecycle(false, mode),
    )
}

/// A second start keeps upstream's rejection behavior and cannot clear the
/// already-installed shutdown deadline of the first worker.
#[test]
fn rejected_second_start_cannot_reset_the_first_workers_shutdown_deadline()
-> Result<(), Box<dyn std::error::Error>> {
    lifecycle_fixture(
        "rejected_second_start_cannot_reset_the_first_workers_shutdown_deadline",
        |mode| python_lifecycle(true, mode),
    )
}

async fn python_lifecycle(
    reject_second: bool,
    mode: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    let schedules = Arc::new(bd_cache_sync::SystemSchedules::system(
        Arc::clone(&repo.store),
        Arc::clone(&repo.cache),
        repo.files(),
    ));
    schedules.set(&every_second()).await?;
    schedules
        .use_python(
            &UsePython {
                python: stub.python.clone(),
                module: "consumer.sync".into(),
                cron: Some("* * * * * *".into()),
                timeout_ms: Some(60_000),
                env: vec![],
            },
            StopFlag::new(),
        )
        .await?;
    let before = repo
        .store
        .list_active()?
        .into_iter()
        .find(|row| row.spec.name.as_str() == bd_cache_sync::PYTHON_ROW)
        .ok_or("Python row missing")?;
    stub.set("tick", "managed_hang");
    let (stop, hurry) = (StopFlag::new(), StopFlag::new());
    let control = schedules
        .worker_store_control_for_tests()
        .ok_or("production controller missing")?;
    let run = tokio::spawn({
        let schedules = Arc::clone(&schedules);
        let (stop, hurry) = (stop.clone(), hurry.clone());
        async move {
            schedules
                .cron_run_hurried(stop, hurry, Duration::from_secs(20), || {})
                .await
        }
    });
    let ready = tokio::time::timeout(Duration::from_secs(5), async {
        while !stub.dir.path().join("run.ready").exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    let mut rejected = false;
    let mut immediate_cancel = false;
    let mut switched = None;
    if ready.is_ok() {
        if reject_second {
            rejected = rejected_start_keeps_deadline(
                &schedules,
                &control,
                &stop,
                mode == "stalled-second-start",
            )
            .await;
        } else {
            switched = Some(schedules.use_builtin().await);
            immediate_cancel = child_disappears(&stub).await;
        }
    }
    // All assertion/error paths stop and really join the worker and its child.
    stop.stop();
    hurry.stop();
    let stopped = common::join_task(run).await?;
    ready?;
    if reject_second {
        assert!(
            rejected,
            "second start must preserve the upstream panic and first deadline"
        );
    } else {
        switched.ok_or("management was not attempted")??;
        assert!(
            immediate_cancel,
            "same-scheduler change waited for reload polling"
        );
        let after = repo.store.get(before.id)?.ok_or("Python row disappeared")?;
        assert_eq!(after.last_run_at, before.last_run_at);
        assert_eq!(
            after.last_outcome, before.last_outcome,
            "cancelled run recorded a new outcome"
        );
    }
    assert!(stopped.idle);
    assert!(
        control.deadline_for_tests().is_none(),
        "joined worker left a stale deadline"
    );
    Ok(())
}

async fn rejected_start_keeps_deadline(
    schedules: &Arc<bd_cache_sync::SystemSchedules>,
    control: &bd_cache_store::ScheduleShutdownControl,
    stop: &StopFlag,
    stall_second: bool,
) -> bool {
    stop.stop();
    let deadline_ready = tokio::time::timeout(Duration::from_secs(2), async {
        while control.deadline_for_tests().is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let deadline = control.deadline_for_tests();
    let second_stop = StopFlag::new();
    let mut second = tokio::spawn({
        let schedules = Arc::clone(schedules);
        let stop = second_stop.clone();
        async move {
            if stall_second {
                eprintln!("stalled second start while first Python child is owned");
                std::future::pending::<()>().await;
            }
            schedules
                .cron_run(stop, Duration::from_secs(1), || {})
                .await
        }
    });
    let outcome =
        if let Ok(outcome) = tokio::time::timeout(Duration::from_secs(2), &mut second).await {
            outcome
        } else {
            eprintln!("second-start cleanup reached its 2s timeout");
            second_stop.stop();
            second.await
        };
    let rejected = if let Err(error) = outcome {
        if error.is_panic() {
            let panic = error.into_panic();
            panic
                .downcast_ref::<&str>()
                .is_some_and(|text| text.contains("worker for this scheduler is already running"))
                || panic.downcast_ref::<String>().is_some_and(|text| {
                    text.contains("worker for this scheduler is already running")
                })
        } else {
            false
        }
    } else {
        false
    };
    rejected
        && deadline_ready.is_ok()
        && deadline.is_some()
        && control.deadline_for_tests() == deadline
}

// Observe process disappearance before any stop signal: only same-core
// management notification can cancel this 60-second child at this point.
async fn child_disappears(stub: &PyStub) -> bool {
    let pid = std::fs::read_to_string(stub.dir.path().join("run.pid"));
    let Ok(pid) = pid else {
        return false;
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let output = std::process::Command::new("/bin/kill")
                .args(["-0", pid.trim()])
                .output();
            if let Ok(output) = output
                && !output.status.success()
                && String::from_utf8_lossy(&output.stderr)
                    .to_lowercase()
                    .contains("no such process")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok()
}

const LIFECYCLE_FIXTURE_MODE: &str = "BD_CACHE_CRON_LIFECYCLE_FIXTURE";

fn lifecycle_fixture<F>(
    test: &str,
    fixture: impl FnOnce(String) -> F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
    if let Ok(mode) = std::env::var(LIFECYCLE_FIXTURE_MODE) {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        return runtime.block_on(fixture(mode));
    }
    let mut child = fixture_child::FixtureChild::spawn(test, LIFECYCLE_FIXTURE_MODE, "normal")?;
    fixture_child::assert_passed(&mut child)?;
    Ok(())
}

#[test]
fn lifecycle_supervisor_kills_and_reaps_stalled_join_families()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "ordinary_expiry_joins_the_contended_production_worker_before_writer_release",
            "stalled-worker-join",
            "stalled lifecycle worker join after real shutdown",
        ),
        (
            "ordinary_expiry_joins_the_contended_production_worker_before_writer_release",
            "stalled-management",
            "stalled management join after real writer release",
        ),
        (
            "rejected_second_start_cannot_reset_the_first_workers_shutdown_deadline",
            "stalled-second-start",
            "stalled second start while first Python child is owned",
        ),
    ];
    for (test, mode, marker) in cases {
        let started = Instant::now();
        let mut child = fixture_child::FixtureChild::spawn(test, LIFECYCLE_FIXTURE_MODE, mode)?;
        assert!(
            child.wait_for_output(marker, Duration::from_secs(3))?,
            "fixture never reached its actual cleanup stall: {}",
            child.output()?
        );
        let pids = child.owned_pids()?;
        if mode == "stalled-second-start" {
            assert!(
                !pids.is_empty(),
                "negative must own the first real Python child"
            );
            for pid in &pids {
                assert!(fixture_child::pid_exists(*pid)?);
            }
        }
        let receipt = child.cleanup_receipt();
        assert!(
            child.finish(Duration::from_secs(5))?.is_none(),
            "stalled fixture unexpectedly exited: {}",
            child.output()?
        );
        assert!(
            receipt.load(Ordering::SeqCst),
            "fixture cleanup did not reap its owned process tree"
        );
        assert!(
            started.elapsed() < Duration::from_secs(7),
            "supervisor cleanup did not terminate"
        );
        for pid in &pids {
            assert!(
                !fixture_child::pid_exists(*pid)?,
                "Python child survived fixture cleanup"
            );
        }
        eprintln!(
            "FTQ-010 {mode}: finite timeout cleanup in {:?}; owned Python PIDs reaped: {pids:?}",
            started.elapsed()
        );
        let output = child.output()?;
        if mode == "stalled-worker-join" {
            assert!(
                output.contains("lifecycle shutdown observation expired after 500ms"),
                "{output}"
            );
        }
        if mode == "stalled-second-start" {
            assert!(
                output.contains("second-start cleanup reached its 2s timeout"),
                "{output}"
            );
        }
    }
    Ok(())
}

#[test]
fn lifecycle_supervisor_reaps_the_fixture_and_python_during_assertion_unwind()
-> Result<(), Box<dyn std::error::Error>> {
    let child = fixture_child::FixtureChild::spawn(
        "rejected_second_start_cannot_reset_the_first_workers_shutdown_deadline",
        LIFECYCLE_FIXTURE_MODE,
        "stalled-second-start",
    )?;
    assert!(
        child.wait_for_output(
            "stalled second start while first Python child is owned",
            Duration::from_secs(3)
        )?,
        "first Python child was not reached: {}",
        child.output()?
    );
    let pids = child.owned_pids()?;
    assert!(!pids.is_empty());
    let receipt = child.cleanup_receipt();
    let started = Instant::now();
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _owner = child;
        panic!("injected parent assertion");
    }));
    assert!(failure.is_err());
    assert!(receipt.load(Ordering::SeqCst));
    assert!(started.elapsed() < Duration::from_secs(2));
    for pid in &pids {
        assert!(
            !fixture_child::pid_exists(*pid)?,
            "unwind left a Python child"
        );
    }
    eprintln!(
        "FTQ-010 assertion-unwind cleanup in {:?}; owned Python PIDs reaped: {pids:?}",
        started.elapsed()
    );
    Ok(())
}

fn every_second() -> bd_cache_sync::ScheduleSet {
    bd_cache_sync::ScheduleSet {
        expr: "* * * * * *".into(),
        budget_ms: None,
        hook_reserve_ms: None,
        post_sync: None,
        python: None,
        env: vec![],
    }
}

fn spawn_stopping_worker(
    schedules: &Arc<bd_cache_sync::SystemSchedules>,
    stop: StopFlag,
    hurry: StopFlag,
    grace: Duration,
    expired_send: std::sync::mpsc::Sender<Instant>,
    stall: bool,
) -> tokio::task::JoinHandle<bd_cache_sync::CronStopped> {
    let schedules = Arc::clone(schedules);
    tokio::spawn(async move {
        let stopped = schedules
            .cron_run_hurried(stop, hurry, grace, move || {
                let _ = expired_send.send(Instant::now());
            })
            .await;
        if stall {
            eprintln!("stalled lifecycle worker join after real shutdown");
            std::future::pending::<()>().await;
        }
        stopped
    })
}
