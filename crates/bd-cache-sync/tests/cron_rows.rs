//! The sync rows on sc-cron (REQ-BDC-0720, 0725, 0730, REQ-CRON-0016):
//! `cron run --once` over the real table with sc-cron's `ManualClock`, the
//! per-row resolver, the Python row through sc-cron's protocol, and the
//! worker's shutdown. The Python row runs a stub interpreter (Unix).

#![cfg(unix)]
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::sync::Arc;
use std::time::Duration;

use bd_cache_sync::{BUILTIN_ROW, PYTHON_ROW, ScheduleSet, Schedules, StopFlag, UsePython};
use common::repo::{PyStub, Repo};
use common::{gate_file, label_scope};
use sc_cron::{RunOutcome, ScheduleSpec, ScheduleStore as _};

fn every_minute() -> ScheduleSet {
    ScheduleSet {
        expr: "0 * * * * *".to_owned(),
        budget_ms: None,
        hook_reserve_ms: None,
        post_sync: None,
        python: None,
        env: vec![],
    }
}

/// `cron run --once` over `repo`: the rows that ran, or the failure.
async fn cron_run_once(repo: &Repo) -> Result<Vec<(String, String)>, bd_cache_api::ApiError> {
    let once = repo
        .schedules()
        .cron_run_once(StopFlag::new())
        .await
        .map_err(|e| bd_cache_api::ApiError::from(&e))?;
    match once.error() {
        Some(e) => Err(e),
        None => Ok(once.ran.into_iter().map(|r| (r.row, r.outcome)).collect()),
    }
}

async fn show(repo: &Repo) -> serde_json::Value {
    serde_json::to_value(repo.schedules().show().await.unwrap()).unwrap()
}

fn use_python(stub: &PyStub, cron: Option<&str>, timeout_ms: u64) -> UsePython {
    UsePython {
        python: stub.python.clone(),
        module: "consumer.sync".into(),
        cron: cron.map(str::to_owned),
        timeout_ms: Some(timeout_ms),
        env: vec![],
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
    repo.schedules().set(&every_minute()).await.unwrap();
    let other = repo
        .scheduler
        .start(
            ScheduleSpec::new(
                "other",
                "0 0 * * * *",
                Duration::from_secs(5),
                serde_json::json!({"x": 1}),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(
        cron_run_once(&repo).await.unwrap().is_empty(),
        "nothing due yet"
    );
    step(&repo, 60);
    let runs = cron_run_once(&repo).await.unwrap();
    assert_eq!(runs, [(BUILTIN_ROW.to_owned(), "ok".to_owned())]);
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
        .start(
            ScheduleSpec::new(
                BUILTIN_ROW,
                "0 * * * * *",
                Duration::from_secs(35),
                serde_json::json!({"target": "nonsense"}),
            )
            .unwrap(),
        )
        .unwrap();
    step(&repo, 60);
    let e = cron_run_once(&repo).await.unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.TICK_RUN_FAILED");
    assert_eq!(e.details()["row"], BUILTIN_ROW, "{e}");
    assert_eq!(e.details()["rows"][0]["row"], BUILTIN_ROW, "{e}");
    let r = row(&repo, BUILTIN_ROW);
    assert!(r.is_active() && r.enabled);
    assert!(
        matches!(&r.last_outcome, Some(RunOutcome::Failed(why)) if why.starts_with("resolve: ")),
        "{r:?}"
    );
}

/// With the Python row enabled, `cron run --once` runs the script with
/// `cron.stop` then `cron.tick` and runs no `bd list`; the built-in row of
/// the same table runs its own Rust target when it is the enabled one.
#[tokio::test(flavor = "multi_thread")]
async fn one_table_runs_the_python_row_with_its_script_and_the_builtin_row_with_rust() {
    let repo = Repo::new(vec![label_scope("lbl", "area")]);
    repo.schedules().set(&every_minute()).await.unwrap();
    let stub = PyStub::new();
    repo.schedules()
        .use_python(&use_python(&stub, None, 5_000), StopFlag::new())
        .await
        .unwrap();
    repo.bd.clear_calls();
    step(&repo, 60);
    let runs = cron_run_once(&repo).await.unwrap();
    assert_eq!(runs, [(PYTHON_ROW.to_owned(), "ok".to_owned())]);
    assert_eq!(
        (stub.count("stop"), stub.count("tick")),
        (2, 2),
        "the immediate run, then this one"
    );
    assert!(stub.request("tick", 2)["payload"]["run_id"].is_string());
    assert!(
        repo.bd
            .calls()
            .iter()
            .all(|c| c.op() != sc_beads::testing::FakeOp::ListScope),
        "no bd list"
    );
    repo.schedules().use_builtin().await.unwrap();
    step(&repo, 60);
    let runs = cron_run_once(&repo).await.unwrap();
    assert_eq!(runs[0].0, BUILTIN_ROW);
    assert!(
        repo.bd
            .calls()
            .iter()
            .any(|c| c.op() == sc_beads::testing::FakeOp::ListScope)
    );
    assert_eq!(stub.count("tick"), 2, "the disabled Python row did not run");
}

/// A group-writable cache file runs nothing; a script that answers
/// `cron.stop` with no JSON is recorded `Failed` (REQ-BDC-0723, 0725).
#[tokio::test(flavor = "multi_thread")]
async fn an_untrusted_cache_or_a_stop_answer_that_is_not_json_runs_nothing() {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    repo.schedules()
        .use_python(
            &use_python(&stub, Some("0 * * * * *"), 5_000),
            StopFlag::new(),
        )
        .await
        .unwrap();
    std::fs::set_permissions(&repo.cache_path, std::fs::Permissions::from_mode(0o664)).unwrap();
    step(&repo, 60);
    let e = cron_run_once(&repo).await.unwrap_err();
    assert!(e.message().contains("not trusted"), "{e}");
    assert_eq!(e.details()["reason"], "untrusted");
    assert_eq!(
        (stub.count("stop"), stub.count("tick")),
        (1, 1),
        "nothing ran after the immediate run"
    );
    std::fs::set_permissions(&repo.cache_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    stub.set("stop", "not json");
    step(&repo, 60);
    let e = cron_run_once(&repo).await.unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.TICK_RUN_FAILED");
    assert_eq!(
        stub.count("tick"),
        1,
        "no cron.tick after an undecided stop check"
    );
    assert!(matches!(
        row(&repo, PYTHON_ROW).last_outcome,
        Some(RunOutcome::Failed(_))
    ));
    // Planted negative: a JSON continue runs the tick.
    stub.unset("stop");
    step(&repo, 60);
    cron_run_once(&repo).await.unwrap();
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
    repo.schedules()
        .use_python(
            &use_python(&stub, Some("* * * * * *"), 60_000),
            StopFlag::new(),
        )
        .await
        .unwrap();
    stub.set("tick", "grandchild");
    let (ready, life) = {
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
        (gate_file(mk("ready")), gate_file(mk("life")))
    };
    let schedules = Arc::new(Schedules::new(
        Arc::clone(&repo.store),
        Arc::new(sc_cron::SystemClock),
        Arc::new(sc_cron::SystemJitter),
        Arc::clone(&repo.cache),
        repo.files(),
    ));
    let stop = StopFlag::new();
    let expired = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let run = tokio::spawn({
        let (schedules, stop, expired) =
            (Arc::clone(&schedules), stop.clone(), Arc::clone(&expired));
        async move {
            schedules
                .cron_run(stop, Duration::from_millis(200), move || {
                    expired.store(true, std::sync::atomic::Ordering::SeqCst);
                })
                .await
        }
    });
    tokio::task::spawn_blocking(move || ready.recv_timeout(Duration::from_secs(20)).unwrap())
        .await
        .unwrap();
    stop.stop();
    let stopped = run.await.unwrap();
    assert!(
        !stopped.drained,
        "the hanging run was killed at the shutdown timeout"
    );
    assert!(
        expired.load(std::sync::atomic::Ordering::SeqCst),
        "the bd calls are stopped only when the timeout expires"
    );
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

/// A Python row whose stored `cache_path` names another, private file runs
/// nothing when the cache file the worker opened is group-writable: the trust
/// check is made on the opened file, never on the row's path (REQ-BDC-0723,
/// B8Q1-001). Planted negative: the opened file private again, it runs, and
/// its script is given the opened file, not the row's.
#[tokio::test(flavor = "multi_thread")]
async fn a_planted_cache_path_in_the_row_cannot_pass_the_trust_check() {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    let decoy = repo.dir.path().join("decoy.sqlite3");
    std::fs::write(&decoy, b"").unwrap();
    common::private(&decoy);
    let payload = bd_cache_sync::PythonPayload::new(
        bd_cache_sync::TrustedInterpreter::new(&stub.python, "consumer.sync").unwrap(),
        decoy.clone(),
        None,
        bd_cache_sync::EnvNames::default(),
        Some(5_000),
    )
    .unwrap();
    repo.scheduler
        .start(
            ScheduleSpec::new(
                PYTHON_ROW,
                "0 * * * * *",
                Duration::from_secs(5),
                bd_cache_sync::SyncPayload::Python(payload).to_json(),
            )
            .unwrap(),
        )
        .unwrap();
    std::fs::set_permissions(&repo.cache_path, std::fs::Permissions::from_mode(0o664)).unwrap();
    step(&repo, 60);
    let e = cron_run_once(&repo).await.unwrap_err();
    assert_eq!(e.details()["reason"], "untrusted", "{e}");
    assert_eq!(
        (stub.count("stop"), stub.count("tick")),
        (0, 0),
        "nothing ran"
    );

    common::private(&repo.cache_path);
    step(&repo, 60);
    cron_run_once(&repo).await.unwrap();
    assert_eq!(stub.count("tick"), 1);
    let given = stub.request("tick", 1)["payload"]["cache_path"].clone();
    assert_eq!(
        given,
        repo.cache_path.display().to_string(),
        "the opened file"
    );
    assert!(
        stub.env("tick")
            .contains(&format!("BD_CACHE_PATH={}", repo.cache_path.display()))
    );
}

/// ANC-008: a built-in or a Python sync payload on a row of another name is
/// refused and recorded `Failed`, while a non-sync row coexists in the table
/// and the sync row beside it runs (B8Q1-017). Planted negative: the same
/// built-in payload on the built-in row's name runs.
#[tokio::test(flavor = "multi_thread")]
async fn a_sync_payload_on_another_row_is_refused_and_a_non_sync_row_coexists() {
    let repo = Repo::new(vec![]);
    repo.schedules().set(&every_minute()).await.unwrap();
    let builtin_payload = row(&repo, BUILTIN_ROW).spec.payload;
    for name in ["impostor.builtin", "plain.other"] {
        let payload = if name == "plain.other" {
            serde_json::json!({"kind": "not a sync row"})
        } else {
            builtin_payload.clone()
        };
        repo.scheduler
            .start(
                ScheduleSpec::new(name, "0 * * * * *", Duration::from_secs(35), payload).unwrap(),
            )
            .unwrap();
    }
    step(&repo, 60);
    let once = repo
        .schedules()
        .cron_run_once(StopFlag::new())
        .await
        .unwrap();
    let outcomes: std::collections::BTreeMap<String, String> =
        once.ran.into_iter().map(|r| (r.row, r.outcome)).collect();
    assert_eq!(outcomes[BUILTIN_ROW], "ok", "the sync row beside them runs");
    assert!(
        outcomes["impostor.builtin"].contains("only bd-cache.sync.builtin may"),
        "{outcomes:?}"
    );
    assert!(
        outcomes["plain.other"].contains("not a sync row"),
        "{outcomes:?}"
    );
    for name in ["impostor.builtin", "plain.other"] {
        let r = row(&repo, name);
        assert!(r.is_active() && r.enabled, "{name} stays");
        assert!(
            matches!(r.last_outcome, Some(RunOutcome::Failed(_))),
            "{name}"
        );
    }
}

/// `schedule show` keeps each row's history as typed data (REQ-BDC-0731,
/// B8Q1-008): an ok run's id; consecutive failures and the first one's time
/// across repeated timeouts and across repeated failures at resolve time,
/// each with the latest run's id. Planted negative: an ok run resets the count.
#[tokio::test(flavor = "multi_thread")]
async fn schedule_show_counts_failures_across_timeouts_and_resolve_failures() {
    let repo = Repo::new(vec![]);
    repo.schedules().set(&every_minute()).await.unwrap();
    step(&repo, 60);
    cron_run_once(&repo).await.unwrap();
    let v = show(&repo).await;
    let ok_id = v["rows"]["builtin"]["run_id"].clone();
    assert!(ok_id.is_string(), "an ok run has its run id: {v}");
    assert_eq!(v["rows"]["builtin"]["consecutive_failures"], 0);

    // The built-in row replaced by one whose payload does not decode: each
    // run fails at resolve time and counts.
    repo.scheduler
        .replace_and_set_enabled(
            ScheduleSpec::new(
                BUILTIN_ROW,
                "0 * * * * *",
                Duration::from_secs(35),
                serde_json::json!({"target": "nonsense"}),
            )
            .unwrap(),
            true,
            &[],
        )
        .unwrap();
    let mut since = None;
    for n in 1..=2 {
        step(&repo, 60);
        cron_run_once(&repo).await.unwrap_err();
        let v = show(&repo).await;
        let b = &v["rows"]["builtin"];
        assert_eq!(b["consecutive_failures"], n, "{v}");
        assert_ne!(b["run_id"], ok_id);
        since.get_or_insert_with(|| b["first_failure_at"].clone());
        assert_eq!(
            &b["first_failure_at"],
            since.as_ref().unwrap(),
            "kept from the first"
        );
    }

    // A Python row whose script outlives its timeout, twice.
    repo.schedules().use_builtin().await.unwrap();
    let stub = PyStub::new();
    repo.schedules()
        .use_python(
            &use_python(&stub, Some("0 * * * * *"), 1_000),
            StopFlag::new(),
        )
        .await
        .unwrap();
    stub.set("tick", "sleep");
    let mut ids = Vec::new();
    for n in 1..=2 {
        step(&repo, 60);
        let e = cron_run_once(&repo).await.unwrap_err();
        assert_eq!(e.details()["reason"], "timeout", "{e}");
        let v = show(&repo).await;
        let p = &v["rows"]["python"];
        assert_eq!(p["consecutive_failures"], n, "{v}");
        assert!(p["first_failure_at"].is_string());
        ids.push(p["run_id"].clone());
    }
    assert_ne!(ids[0], ids[1], "the latest run's id");
    stub.unset("tick");
    step(&repo, 60);
    cron_run_once(&repo).await.unwrap();
    let v = show(&repo).await;
    assert_eq!(
        v["rows"]["python"]["consecutive_failures"], 0,
        "an ok run resets it"
    );
    assert!(v["rows"]["python"]["first_failure_at"].is_null());
}

/// A clock whose wall time panics: the worker task, which reads it on its
/// own task, dies.
struct PanicsOnNow;

#[async_trait::async_trait]
impl sc_cron::Clock for PanicsOnNow {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        panic!("injected: the worker's clock panics")
    }

    async fn sleep_until(&self, _target: chrono::DateTime<chrono::Utc>) {}
}

/// A worker that dies without a signal ends `cron run` with the fault
/// reported, instead of waiting forever (B8Q1-049). Planted negative: the
/// healthy worker of the test above runs until its stop flag.
#[tokio::test(flavor = "multi_thread")]
async fn cron_run_ends_when_its_worker_dies() {
    let repo = Repo::new(vec![]);
    let schedules = Schedules::new(
        Arc::clone(&repo.store),
        Arc::new(PanicsOnNow),
        Arc::new(sc_cron::SystemJitter),
        Arc::clone(&repo.cache),
        repo.files(),
    );
    let stopped = tokio::time::timeout(
        Duration::from_secs(20),
        schedules.cron_run(StopFlag::new(), Duration::from_millis(100), || {}),
    )
    .await
    .expect("cron run returns once its worker died");
    assert!(stopped.worker_ended, "{stopped:?}");
}

/// A built-in run whose hook overruns is ended by the run itself at the
/// hook's deadline, its lease released, before sc-cron's own timeout for the
/// row (`budget_ms + 10000`, on the injected clock) can kill it: sc-cron
/// records the run `Ok`, not `TimedOut` (acceptance 8, B8Q1-015). Planted
/// negative: the row's timeout is the budget plus the full 10 s margin.
#[tokio::test(flavor = "multi_thread")]
async fn an_overrunning_hook_ends_its_run_before_sc_crons_timeout() {
    let repo = Repo::new(vec![]);
    let hook = common::HookStub::new();
    hook.mode("hang");
    let (ready, _life) = (gate_file(hook.fifo("ready")), gate_file(hook.fifo("life")));
    let set = ScheduleSet {
        budget_ms: Some(2_000),
        hook_reserve_ms: Some(1_000),
        post_sync: Some("consumer.hooks:after".into()),
        python: Some(hook.python.clone()),
        ..every_minute()
    };
    let reply = repo.schedules().set(&set).await.unwrap();
    assert_eq!(reply.timeout_ms, 12_000, "budget_ms + 10000");
    step(&repo, 60);
    let schedules = repo.schedules();
    let run = tokio::spawn(async move { schedules.cron_run_once(StopFlag::new()).await });
    tokio::task::spawn_blocking(move || ready.recv_timeout(Duration::from_secs(20)).unwrap())
        .await
        .unwrap();
    // Most of sc-cron's timeout passes on its clock while the hook hangs.
    step(&repo, 9);
    let once = run.await.unwrap().unwrap();
    assert_eq!(once.ran[0].outcome, "ok", "{:?}", once.ran);
    assert!(once.error().is_none());
    assert!(
        repo.cache.lock().unwrap().lease().unwrap().is_none(),
        "released by the run"
    );
    let v = show(&repo).await;
    assert_eq!(
        v["hook"]["consecutive_failures"], 1,
        "the hook's timeout is the hook's failure: {v}"
    );
}
