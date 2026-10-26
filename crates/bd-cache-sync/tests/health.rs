//! `cache health` judges the sync rows from what [`sync_facts`] reads of the
//! real table (REQ-BDC-0732, b-9 acceptance 4): both rows disabled
//! (`no_sync_row_enabled`), no worker for three periods (`sync_stale`, on
//! the injected clocks), the Python row failing three runs in a row
//! (`sync_failing`) and the hook failing three times in a row
//! (`hook_failing`), each with its planted negative. The rows run over the
//! real composition with sc-cron's `ManualClock`; the Python row and the hook
//! run stub interpreters (Unix).

#![expect(
    clippy::unwrap_used,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache_api::{CacheFacts, Command, decode, execute};
use bd_cache_sync::{ScheduleSet, StopFlag, sync_facts};
use common::label_scope;
use common::repo::Repo;
use serde_json::{Value, json};

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

/// `cache health` over `repo`'s cache, with the sync facts read now.
fn health(repo: &Repo) -> Value {
    let mut cache = repo.cache.lock().unwrap();
    let facts = CacheFacts {
        sync: Some(sync_facts(repo.store.as_ref(), &cache).unwrap()),
        ..CacheFacts::default()
    };
    let req = decode(Command::CacheHealth, &json!({})).unwrap();
    execute(&mut cache, &req, &facts)
        .unwrap()
        .to_json()
        .unwrap()
}

fn codes(h: &Value) -> Vec<String> {
    h["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["code"].as_str().unwrap().to_owned())
        .collect()
}

/// Advances both clocks: sc-cron's (the rows' times) and the cache's (health's now).
fn step(repo: &Repo, secs: u64) {
    repo.cron_clock
        .advance(chrono::Duration::seconds(i64::try_from(secs).unwrap()));
    repo.clock.advance(Duration::from_secs(secs));
}

/// `cron run --once` over `repo`; the error when a row failed.
async fn cron_run_once(repo: &Repo) -> Option<bd_cache_api::ApiError> {
    repo.schedules()
        .cron_run_once(StopFlag::new())
        .await
        .unwrap()
        .error()
}

/// Both rows disabled is `no_sync_row_enabled`. Planted negative: the
/// built-in row enabled again clears it, and a file with no sync row reports
/// mode `none` without the reason.
#[tokio::test(flavor = "multi_thread")]
async fn both_rows_disabled_is_no_sync_row_enabled() {
    let repo = Repo::new(vec![]);
    let h = health(&repo);
    assert_eq!(h["sync"]["mode"], "none", "{h}");
    assert_eq!(codes(&h), Vec::<String>::new(), "no sync row exists: {h}");

    repo.schedules().set(&every_minute()).await.unwrap();
    repo.schedules().enable(false).await.unwrap();
    let h = health(&repo);
    assert_eq!(h["state"], "degraded", "{h}");
    assert_eq!(codes(&h), ["no_sync_row_enabled"], "{h}");
    assert_eq!(h["sync"], json!({"mode": "none", "last_run": null}), "{h}");

    repo.schedules().enable(true).await.unwrap();
    let h = health(&repo);
    assert_eq!(h["state"], "ok", "{h}");
    assert_eq!(h["sync"]["mode"], "builtin", "{h}");
    assert_eq!(h["sync"]["last_run"]["row"], "builtin", "{h}");
}

/// No worker runs the enabled row for more than three periods: a registered
/// scope that never synced is `sync_stale`, aged from the row's
/// `created_at` on the injected clocks. Planted negative: at exactly three
/// periods it is not, and one run of the row makes the scope fresh again.
#[tokio::test(flavor = "multi_thread")]
async fn no_worker_for_three_periods_is_sync_stale() {
    let repo = Repo::new(vec![label_scope("lbl", "area")]);
    repo.schedules().set(&every_minute()).await.unwrap();
    step(&repo, 180);
    let h = health(&repo);
    assert_eq!(
        codes(&h),
        Vec::<String>::new(),
        "three periods exactly: {h}"
    );
    step(&repo, 1);
    let h = health(&repo);
    assert_eq!(h["state"], "degraded", "{h}");
    assert_eq!(codes(&h), ["sync_stale"], "{h}");
    assert!(
        h["reasons"][0]["message"]
            .as_str()
            .unwrap()
            .contains("scope(s) lbl"),
        "{h}"
    );

    assert!(cron_run_once(&repo).await.is_none(), "the run syncs lbl");
    let h = health(&repo);
    assert_eq!(h["state"], "ok", "{h}");
    assert_eq!(h["sync"]["last_run"]["outcome"], "ok", "{h}");
    assert_eq!(h["sync"]["last_run"]["consecutive_failures"], 0, "{h}");
}

/// The Python row failing three runs in a row is `sync_failing`. Planted
/// negative: after two failures it is not, and an ok run resets the count.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn the_python_row_failing_three_runs_is_sync_failing() {
    use common::repo::PyStub;
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    repo.schedules()
        .use_python(
            &bd_cache_sync::UsePython {
                python: stub.python.clone(),
                module: "consumer.sync".into(),
                cron: Some("0 * * * * *".into()),
                timeout_ms: Some(5_000),
                env: vec![],
            },
            StopFlag::new(),
        )
        .await
        .unwrap();
    stub.set("tick", "exit:1");
    for n in 1..=2 {
        step(&repo, 60);
        assert!(cron_run_once(&repo).await.is_some(), "run {n} failed");
        let h = health(&repo);
        assert_eq!(h["sync"]["mode"], "python", "{h}");
        assert_eq!(h["sync"]["last_run"]["consecutive_failures"], n, "{h}");
        assert_eq!(codes(&h), Vec::<String>::new(), "{n} failures: {h}");
    }
    step(&repo, 60);
    assert!(cron_run_once(&repo).await.is_some());
    let h = health(&repo);
    assert_eq!(h["state"], "degraded", "{h}");
    assert_eq!(codes(&h), ["sync_failing"], "{h}");
    assert_eq!(h["sync"]["last_run"]["row"], "python", "{h}");

    stub.unset("tick");
    step(&repo, 60);
    assert!(cron_run_once(&repo).await.is_none());
    let h = health(&repo);
    assert_eq!(h["state"], "ok", "an ok run resets the count: {h}");
}

/// The hook failing three times in a row is `hook_failing`. Planted
/// negative: after two failures it is not.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn the_hook_failing_three_times_is_hook_failing() {
    let repo = Repo::new(vec![]);
    let hook = common::HookStub::new();
    hook.mode("fail");
    let set = ScheduleSet {
        post_sync: Some("consumer.hooks:after".into()),
        python: Some(hook.python.clone()),
        ..every_minute()
    };
    repo.schedules().set(&set).await.unwrap();
    for n in 1..=2 {
        step(&repo, 60);
        assert!(cron_run_once(&repo).await.is_none(), "the run itself is ok");
        assert_eq!(hook.calls(), n);
        let h = health(&repo);
        assert_eq!(codes(&h), Vec::<String>::new(), "{n} hook failures: {h}");
    }
    step(&repo, 60);
    assert!(cron_run_once(&repo).await.is_none());
    assert_eq!(hook.calls(), 3);
    let h = health(&repo);
    assert_eq!(h["state"], "degraded", "{h}");
    assert_eq!(codes(&h), ["hook_failing"], "{h}");
}
