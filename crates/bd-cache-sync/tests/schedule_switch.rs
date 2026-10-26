//! The schedule rows and the switch (REQ-BDC-0720, 0722, 0724, 0731;
//! ADR-SYNC-0008, ADR-BDC-0014) on the real store, each rule with its
//! planted negative. The Python row runs a stub interpreter (Unix).

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
use std::sync::atomic::{AtomicBool, Ordering};

use bd_cache_model::{CacheError, ScheduleReason, TickRunReason};
use bd_cache_sync::{
    BUILTIN_ROW, PYTHON_ROW, ScheduleSet, SyncError, SyncMode, UsePython, schedule_clear,
    schedule_enable, schedule_set, schedule_show, sync_use_builtin, sync_use_python, tick,
};
use common::repo::{PyStub, Repo};

fn set(expr: &str) -> ScheduleSet {
    ScheduleSet {
        expr: expr.to_owned(),
        ..ScheduleSet::default()
    }
}

fn use_python(stub: &PyStub) -> UsePython {
    UsePython {
        python: stub.python.clone(),
        module: "consumer.sync".to_owned(),
        cron: None,
        timeout_ms: Some(5_000),
        env: vec![],
    }
}

fn reason(e: &CacheError) -> (&'static str, ScheduleReason) {
    match e {
        CacheError::ScheduleInvalid { field, reason, .. } => (field, *reason),
        other => panic!("not SCHEDULE_INVALID: {other:?}"),
    }
}

fn run_reason(e: &CacheError) -> (TickRunReason, Option<bool>) {
    match e {
        CacheError::TickRunFailed { reason, restored, .. } => (*reason, *restored),
        other => panic!("not TICK_RUN_FAILED: {other:?}"),
    }
}

/// `schedule set/show/enable/disable/clear` round-trip; a second `set`
/// replaces the one built-in row in place; the row's sc-cron timeout is
/// `budget_ms + 10000` (REQ-BDC-0720).
#[tokio::test(flavor = "multi_thread")]
async fn schedule_set_show_enable_disable_clear_round_trip() {
    let repo = Repo::new(vec![]);
    let first = schedule_set(&repo.scheduler, &set("*/30 * * * * *")).await.unwrap();
    assert_eq!((first["enabled"].clone(), first["mode"].clone()), (true.into(), "builtin".into()));
    assert_eq!(first["timeout_ms"], 35_000);
    let again = schedule_set(&repo.scheduler, &ScheduleSet { budget_ms: Some(30_000), ..set("0 * * * * *") }).await.unwrap();
    assert_eq!(again["generation"], first["generation"].as_u64().unwrap() + 1, "replaced in place");
    assert_eq!(again["timeout_ms"], 40_000);
    let show = schedule_show(&repo.store, &repo.cache).await.unwrap();
    assert_eq!(show["mode"], "builtin");
    assert_eq!(show["rows"]["builtin"]["expr"], "0 * * * * *");
    assert!(show["rows"]["python"].is_null());
    let active = sc_cron::ScheduleStore::list_active(repo.store.as_ref()).unwrap();
    assert_eq!(active.iter().filter(|r| r.spec.name.as_str() == BUILTIN_ROW).count(), 1, "one built-in row");
    schedule_enable(&repo.scheduler, false).await.unwrap();
    assert_eq!(repo.flags(), (Some(false), None));
    // `schedule set` keeps a disabled row disabled.
    schedule_set(&repo.scheduler, &set("*/30 * * * * *")).await.unwrap();
    assert_eq!(repo.flags(), (Some(false), None));
    let show = schedule_show(&repo.store, &repo.cache).await.unwrap();
    assert!(show["flags"].as_array().unwrap().contains(&"both_disabled".into()));
    schedule_enable(&repo.scheduler, true).await.unwrap();
    assert_eq!(repo.flags(), (Some(true), None));
    assert_eq!(schedule_clear(&repo.scheduler).await.unwrap()["cleared"], true);
    assert_eq!(repo.flags(), (None, None));
}

/// Each `schedule set` refusal is `SCHEDULE_INVALID` naming its field, and
/// nothing is written; planted negative: a valid set writes.
#[tokio::test(flavor = "multi_thread")]
async fn each_schedule_set_refusal_names_its_field_and_writes_nothing() {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    let hook = |python: &std::path::Path, target: &str| ScheduleSet {
        post_sync: Some(target.to_owned()),
        python: Some(python.to_path_buf()),
        ..set("*/30 * * * * *")
    };
    let cases: Vec<(ScheduleSet, &str, ScheduleReason)> = vec![
        (set("not cron"), "cron", ScheduleReason::OutOfRange),
        (ScheduleSet { budget_ms: Some(999), ..set("*/30 * * * * *") }, "budget_ms", ScheduleReason::OutOfRange),
        (ScheduleSet { hook_reserve_ms: Some(24_500), ..set("*/30 * * * * *") }, "hook_reserve_ms", ScheduleReason::OutOfRange),
        (ScheduleSet { hook_reserve_ms: Some(500), ..hook(&stub.python, "m:f") }, "hook_reserve_ms", ScheduleReason::OutOfRange),
        (ScheduleSet { post_sync: Some("m:f".into()), ..set("*/30 * * * * *") }, "python", ScheduleReason::NotAbsolute),
        (hook(std::path::Path::new("python3"), "m:f"), "python", ScheduleReason::NotAbsolute),
        (hook(repo.dir.path(), "m:f"), "python", ScheduleReason::NotExecutable),
        (hook(&stub.python, "not-a-module:f"), "post_sync", ScheduleReason::BadModule),
        (hook(&stub.python, "m"), "post_sync", ScheduleReason::BadModule),
    ];
    for (req, field, why) in cases {
        let e = schedule_set(&repo.scheduler, &req).await.unwrap_err();
        assert_eq!(reason(&e), (field, why), "{req:?}");
    }
    stub.set("noimport", "");
    let e = schedule_set(&repo.scheduler, &hook(&stub.python, "m:f")).await.unwrap_err();
    assert_eq!(reason(&e), ("python", ScheduleReason::ImportFailed));
    assert_eq!(repo.flags(), (None, None), "nothing written");
    stub.unset("noimport");
    schedule_set(&repo.scheduler, &hook(&stub.python, "m:f")).await.unwrap();
    assert_eq!(repo.flags(), (Some(true), None));
}

/// `use-python` leaves exactly the Python row enabled and runs it at once,
/// its reply carrying the run id, the exit status and stderr; the script
/// gets `cache_path`, `beads_dir` and `run_id` and only the allowlisted
/// environment; `use-builtin` reverses it; `schedule enable` while the
/// Python row is enabled is refused, and `schedule set` then writes the
/// built-in row disabled.
#[tokio::test(flavor = "multi_thread")]
async fn use_python_switches_and_runs_at_once_and_use_builtin_reverses_it() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo.scheduler, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    stub.set("tick", "stderr:hello from the script");
    let reply = sync_use_python(&repo.scheduler, &repo.ctx(), &use_python(&stub)).await.unwrap();
    assert_eq!(repo.flags(), (Some(false), Some(true)));
    assert_eq!(reply["exit_status"], 0);
    assert_eq!(reply["stderr"], "hello from the script");
    let run_id = reply["run_id"].as_str().unwrap();
    assert_eq!((stub.count("stop"), stub.count("tick")), (1, 1), "cron.stop then cron.tick");
    let req = stub.request("tick", 1);
    assert_eq!(req["protocol"], "sc-cron.hook.v1");
    assert_eq!(req["payload"]["run_id"], run_id);
    assert_eq!(req["payload"]["cache_path"], repo.cache_path.display().to_string());
    assert_eq!(req["payload"]["beads_dir"], repo.beads_dir.display().to_string());
    for line in stub.env("tick").lines() {
        let name = line.split('=').next().unwrap();
        assert!(["PATH", "HOME", "LANG", "BEADS_DIR", "BD_CACHE_PATH", "PWD", "SHLVL", "_", "OLDPWD"].contains(&name)
            || name.starts_with("LC_"), "{name} leaked into the script");
    }
    assert_eq!(repo.store_lease_free(), true);

    let e = schedule_enable(&repo.scheduler, true).await.unwrap_err();
    assert_eq!(reason(&e), ("schedule", ScheduleReason::PythonSyncEnabled));
    schedule_set(&repo.scheduler, &set("0 * * * * *")).await.unwrap();
    assert_eq!(repo.flags(), (Some(false), Some(true)), "kept disabled while mode is Python");

    let show = schedule_show(&repo.store, &repo.cache).await.unwrap();
    assert_eq!(show["mode"], "python");
    assert_eq!(show["rows"]["python"]["module"], "consumer.sync");
    sync_use_builtin(&repo.scheduler).await.unwrap();
    assert_eq!(repo.flags(), (Some(true), Some(false)));
}

trait LeaseFree {
    fn store_lease_free(&self) -> bool;
}

impl LeaseFree for Repo {
    fn store_lease_free(&self) -> bool {
        self.cache.lock().unwrap().lease().unwrap().is_none()
    }
}

/// A failed immediate run fails the command with `TICK_RUN_FAILED` and
/// restores the mode before it: exit 1, a timeout, a `cron.stop` answer of
/// `disable`, a group-writable cache file, a stop signal (REQ-BDC-0724).
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_immediate_run_restores_the_mode_before_the_command() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo.scheduler, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let cases: Vec<(&str, &str, TickRunReason)> = vec![
        ("tick", "exit:1", TickRunReason::Exit),
        ("tick", "sleep", TickRunReason::Timeout),
        ("stop", r#"{"action": "disable", "reason": "not today"}"#, TickRunReason::StopAnswer),
    ];
    for (file, text, why) in cases {
        stub.set(file, text);
        let req = UsePython { timeout_ms: Some(1_000), ..use_python(&stub) };
        let e = sync_use_python(&repo.scheduler, &repo.ctx(), &req).await.unwrap_err();
        assert_eq!(run_reason(&e), (why, Some(true)), "{text}");
        assert_eq!(repo.flags(), (Some(true), Some(false)), "back to builtin after {text}");
        assert!(repo.store_lease_free());
        stub.unset(file);
    }
    std::fs::set_permissions(&repo.cache_path, std::fs::Permissions::from_mode(0o664)).unwrap();
    let e = sync_use_python(&repo.scheduler, &repo.ctx(), &use_python(&stub)).await.unwrap_err();
    assert_eq!(run_reason(&e).0, TickRunReason::Untrusted);
    assert_eq!(repo.flags(), (Some(true), Some(false)));
    std::fs::set_permissions(&repo.cache_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    // A signal mid-run kills the script's tree and restores the state.
    stub.set("tick", "sleep");
    let ctx = repo.ctx();
    let kill = ctx.kill.clone();
    let stopper = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        kill.stop();
    });
    let e = sync_use_python(&repo.scheduler, &ctx, &use_python(&stub)).await.unwrap_err();
    stopper.await.unwrap();
    assert_eq!(run_reason(&e), (TickRunReason::Signal, Some(true)));
    assert_eq!(repo.flags(), (Some(true), Some(false)));
    // Planted negative: a script that succeeds keeps the Python row.
    stub.unset("tick");
    sync_use_python(&repo.scheduler, &repo.ctx(), &use_python(&stub)).await.unwrap();
    assert_eq!(repo.flags(), (Some(false), Some(true)));
}

/// With no built-in row a failure leaves mode `none` and `use-builtin` is
/// `SCHEDULE_INVALID` (`no_builtin_row`); `--cron` is then required.
#[tokio::test(flavor = "multi_thread")]
async fn with_no_builtin_row_a_failure_leaves_mode_none() {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    let e = sync_use_python(&repo.scheduler, &repo.ctx(), &use_python(&stub)).await.unwrap_err();
    assert_eq!(reason(&e), ("cron", ScheduleReason::CronRequired));
    stub.set("tick", "exit:1");
    let req = UsePython { cron: Some("*/30 * * * * *".into()), ..use_python(&stub) };
    let e = sync_use_python(&repo.scheduler, &repo.ctx(), &req).await.unwrap_err();
    assert_eq!(run_reason(&e), (TickRunReason::Exit, Some(true)));
    assert_eq!(repo.flags(), (None, Some(false)), "mode none");
    let e = sync_use_builtin(&repo.scheduler).await.unwrap_err();
    assert_eq!(reason(&e), ("schedule", ScheduleReason::NoBuiltinRow));
}

/// A second `use-python` with a new script replaces the one row in place
/// (generation raised); a failed re-run from mode Python restores the old
/// script.
#[tokio::test(flavor = "multi_thread")]
async fn a_re_run_replaces_the_python_row_in_place_and_a_failed_one_restores_the_old_script() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo.scheduler, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    sync_use_python(&repo.scheduler, &repo.ctx(), &use_python(&stub)).await.unwrap();
    let python_row = |repo: &Repo| bd_cache_sync::Rows::read(repo.store.as_ref()).unwrap().python.unwrap();
    let first = python_row(&repo);
    let req = UsePython { module: "consumer.sync_v2".into(), ..use_python(&stub) };
    sync_use_python(&repo.scheduler, &repo.ctx(), &req).await.unwrap();
    let second = python_row(&repo);
    assert_eq!(second.id, first.id, "the same row");
    assert!(second.generation > first.generation, "a run of the old one is fenced");
    stub.set("tick", "exit:3");
    let req = UsePython { module: "consumer.broken".into(), ..use_python(&stub) };
    let e = sync_use_python(&repo.scheduler, &repo.ctx(), &req).await.unwrap_err();
    assert_eq!(run_reason(&e).1, Some(true));
    let show = schedule_show(&repo.store, &repo.cache).await.unwrap();
    assert_eq!(show["rows"]["python"]["module"], "consumer.sync_v2", "the old script is back");
    assert_eq!(show["mode"], "python");
}

/// A second connection polling while the Python row is created and the
/// rows are switched never sees both enabled (REQ-BDC-0724).
#[tokio::test(flavor = "multi_thread")]
async fn a_second_connection_never_sees_both_rows_enabled() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo.scheduler, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let done = Arc::new(AtomicBool::new(false));
    let path = repo.cache_path.clone();
    let poll_done = Arc::clone(&done);
    let poller = std::thread::spawn(move || {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
        let mut seen = 0_u64;
        while !poll_done.load(Ordering::SeqCst) {
            let both: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sc_cron_schedule WHERE enabled = 1 AND ended_at IS NULL \
                     AND name IN (?1, ?2)",
                    [BUILTIN_ROW, PYTHON_ROW],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(both <= 1, "both sync rows enabled at once");
            seen += 1;
        }
        seen
    });
    for _ in 0..3 {
        sync_use_python(&repo.scheduler, &repo.ctx(), &use_python(&stub)).await.unwrap();
        sync_use_builtin(&repo.scheduler).await.unwrap();
    }
    done.store(true, Ordering::SeqCst);
    assert!(poller.join().unwrap() > 0, "the poller read the table");
}

/// A crash between the mode commit and the run leaves one row enabled, and
/// `schedule show` flags the Python row as never run.
#[tokio::test(flavor = "multi_thread")]
async fn a_python_row_that_never_ran_is_flagged() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo.scheduler, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let payload = bd_cache_sync::PythonPayload::new(
        bd_cache_sync::TrustedInterpreter::new(&stub.python, "consumer.sync", "module").unwrap(),
        repo.cache_path.clone(),
        None,
        bd_cache_sync::EnvNames::default(),
        None,
    )
    .unwrap();
    let spec = sc_cron::ScheduleSpec::new(PYTHON_ROW, "*/30 * * * * *", std::time::Duration::from_secs(60),
        bd_cache_sync::SyncPayload::Python(payload).to_json()).unwrap();
    // The commit alone, as a crash after it would leave it.
    repo.scheduler
        .replace_and_set_enabled(spec, true, &[(sc_cron::ScheduleName::new(BUILTIN_ROW).unwrap(), false)])
        .unwrap();
    assert_eq!(repo.flags(), (Some(false), Some(true)), "one row enabled");
    let show = schedule_show(&repo.store, &repo.cache).await.unwrap();
    assert!(show["flags"].as_array().unwrap().contains(&"python_never_run".into()), "{show}");
}

/// While the Python row is enabled an on-demand tick is `PRECONDITION` and
/// starts no bd process; planted negative: in mode builtin it runs.
#[tokio::test(flavor = "multi_thread")]
async fn a_tick_while_the_python_row_is_enabled_is_refused_and_starts_no_bd() {
    let repo = Repo::new(vec![common::label_scope("lbl", "area")]);
    schedule_set(&repo.scheduler, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    sync_use_python(&repo.scheduler, &repo.ctx(), &use_python(&stub)).await.unwrap();
    repo.bd.clear_calls();
    let e = tick(&repo.store, &repo.cache, &repo.run_ctx()).await.unwrap_err();
    assert!(matches!(e, SyncError::PythonSyncEnabled(_)), "{e:?}");
    assert_eq!(e.code(), "BD_CACHE.PRECONDITION");
    assert!(repo.bd.calls().is_empty(), "no bd process");
    sync_use_builtin(&repo.scheduler).await.unwrap();
    let reply = tick(&repo.store, &repo.cache, &repo.run_ctx()).await.unwrap();
    assert_eq!(reply.scopes.len(), 1);
    assert!(!repo.bd.calls().is_empty());
}

/// `use-python` while another run holds the lease is `TICK_IN_FLIGHT` and
/// changes nothing.
#[tokio::test(flavor = "multi_thread")]
async fn use_python_during_a_run_in_flight_is_tick_in_flight() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo.scheduler, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    {
        let mut cache = repo.cache.lock().unwrap();
        let clock = cache.clock();
        let run_id = bd_cache_sync::new_run_id(&*clock);
        let taken = bd_cache_sync::TickLease::take(&mut cache, run_id, sc_beads::Deadline::after(&*clock, std::time::Duration::from_secs(30))).unwrap();
        let bd_cache_sync::LeaseTaken::Taken(lease) = taken else { panic!() };
        std::mem::forget(lease);
    }
    let e = sync_use_python(&repo.scheduler, &repo.ctx(), &use_python(&stub)).await.unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.TICK_IN_FLIGHT");
    assert_eq!(repo.flags(), (Some(true), None));
    let _ = SyncMode::None;
}

/// A cache file whose `sc_cron_schedule_version` row names another version
/// fails with a typed `STORE_FAILED` before any schedule is served (ADR-BDC-0014).
#[test]
fn a_schedule_table_of_another_version_is_store_failed() {
    let Repo { dir, bd, clock, cron_clock, cache, store, scheduler, cache_path: path, beads_dir: _ } =
        Repo::new(vec![]);
    let root = dir.path().to_path_buf();
    drop((bd, clock, cron_clock, cache, store, scheduler));
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("UPDATE meta SET value = '99' WHERE key = 'sc_cron_schedule_version'", []).unwrap();
    drop(conn);
    let clock = Arc::new(sc_beads::testing::ManualClock::new(common::START));
    let fake = sc_bd_cli::testing::FakeBd::new(Arc::clone(&clock));
    let composed = bd_cache_sync::compose_with_config_and_backend(
        &root,
        bd_cache_model::RepoConfig::default(),
        |_| sc_bd_cli::BdCli::with_invoker(Box::new(fake)),
        Arc::new(sc_bd_cli::ProjectLocks::new()),
        clock,
        None,
    )
    .unwrap();
    let e = composed.schedule_store().unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.STORE_FAILED");
    assert!(e.to_string().contains("sc_cron_schedule"), "{e}");
}
