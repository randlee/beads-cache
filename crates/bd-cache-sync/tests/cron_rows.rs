//! The sync rows on sc-cron (REQ-BDC-0720, 0725, 0730, REQ-CRON-0016):
//! `cron run --once` over the real table with sc-cron's `ManualClock`, the
//! per-row resolver, the Python row through sc-cron's protocol, and the
//! worker's shutdown. The Python row runs a stub interpreter (Unix).

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::sync::Arc;
use std::time::Duration;

use bd_cache_sync::{
    BUILTIN_ROW, PYTHON_ROW, ScheduleSet, UsePython, cron_run, cron_run_once, schedule_set,
    sync_use_python,
};
use common::repo::{PyStub, Repo};
use common::{gate_file, label_scope};
use sc_cron::{RunOutcome, ScheduleSpec, ScheduleStore as _};

fn every_minute() -> ScheduleSet {
    ScheduleSet {
        expr: "0 * * * * *".to_owned(),
        ..ScheduleSet::default()
    }
}

fn row(repo: &Repo, name: &str) -> sc_cron::Schedule {
    repo.store
        .list_active()
        .unwrap()
        .into_iter()
        .find(|r| r.spec.name.as_str() == name)
        .unwrap()
}

fn step(repo: &Repo, secs: i64) {
    repo.cron_clock.advance(chrono::Duration::seconds(secs));
}

/// `cron run --once` runs a due row and leaves one that is not due alone.
#[tokio::test(flavor = "multi_thread")]
async fn cron_run_once_runs_a_due_row_and_skips_one_not_due() {
    let repo = Repo::new(vec![label_scope("lbl", "area")]);
    repo.bd.insert({
        let mut b = sc_beads::testing::bead("b1");
        b.labels = vec!["area".into()];
        b
    });
    schedule_set(&repo.scheduler, &every_minute()).await.unwrap();
    let other = repo
        .scheduler
        .start(ScheduleSpec::new("other", "0 0 * * * *", Duration::from_secs(5), serde_json::json!({"x": 1})).unwrap())
        .unwrap();
    assert!(cron_run_once(&repo.scheduler, Arc::new(repo.ctx())).await.unwrap().is_empty(), "nothing due yet");
    step(&repo, 60);
    let runs = cron_run_once(&repo.scheduler, Arc::new(repo.ctx())).await.unwrap();
    assert_eq!(runs.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), [BUILTIN_ROW]);
    assert_eq!(runs[0].outcome, RunOutcome::Ok);
    assert_eq!(row(&repo, BUILTIN_ROW).last_outcome, Some(RunOutcome::Ok));
    let members = repo.cache.lock().unwrap().scopes().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(row(&repo, "other").last_outcome, None, "not due, not run");
    assert_eq!(row(&repo, "other").generation, other.generation);
}

/// A row the resolver cannot resolve is recorded `Failed`, stays active and
/// enabled, and `cron run --once` fails with `TICK_RUN_FAILED` naming it.
#[tokio::test(flavor = "multi_thread")]
async fn an_unresolvable_sync_row_is_recorded_failed_and_stays_active() {
    let repo = Repo::new(vec![]);
    repo.scheduler
        .start(ScheduleSpec::new(BUILTIN_ROW, "0 * * * * *", Duration::from_secs(35), serde_json::json!({"target": "nonsense"})).unwrap())
        .unwrap();
    step(&repo, 60);
    let e = cron_run_once(&repo.scheduler, Arc::new(repo.ctx())).await.unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.TICK_RUN_FAILED");
    assert!(e.to_string().contains(BUILTIN_ROW), "{e}");
    let r = row(&repo, BUILTIN_ROW);
    assert!(r.is_active() && r.enabled);
    assert!(matches!(&r.last_outcome, Some(RunOutcome::Failed(why)) if why.starts_with("resolve: ")), "{r:?}");
}

/// With the Python row enabled, `cron run --once` runs the script with
/// `cron.stop` then `cron.tick` and runs no `bd list`; the built-in row of
/// the same table runs its own Rust target when it is the enabled one.
#[tokio::test(flavor = "multi_thread")]
async fn one_table_runs_the_python_row_with_its_script_and_the_builtin_row_with_rust() {
    let repo = Repo::new(vec![label_scope("lbl", "area")]);
    schedule_set(&repo.scheduler, &every_minute()).await.unwrap();
    let stub = PyStub::new();
    let req = UsePython { python: stub.python.clone(), module: "consumer.sync".into(), cron: None, timeout_ms: Some(5_000), env: vec![] };
    sync_use_python(&repo.scheduler, &repo.ctx(), &req).await.unwrap();
    repo.bd.clear_calls();
    step(&repo, 60);
    let runs = cron_run_once(&repo.scheduler, Arc::new(repo.ctx())).await.unwrap();
    assert_eq!(runs.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), [PYTHON_ROW]);
    assert_eq!(runs[0].outcome, RunOutcome::Ok);
    assert_eq!((stub.count("stop"), stub.count("tick")), (2, 2), "the immediate run, then this one");
    assert!(stub.request("tick", 2)["payload"]["run_id"].is_string());
    assert!(repo.bd.calls().iter().all(|c| c.op() != sc_beads::testing::FakeOp::ListScope), "no bd list");
    bd_cache_sync::sync_use_builtin(&repo.scheduler).await.unwrap();
    step(&repo, 60);
    let runs = cron_run_once(&repo.scheduler, Arc::new(repo.ctx())).await.unwrap();
    assert_eq!(runs.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), [BUILTIN_ROW]);
    assert!(repo.bd.calls().iter().any(|c| c.op() == sc_beads::testing::FakeOp::ListScope));
    assert_eq!(stub.count("tick"), 2, "the disabled Python row did not run");
}

/// A group-writable cache file runs nothing; a script that answers
/// `cron.stop` with no JSON is recorded `Failed` (REQ-BDC-0723, 0725).
#[tokio::test(flavor = "multi_thread")]
async fn an_untrusted_cache_or_a_stop_answer_that_is_not_json_runs_nothing() {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    let req = UsePython { python: stub.python.clone(), module: "consumer.sync".into(), cron: Some("0 * * * * *".into()), timeout_ms: Some(5_000), env: vec![] };
    sync_use_python(&repo.scheduler, &repo.ctx(), &req).await.unwrap();
    std::fs::set_permissions(&repo.cache_path, std::fs::Permissions::from_mode(0o664)).unwrap();
    step(&repo, 60);
    let e = cron_run_once(&repo.scheduler, Arc::new(repo.ctx())).await.unwrap_err();
    assert!(e.to_string().contains("not trusted"), "{e}");
    assert_eq!((stub.count("stop"), stub.count("tick")), (1, 1), "nothing ran after the immediate run");
    std::fs::set_permissions(&repo.cache_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    stub.set("stop", "not json");
    step(&repo, 60);
    let e = cron_run_once(&repo.scheduler, Arc::new(repo.ctx())).await.unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.TICK_RUN_FAILED");
    assert_eq!(stub.count("tick"), 1, "no cron.tick after an undecided stop check");
    assert!(matches!(row(&repo, PYTHON_ROW).last_outcome, Some(RunOutcome::Failed(_))));
    // Planted negative: a JSON continue runs the tick.
    stub.unset("stop");
    step(&repo, 60);
    cron_run_once(&repo.scheduler, Arc::new(repo.ctx())).await.unwrap();
    assert_eq!(stub.count("tick"), 2);
}

/// `cron run` stops on its stop flag: no new run, the run in flight is
/// waited for up to the shutdown timeout and then killed with its tree
/// (here a Python row whose script starts a grandchild that ignores
/// SIGTERM), and the lease is released (REQ-BDC-0730).
#[tokio::test(flavor = "multi_thread")]
async fn cron_run_stops_on_its_flag_kills_the_tree_and_releases_the_lease() {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    let req = UsePython { python: stub.python.clone(), module: "consumer.sync".into(), cron: Some("* * * * * *".into()), timeout_ms: Some(60_000), env: vec![] };
    sync_use_python(&repo.scheduler, &repo.ctx(), &req).await.unwrap();
    stub.set("tick", "grandchild");
    let (ready, life) = {
        let mk = |n: &str| {
            let p = stub.dir.path().join(n);
            assert!(std::process::Command::new("mkfifo").arg(&p).status().unwrap().success());
            p
        };
        (gate_file(mk("ready")), gate_file(mk("life")))
    };
    let real = Arc::new(sc_cron::SystemClock);
    let ctx = Arc::new(bd_cache_sync::SyncContext {
        cache: Arc::clone(&repo.cache),
        clock: Arc::clone(&real),
        cache_path: repo.cache_path.clone(),
        beads_dir: None,
        stop: bd_cache_sync::StopFlag::new(),
        kill: bd_cache_sync::StopFlag::new(),
    });
    let scheduler = sc_cron::Scheduler::new(Arc::clone(&repo.store), real, Arc::new(sc_cron::SystemJitter));
    let stop = ctx.stop.clone();
    let run = tokio::spawn(cron_run(scheduler, ctx, Duration::from_millis(200)));
    tokio::task::spawn_blocking(move || ready.recv_timeout(Duration::from_secs(20)).unwrap()).await.unwrap();
    stop.stop();
    let stopped = run.await.unwrap();
    assert!(!stopped.drained, "the hanging run was killed at the shutdown timeout");
    tokio::task::spawn_blocking(move || life.recv_timeout(Duration::from_secs(20)).unwrap()).await.unwrap();
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
