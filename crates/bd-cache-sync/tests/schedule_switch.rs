//! The schedule rows and the switch (REQ-BDC-0720, 0722, 0724, 0731;
//! ADR-SYNC-0008, ADR-BDC-0014) on the real store, each rule with its
//! planted negative. The Python row runs a stub interpreter (Unix).

#![cfg(unix)]
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bd_cache_model::{CacheError, ScheduleReason, TickRunReason};
use bd_cache_sync::{
    BUILTIN_ROW, PYTHON_ROW, ScheduleSet, StopFlag, SyncError, SyncMode, UsePython,
};
use common::repo::{PyStub, Repo};
use serde_json::Value;

fn set(expr: &str) -> ScheduleSet {
    ScheduleSet {
        expr: expr.to_owned(),
        budget_ms: None,
        hook_reserve_ms: None,
        post_sync: None,
        python: None,
        env: vec![],
    }
}

fn json<T: serde::Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap()
}

async fn schedule_set(repo: &Repo, req: &ScheduleSet) -> Result<Value, CacheError> {
    repo.schedules().set(req).await.map(json)
}

async fn schedule_enable(repo: &Repo, enabled: bool) -> Result<Value, CacheError> {
    repo.schedules().enable(enabled).await.map(json)
}

async fn schedule_clear(repo: &Repo) -> Result<Value, CacheError> {
    repo.schedules().clear().await.map(json)
}

async fn schedule_show(repo: &Repo) -> Result<Value, CacheError> {
    repo.schedules().show().await.map(json)
}

async fn sync_use_python(
    repo: &Repo,
    kill: StopFlag,
    req: &UsePython,
) -> Result<Value, CacheError> {
    repo.schedules().use_python(req, kill).await.map(json)
}

async fn sync_use_builtin(repo: &Repo) -> Result<Value, CacheError> {
    repo.schedules().use_builtin().await.map(json)
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
        CacheError::TickRunFailed {
            reason, restored, ..
        } => (*reason, *restored),
        other => panic!("not TICK_RUN_FAILED: {other:?}"),
    }
}

/// `schedule set/show/enable/disable/clear` round-trip; a second `set`
/// replaces the one built-in row in place; the row's sc-cron timeout is
/// `budget_ms + 10000` (REQ-BDC-0720).
#[tokio::test(flavor = "multi_thread")]
async fn schedule_set_show_enable_disable_clear_round_trip() {
    let repo = Repo::new(vec![]);
    let first = schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    assert_eq!(
        (first["enabled"].clone(), first["mode"].clone()),
        (true.into(), "builtin".into())
    );
    assert_eq!(first["timeout_ms"], 35_000);
    let again = schedule_set(
        &repo,
        &ScheduleSet {
            budget_ms: Some(30_000),
            ..set("0 * * * * *")
        },
    )
    .await
    .unwrap();
    assert_eq!(
        again["generation"],
        first["generation"].as_u64().unwrap() + 1,
        "replaced in place"
    );
    assert_eq!(again["timeout_ms"], 40_000);
    let show = schedule_show(&repo).await.unwrap();
    assert_eq!(show["mode"], "builtin");
    assert_eq!(show["rows"]["builtin"]["expr"], "0 * * * * *");
    assert!(show["rows"]["python"].is_null());
    let active = sc_cron::ScheduleStore::list_active(repo.store.as_ref()).unwrap();
    assert_eq!(
        active
            .iter()
            .filter(|r| r.spec.name.as_str() == BUILTIN_ROW)
            .count(),
        1,
        "one built-in row"
    );
    schedule_enable(&repo, false).await.unwrap();
    assert_eq!(repo.flags(), (Some(false), None));
    // `schedule set` keeps a disabled row disabled.
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    assert_eq!(repo.flags(), (Some(false), None));
    let show = schedule_show(&repo).await.unwrap();
    assert!(
        show["flags"]
            .as_array()
            .unwrap()
            .contains(&"both_disabled".into())
    );
    schedule_enable(&repo, true).await.unwrap();
    assert_eq!(repo.flags(), (Some(true), None));
    assert_eq!(schedule_clear(&repo).await.unwrap()["cleared"], true);
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
        (
            ScheduleSet {
                budget_ms: Some(999),
                ..set("*/30 * * * * *")
            },
            "budget_ms",
            ScheduleReason::OutOfRange,
        ),
        (
            ScheduleSet {
                hook_reserve_ms: Some(24_500),
                ..set("*/30 * * * * *")
            },
            "hook_reserve_ms",
            ScheduleReason::OutOfRange,
        ),
        (
            ScheduleSet {
                hook_reserve_ms: Some(500),
                ..hook(&stub.python, "m:f")
            },
            "hook_reserve_ms",
            ScheduleReason::OutOfRange,
        ),
        (
            ScheduleSet {
                post_sync: Some("m:f".into()),
                ..set("*/30 * * * * *")
            },
            "python",
            ScheduleReason::NotAbsolute,
        ),
        (
            hook(std::path::Path::new("python3"), "m:f"),
            "python",
            ScheduleReason::NotAbsolute,
        ),
        (
            hook(repo.dir.path(), "m:f"),
            "python",
            ScheduleReason::NotExecutable,
        ),
        (
            hook(&stub.python, "not-a-module:f"),
            "post_sync",
            ScheduleReason::BadModule,
        ),
        (
            hook(&stub.python, "m"),
            "post_sync",
            ScheduleReason::BadModule,
        ),
    ];
    for (req, field, why) in cases {
        let e = schedule_set(&repo, &req).await.unwrap_err();
        assert_eq!(reason(&e), (field, why), "{req:?}");
    }
    stub.set("noimport", "");
    let e = schedule_set(&repo, &hook(&stub.python, "m:f"))
        .await
        .unwrap_err();
    assert_eq!(reason(&e), ("python", ScheduleReason::ImportFailed));
    assert_eq!(repo.flags(), (None, None), "nothing written");
    stub.unset("noimport");
    schedule_set(&repo, &hook(&stub.python, "m:f"))
        .await
        .unwrap();
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
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    stub.set("tick", "stderr:hello from the script");
    let reply = sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap();
    assert_eq!(repo.flags(), (Some(false), Some(true)));
    assert_eq!(reply["exit_status"], 0);
    assert_eq!(reply["stderr"], "hello from the script");
    let run_id = reply["run_id"].as_str().unwrap();
    assert_eq!(
        (stub.count("stop"), stub.count("tick")),
        (1, 1),
        "cron.stop then cron.tick"
    );
    let req = stub.request("tick", 1);
    assert_eq!(req["protocol"], "sc-cron.hook.v1");
    assert_eq!(req["payload"]["run_id"], run_id);
    assert_eq!(
        req["payload"]["cache_path"],
        repo.cache_path.display().to_string()
    );
    assert_eq!(
        req["payload"]["beads_dir"],
        repo.beads_dir.display().to_string()
    );
    for line in stub.env("tick").lines() {
        let name = line.split('=').next().unwrap();
        assert!(
            [
                "PATH",
                "HOME",
                "LANG",
                "BEADS_DIR",
                "BD_CACHE_PATH",
                "PWD",
                "SHLVL",
                "_",
                "OLDPWD"
            ]
            .contains(&name)
                || name.starts_with("LC_"),
            "{name} leaked into the script"
        );
    }
    assert!(repo.store_lease_free());

    let e = schedule_enable(&repo, true).await.unwrap_err();
    assert_eq!(reason(&e), ("schedule", ScheduleReason::PythonSyncEnabled));
    schedule_set(&repo, &set("0 * * * * *")).await.unwrap();
    assert_eq!(
        repo.flags(),
        (Some(false), Some(true)),
        "kept disabled while mode is Python"
    );

    let show = schedule_show(&repo).await.unwrap();
    assert_eq!(show["mode"], "python");
    assert_eq!(show["rows"]["python"]["module"], "consumer.sync");
    sync_use_builtin(&repo).await.unwrap();
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
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let cases: Vec<(&str, &str, TickRunReason)> = vec![
        ("tick", "exit:1", TickRunReason::Exit),
        ("tick", "sleep", TickRunReason::Timeout),
        (
            "stop",
            r#"{"action": "disable", "reason": "not today"}"#,
            TickRunReason::StopAnswer,
        ),
    ];
    for (file, text, why) in cases {
        stub.set(file, text);
        let req = UsePython {
            timeout_ms: Some(1_000),
            ..use_python(&stub)
        };
        let e = sync_use_python(&repo, StopFlag::new(), &req)
            .await
            .unwrap_err();
        assert_eq!(run_reason(&e), (why, Some(true)), "{text}");
        assert_eq!(
            repo.flags(),
            (Some(true), Some(false)),
            "back to builtin after {text}"
        );
        assert!(repo.store_lease_free());
        stub.unset(file);
    }
    std::fs::set_permissions(&repo.cache_path, std::fs::Permissions::from_mode(0o664)).unwrap();
    let e = sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap_err();
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
    let e = sync_use_python(&repo, ctx.kill.clone(), &use_python(&stub))
        .await
        .unwrap_err();
    stopper.await.unwrap();
    assert_eq!(run_reason(&e), (TickRunReason::Signal, Some(true)));
    assert_eq!(repo.flags(), (Some(true), Some(false)));
    // Planted negative: a script that succeeds keeps the Python row.
    stub.unset("tick");
    sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap();
    assert_eq!(repo.flags(), (Some(false), Some(true)));
}

/// With no built-in row a failure leaves mode `none` and `use-builtin` is
/// `SCHEDULE_INVALID` (`no_builtin_row`); `--cron` is then required.
#[tokio::test(flavor = "multi_thread")]
async fn with_no_builtin_row_a_failure_leaves_mode_none() {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    let e = sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap_err();
    assert_eq!(reason(&e), ("cron", ScheduleReason::CronRequired));
    stub.set("tick", "exit:1");
    let req = UsePython {
        cron: Some("*/30 * * * * *".into()),
        ..use_python(&stub)
    };
    let e = sync_use_python(&repo, StopFlag::new(), &req)
        .await
        .unwrap_err();
    assert_eq!(run_reason(&e), (TickRunReason::Exit, Some(true)));
    assert_eq!(repo.flags(), (None, Some(false)), "mode none");
    let e = sync_use_builtin(&repo).await.unwrap_err();
    assert_eq!(reason(&e), ("schedule", ScheduleReason::NoBuiltinRow));
}

/// A second `use-python` with a new script replaces the one row in place
/// (generation raised); a failed re-run from mode Python restores the old
/// script.
#[tokio::test(flavor = "multi_thread")]
async fn a_re_run_replaces_the_python_row_in_place_and_a_failed_one_restores_the_old_script() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap();
    let python_row = |repo: &Repo| {
        bd_cache_sync::Rows::read(repo.store.as_ref())
            .unwrap()
            .python
            .unwrap()
    };
    let first = python_row(&repo);
    let req = UsePython {
        module: "consumer.sync_v2".into(),
        ..use_python(&stub)
    };
    sync_use_python(&repo, StopFlag::new(), &req).await.unwrap();
    let second = python_row(&repo);
    assert_eq!(second.id, first.id, "the same row");
    assert!(
        second.generation > first.generation,
        "a run of the old one is fenced"
    );
    stub.set("tick", "exit:3");
    let req = UsePython {
        module: "consumer.broken".into(),
        ..use_python(&stub)
    };
    let e = sync_use_python(&repo, StopFlag::new(), &req)
        .await
        .unwrap_err();
    assert_eq!(run_reason(&e).1, Some(true));
    let show = schedule_show(&repo).await.unwrap();
    assert_eq!(
        show["rows"]["python"]["module"], "consumer.sync_v2",
        "the old script is back"
    );
    assert_eq!(show["mode"], "python");
}

/// A second connection polling while the Python row is created and the
/// rows are switched never sees both enabled (REQ-BDC-0724).
#[tokio::test(flavor = "multi_thread")]
async fn a_second_connection_never_sees_both_rows_enabled() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let done = Arc::new(AtomicBool::new(false));
    let path = repo.cache_path.clone();
    let poll_done = Arc::clone(&done);
    let poller = std::thread::spawn(move || {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .unwrap();
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
        sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
            .await
            .unwrap();
        sync_use_builtin(&repo).await.unwrap();
    }
    done.store(true, Ordering::SeqCst);
    assert!(poller.join().unwrap() > 0, "the poller read the table");
}

/// A crash between the mode commit and the run leaves one row enabled, and
/// `schedule show` flags the Python row as never run.
#[tokio::test(flavor = "multi_thread")]
async fn a_python_row_that_never_ran_is_flagged() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let payload = bd_cache_sync::PythonPayload::new(
        bd_cache_sync::TrustedInterpreter::new(&stub.python, "consumer.sync").unwrap(),
        repo.cache_path.clone(),
        None,
        bd_cache_sync::EnvNames::default(),
        None,
    )
    .unwrap();
    let spec = sc_cron::ScheduleSpec::new(
        PYTHON_ROW,
        "*/30 * * * * *",
        std::time::Duration::from_secs(60),
        bd_cache_sync::SyncPayload::Python(payload).to_json(),
    )
    .unwrap();
    // The commit alone, as a crash after it would leave it.
    repo.scheduler
        .replace_and_set_enabled(
            spec,
            true,
            &[(sc_cron::ScheduleName::new(BUILTIN_ROW).unwrap(), false)],
        )
        .unwrap();
    assert_eq!(repo.flags(), (Some(false), Some(true)), "one row enabled");
    let show = schedule_show(&repo).await.unwrap();
    assert!(
        show["flags"]
            .as_array()
            .unwrap()
            .contains(&"python_never_run".into()),
        "{show}"
    );
}

/// While the Python row is enabled an on-demand tick is `PRECONDITION` and
/// starts no bd process; planted negative: in mode builtin it runs.
#[tokio::test(flavor = "multi_thread")]
async fn a_tick_while_the_python_row_is_enabled_is_refused_and_starts_no_bd() {
    let repo = Repo::new(vec![common::label_scope("lbl", "area")]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap();
    repo.bd.clear_calls();
    let e = repo.schedules().tick(StopFlag::new()).await.unwrap_err();
    assert!(matches!(e, SyncError::PythonSyncEnabled), "{e:?}");
    assert_eq!(e.code(), "BD_CACHE.PRECONDITION");
    assert!(repo.bd.calls().is_empty(), "no bd process");
    sync_use_builtin(&repo).await.unwrap();
    let reply = repo.schedules().tick(StopFlag::new()).await.unwrap();
    assert_eq!(reply.scopes.len(), 1);
    assert!(!repo.bd.calls().is_empty());
}

/// `use-python` while another run holds the lease is `TICK_IN_FLIGHT` and
/// changes nothing.
#[tokio::test(flavor = "multi_thread")]
async fn use_python_during_a_run_in_flight_is_tick_in_flight() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    {
        let mut cache = repo.cache.lock().unwrap();
        let clock = cache.clock();
        let run_id = bd_cache_sync::new_run_id(1).unwrap();
        let taken = bd_cache_sync::TickLease::take(
            &repo.cache,
            &mut cache,
            run_id,
            sc_beads::Deadline::after(&*clock, std::time::Duration::from_secs(30)),
        )
        .unwrap();
        let bd_cache_sync::LeaseTaken::Taken(lease) = taken else {
            panic!()
        };
        std::mem::forget(lease);
    }
    let e = sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.TICK_IN_FLIGHT");
    assert_eq!(repo.flags(), (Some(true), None));
    let _ = SyncMode::None;
}

/// A cache file whose `sc_cron_schedule_version` row names another version
/// fails with a typed `STORE_FAILED` before any schedule is served (ADR-BDC-0014).
#[test]
fn a_schedule_table_of_another_version_is_store_failed() {
    let Repo {
        dir,
        bd,
        clock,
        cron_clock,
        cache,
        store,
        scheduler,
        cache_path: path,
        beads_dir: _,
        down: _,
    } = Repo::new(vec![]);
    let root = dir.path().to_path_buf();
    drop((bd, clock, cron_clock, cache, store, scheduler));
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE meta SET value = '99' WHERE key = 'sc_cron_schedule_version'",
        [],
    )
    .unwrap();
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

/// Each `use-python` refusal of REQ-BDC-0724 is `SCHEDULE_INVALID` naming
/// its field, before anything is written (B8Q1-014): a module that is not a
/// dotted name, a timeout outside 1000-600000, a relative or non-executable
/// interpreter, an invalid `--env` name, an interpreter that cannot import
/// `bd_cache_rs`. Planted negative: the valid request switches.
#[tokio::test(flavor = "multi_thread")]
async fn each_use_python_refusal_names_its_field_and_writes_nothing() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let plain = std::env::temp_dir().join(format!("not-exec-{}", std::process::id()));
    std::fs::write(&plain, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
    let cases: Vec<(UsePython, &str, ScheduleReason)> = vec![
        (
            UsePython {
                module: "not-a-module".into(),
                ..use_python(&stub)
            },
            "module",
            ScheduleReason::BadModule,
        ),
        (
            UsePython {
                timeout_ms: Some(999),
                ..use_python(&stub)
            },
            "timeout_ms",
            ScheduleReason::OutOfRange,
        ),
        (
            UsePython {
                timeout_ms: Some(600_001),
                ..use_python(&stub)
            },
            "timeout_ms",
            ScheduleReason::OutOfRange,
        ),
        (
            UsePython {
                python: "python3".into(),
                ..use_python(&stub)
            },
            "python",
            ScheduleReason::NotAbsolute,
        ),
        (
            UsePython {
                python: plain.clone(),
                ..use_python(&stub)
            },
            "python",
            ScheduleReason::NotExecutable,
        ),
        (
            UsePython {
                env: vec!["1BAD".into()],
                ..use_python(&stub)
            },
            "env",
            ScheduleReason::OutOfRange,
        ),
    ];
    for (req, field, why) in cases {
        let e = sync_use_python(&repo, StopFlag::new(), &req)
            .await
            .unwrap_err();
        assert_eq!(reason(&e), (field, why), "{req:?}");
        assert_eq!(repo.flags(), (Some(true), None), "nothing written");
    }
    stub.set("noimport", "");
    let e = sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap_err();
    assert_eq!(reason(&e), ("python", ScheduleReason::ImportFailed));
    stub.unset("noimport");
    let _ = std::fs::remove_file(&plain);
    assert_eq!(
        (stub.count("stop"), stub.count("tick")),
        (0, 0),
        "no run started"
    );
    sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap();
    assert_eq!(repo.flags(), (Some(false), Some(true)));
}

/// An interpreter missing at run time (it passed every check, then
/// vanished) fails the run as `spawn` and restores the mode before the
/// command (B8Q1-014). Planted negative: present, it runs.
#[tokio::test(flavor = "multi_thread")]
async fn an_interpreter_missing_at_run_time_restores_the_prior_mode() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    stub.set("vanish", "");
    let e = sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap_err();
    assert_eq!(run_reason(&e), (TickRunReason::Spawn, Some(true)), "{e:?}");
    assert_eq!(repo.flags(), (Some(true), Some(false)), "back to builtin");
    assert!(repo.store_lease_free());
    let present = PyStub::new();
    sync_use_python(&repo, StopFlag::new(), &use_python(&present))
        .await
        .unwrap();
}

/// A failed restore (here: another command replaced the Python row during
/// the run, so its state stands) is `details.restored: false`, and
/// `use-builtin` then restores the built-in sync (B8Q1-014).
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_restore_says_restored_false_and_use_builtin_recovers() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let gate = stub.dir.path().join("stopgate");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&gate)
            .status()
            .unwrap()
            .success()
    );
    stub.set("tick", "exit:1");
    let run = {
        let schedules = repo.schedules();
        let req = use_python(&stub);
        tokio::spawn(async move { schedules.use_python(&req, StopFlag::new()).await })
    };
    let started = stub.dir.path().join("stop.started");
    common::wait_until("the immediate run's stop check", || started.exists());
    // Another command replaces the Python row while the run is in flight.
    let row = repo.rows().python.unwrap();
    repo.scheduler
        .replace_and_set_enabled(row.spec.clone(), true, &[])
        .unwrap();
    std::fs::write(&gate, "go").unwrap();
    let e = run.await.unwrap().unwrap_err();
    assert_eq!(run_reason(&e), (TickRunReason::Exit, Some(false)), "{e:?}");
    assert!(e.to_string().contains("could NOT be restored"), "{e}");
    assert_eq!(
        repo.flags(),
        (Some(false), Some(true)),
        "the later command's state stands"
    );
    sync_use_builtin(&repo).await.unwrap();
    assert_eq!(
        repo.flags(),
        (Some(true), Some(false)),
        "use-builtin recovers"
    );
}

/// The immediate run's timeout is measured on the injected clock: the
/// clock moved past the deadline during the stop check makes the tick time
/// out at once, with no real wait (B8Q1-014). Planted negative: the clock
/// not moved, the same run succeeds.
#[tokio::test(flavor = "multi_thread")]
async fn the_immediate_runs_timeout_is_on_the_injected_clock() {
    for advance in [true, false] {
        let repo = Repo::new(vec![]);
        schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
        let stub = PyStub::new();
        let gate = stub.dir.path().join("stopgate");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&gate)
                .status()
                .unwrap()
                .success()
        );
        let schedules = repo.schedules();
        let req = UsePython {
            timeout_ms: Some(60_000),
            ..use_python(&stub)
        };
        let begun = std::time::Instant::now();
        let run = tokio::spawn(async move { schedules.use_python(&req, StopFlag::new()).await });
        let started = stub.dir.path().join("stop.started");
        common::wait_until("the stop check", || started.exists());
        if advance {
            repo.cron_clock.advance(chrono::Duration::seconds(61));
        }
        std::fs::write(&gate, "go").unwrap();
        let result = run.await.unwrap();
        if advance {
            let e = result.unwrap_err();
            assert_eq!(
                run_reason(&e),
                (TickRunReason::Timeout, Some(true)),
                "{e:?}"
            );
            assert_eq!(stub.count("tick"), 0, "the tick never started");
            assert!(
                begun.elapsed() < std::time::Duration::from_secs(30),
                "no real wait"
            );
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
    }
}

/// With no built-in row and the Python row enabled, `schedule set` writes
/// the new built-in row disabled (B8Q1-014). Planted negative: with no
/// Python row, a new built-in row is enabled.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_builtin_row_is_written_disabled_while_the_python_row_is_enabled() {
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    let req = UsePython {
        cron: Some("*/30 * * * * *".into()),
        ..use_python(&stub)
    };
    sync_use_python(&repo, StopFlag::new(), &req).await.unwrap();
    assert_eq!(repo.flags(), (None, Some(true)));
    let reply = schedule_set(&repo, &set("0 * * * * *")).await.unwrap();
    assert_eq!(reply["enabled"], false);
    assert_eq!(
        repo.flags(),
        (Some(false), Some(true)),
        "never both enabled"
    );
    let fresh = Repo::new(vec![]);
    assert_eq!(
        schedule_set(&fresh, &set("0 * * * * *")).await.unwrap()["enabled"],
        true
    );
}

/// A completed `use-python` records its run on the Python row as a worker
/// run would: `schedule show` shows its run id and outcome, and does not
/// flag the row as never run (B8Q1-012). Planted negative: a row committed
/// without its run (a crash) is flagged (`a_python_row_that_never_ran_is_flagged`).
#[tokio::test(flavor = "multi_thread")]
async fn a_completed_use_python_is_recorded_on_its_row() {
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let reply = sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap();
    let show = schedule_show(&repo).await.unwrap();
    let python = &show["rows"]["python"];
    assert_eq!(python["run_id"], reply["run_id"], "{show}");
    assert_eq!(python["outcome"], "ok");
    assert!(python["last_run_at"].is_string());
    assert!(
        !show["flags"]
            .as_array()
            .unwrap()
            .contains(&"python_never_run".into()),
        "{show}"
    );
}

/// A run aborted while it waits to take the lease still releases the lease:
/// the drop-release is installed by `TickLease::take` itself, before the
/// lease exists (B8Q1-024). Planted negative: a lease forgotten outright
/// (no drop) stays held, which this check would see.
#[tokio::test(flavor = "multi_thread")]
async fn a_run_aborted_while_it_takes_the_lease_releases_it() {
    let repo = Repo::new(vec![]);
    // Another step holds the cache while the run starts: its take waits.
    let (held, holding) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let holder = {
        let cache = Arc::clone(&repo.cache);
        std::thread::spawn(move || {
            let _guard = cache.lock().unwrap();
            held.send(()).unwrap();
            let _ = released.recv();
        })
    };
    holding.recv().unwrap();
    let run = {
        let cache = Arc::clone(&repo.cache);
        let ctx = repo.run_ctx();
        tokio::spawn(async move {
            bd_cache_sync::run_builtin(
                &cache,
                &bd_cache_sync::BuiltinPayload::default(),
                &ctx,
                bd_cache_sync::new_run_id(7).unwrap(),
            )
            .await
        })
    };
    // The run's blocking step waits for the mutex; abort the run meanwhile.
    std::thread::sleep(std::time::Duration::from_millis(100));
    run.abort();
    let _ = run.await;
    release.send(()).unwrap();
    holder.join().unwrap();
    common::wait_until("the aborted run's lease release", || {
        repo.store_lease_free()
    });

    let mut cache = repo.cache.lock().unwrap();
    let clock = cache.clock();
    let taken = bd_cache_sync::TickLease::take(
        &repo.cache,
        &mut cache,
        bd_cache_sync::new_run_id(8).unwrap(),
        sc_beads::Deadline::after(&*clock, std::time::Duration::from_secs(30)),
    )
    .unwrap();
    let bd_cache_sync::LeaseTaken::Taken(lease) = taken else {
        panic!()
    };
    std::mem::forget(lease);
    assert!(
        cache.lease().unwrap().is_some(),
        "a forgotten lease stays held"
    );
}

/// Takes the mode lock file as a mode command inside its read-decide-write
/// window does (an exclusive `flock` on `<cache file stem>.mode.lock`), until
/// the returned handle drops.
fn hold_mode_lock(repo: &Repo) -> std::fs::File {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(repo.cache_path.with_extension("mode.lock"))
        .unwrap();
    file.lock().unwrap();
    file
}

/// Every mode command goes through the mode lock (REQ-BDC-0720, 0724,
/// B8Q1-005): while another holder is inside its read-decide-write window
/// (here the test, holding the lock file), the command neither finishes nor
/// writes; once the holder leaves, it completes. This is the window the
/// lock closes, forced open deterministically. Planted negative: with
/// `ModeLock::hold` a no-op, each command finishes (and `use-python` writes
/// the Python row) while the window is held, and the test fails.
#[tokio::test(flavor = "multi_thread")]
async fn every_mode_command_waits_while_another_holds_its_read_decide_write_window() {
    const HELD: std::time::Duration = std::time::Duration::from_millis(500);
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stub = PyStub::new();
    let names = [
        "schedule set",
        "schedule enable",
        "sync use-python",
        "sync use-builtin",
        "schedule enable (again)",
        "schedule clear",
    ];
    for (op, name) in names.into_iter().enumerate() {
        let flags = repo.flags();
        let window = hold_mode_lock(&repo);
        let schedules = repo.schedules();
        let req = use_python(&stub);
        let task = tokio::spawn(async move {
            match op {
                0 => schedules.set(&set("*/20 * * * * *")).await.map(|_| ()),
                1 => schedules.enable(false).await.map(|_| ()),
                2 => schedules
                    .use_python(&req, StopFlag::new())
                    .await
                    .map(|_| ()),
                3 => schedules.use_builtin().await.map(|_| ()),
                4 => schedules.enable(true).await.map(|_| ()),
                _ => schedules.clear().await.map(|_| ()),
            }
        });
        tokio::time::sleep(HELD).await;
        assert!(
            !task.is_finished(),
            "{name} finished inside another command's window"
        );
        assert_eq!(repo.flags(), flags, "{name} wrote inside the window");
        drop(window);
        tokio::time::timeout(std::time::Duration::from_secs(30), task)
            .await
            .unwrap_or_else(|_| panic!("{name} never took the lock after the window closed"))
            .unwrap()
            .unwrap();
    }
}

/// No interleaving of `schedule set`, `schedule enable`, `use-python` and
/// `use-builtin` leaves both rows enabled, as a second connection polling the
/// file sees it (REQ-BDC-0720, 0724, B8Q1-005): four commands race from
/// fresh repositories, the rows created during the race. Planted negative:
/// the race the mode lock closes, a write decided from a read made before
/// another command created the other row, leaves both enabled.
#[tokio::test(flavor = "multi_thread")]
async fn no_interleaving_of_the_mode_commands_leaves_both_rows_enabled() {
    for round in 0..4 {
        let repo = Repo::new(vec![]);
        let stub = PyStub::new();
        let done = Arc::new(AtomicBool::new(false));
        let poller = {
            let (path, done) = (repo.cache_path.clone(), Arc::clone(&done));
            std::thread::spawn(move || {
                let conn = rusqlite::Connection::open(&path).unwrap();
                let mut seen = 0_u32;
                while !done.load(Ordering::SeqCst) {
                    let both: i64 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM sc_cron_schedule WHERE enabled = 1 AND ended_at IS NULL \
                             AND name IN ('bd-cache.sync.builtin', 'bd-cache.sync.python')",
                            [],
                            |r| r.get(0),
                        )
                        .unwrap_or(0);
                    assert!(both <= 1, "both rows enabled");
                    seen += 1;
                }
                seen
            })
        };
        let mut tasks = Vec::new();
        for op in 0..4 {
            let schedules = repo.schedules();
            let req = UsePython {
                cron: Some("*/30 * * * * *".into()),
                ..use_python(&stub)
            };
            tasks.push(tokio::spawn(async move {
                for _ in 0..3 {
                    let _ = match (op + round) % 4 {
                        0 => schedules.set(&set("*/30 * * * * *")).await.map(|_| ()),
                        1 => schedules
                            .use_python(&req, StopFlag::new())
                            .await
                            .map(|_| ()),
                        2 => schedules.use_builtin().await.map(|_| ()),
                        _ => schedules.enable(true).await.map(|_| ()),
                    };
                }
            }));
        }
        for t in tasks {
            t.await.unwrap();
        }
        done.store(true, Ordering::SeqCst);
        assert!(poller.join().unwrap() > 0);
        let (b, p) = repo.flags();
        assert!(!(b == Some(true) && p == Some(true)), "round {round}");
    }

    // Planted negative: the stale-read write the lock prevents.
    let repo = Repo::new(vec![]);
    schedule_set(&repo, &set("*/30 * * * * *")).await.unwrap();
    let stale = repo.rows();
    assert!(stale.python.is_none());
    let stub = PyStub::new();
    sync_use_python(&repo, StopFlag::new(), &use_python(&stub))
        .await
        .unwrap();
    let builtin = stale.builtin.unwrap();
    // Written from the stale read: enabled (its old flag), no Python flag listed.
    repo.scheduler
        .replace_and_set_enabled(builtin.spec, true, &[])
        .unwrap();
    assert_eq!(repo.flags(), (Some(true), Some(true)), "the hazard is real");
}
