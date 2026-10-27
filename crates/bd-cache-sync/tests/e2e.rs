//! End to end through the real composition (REQ-SYNC-0010 as amended,
//! REQ-BDC-0003, 0903, 0904): journaled writes applied by the drain of the
//! built-in row's scheduled run (`cron run --once`: sc-cron's
//! `run_due_with` with the sync resolver), through `Cache`, `SqliteStore`,
//! `FlockDrainLock` and `BdCli`, with the `bd` process replaced by `FakeBd`
//! (`common::real::Switched`), which records every argv. The same flow
//! against a scratch `bd` is `tests/live.rs` (testbed, `tests/run-live.sh`).
//!
//! Where the phase-a cases of the files sprint b-8 retired went (the
//! consumer messaging they tested is gone, ADR-BDC-0010):
//!
//! | Retired file | Its cases now |
//! |---|---|
//! | `outage.rs` | an outage is a scope result with its rows kept and reads answering (`builtin_run.rs`); an outage of several runs, with writes waiting and the recovery draining (here); the hook is told (`hook_runner.rs`); nothing is announced by the cache |
//! | `tick_budget.rs` | the drain before the scopes and one event per step (`cron_logging.rs`); the hook's reserve kept when the sync uses its share (`hook_runner.rs`); a run with every bd call slow ends within its budget (here); a stop seen between steps (`builtin_run.rs`) |
//! | `marker_wait.rs` | no marker is written by the run (the hook marks with its own verbs, `hook_runner.rs`); the bounded marker writes stay in `bd-cache-store`'s tests |

#![cfg(unix)]
#![expect(
    clippy::unwrap_used,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bd_cache_model::{IdemKey, JournalState, Seq};
use bd_cache_sync::{ScheduleSet, StopFlag};
use common::repo::Repo;
use common::{id, label_scope, scope};
use sc_beads::testing::{FakeOp, Injection, actor, bead};
use sc_beads::{DepType, Guards};

fn every_minute(budget_ms: Option<u64>) -> ScheduleSet {
    ScheduleSet {
        expr: "0 * * * * *".to_owned(),
        budget_ms,
        hook_reserve_ms: Some(0),
        post_sync: None,
        python: None,
        env: vec![],
    }
}

/// A repository with the built-in row and the label scope `x` of beads
/// labelled `area`.
async fn repo() -> Repo {
    let repo = Repo::new(vec![label_scope("x", "area")]);
    repo.schedules().set(&every_minute(None)).await.unwrap();
    repo
}

fn add(repo: &Repo, name: &str, labelled: bool) {
    let mut b = bead(name);
    if labelled {
        b.labels = vec!["area".into()];
    }
    repo.bd.insert(b);
}

/// One scheduled run of the built-in row: the clock moves to its next
/// minute and `cron run --once` runs it. Returns the recorded outcome.
async fn step(repo: &Repo) -> String {
    repo.cron_clock.advance(chrono::Duration::seconds(60));
    repo.clock.advance(Duration::from_secs(60));
    let once = repo
        .schedules()
        .cron_run_once(StopFlag::new())
        .await
        .unwrap();
    assert_eq!(once.ran.len(), 1, "{:?}", once.ran);
    once.ran
        .first()
        .map(|r| r.outcome.clone())
        .unwrap_or_default()
}

fn write(repo: &Repo, bead: &str, op: &str, args: &str, key: &str) -> Seq {
    repo.cache
        .lock()
        .unwrap()
        .write(
            &id(bead),
            op,
            args,
            Guards::default(),
            IdemKey::new(key).unwrap(),
            actor("san"),
        )
        .unwrap()
        .seq
}

fn entry(repo: &Repo, seq: Seq) -> bd_cache_model::JournalEntry {
    repo.cache.lock().unwrap().entry(seq).unwrap()
}

/// A write journaled through the core lands in bd via the scheduled run's
/// drain and is verified by read-back, end to end (REQ-SYNC-0010).
#[tokio::test(flavor = "multi_thread")]
async fn a_write_journaled_through_the_core_lands_in_bd_via_the_scheduled_drain() {
    let repo = repo().await;
    add(&repo, "b1", true);
    assert_eq!(step(&repo).await, "ok", "the first run syncs the scope");

    let seq = write(&repo, "b1", "close", r#"{"reason": "PASS"}"#, "k1");
    let cached = repo.cache.lock().unwrap().get(&id("b1"), None).unwrap();
    assert_eq!(
        cached.row.bead.status.as_deref(),
        Some("closed"),
        "reads see it at once"
    );
    assert_eq!(
        repo.bd.row("b1").unwrap().status.as_deref(),
        Some("open"),
        "not yet in bd"
    );
    assert_eq!(entry(&repo, seq).state, JournalState::Pending);

    assert_eq!(step(&repo).await, "ok");
    let row = repo.bd.row("b1").unwrap();
    assert_eq!(
        row.status.as_deref(),
        Some("closed"),
        "written to bd by the run's drain"
    );
    assert_eq!(row.close_reason.as_deref(), Some("PASS"));
    let e = entry(&repo, seq);
    assert_eq!(e.state, JournalState::Applied);
    assert!(e.applied_at.is_some() && e.last_error.is_none(), "{e:?}");
    let closes: Vec<_> = repo
        .bd
        .calls()
        .into_iter()
        .filter(|c| c.op() == FakeOp::Close)
        .collect();
    assert_eq!(closes.len(), 1, "one bd close, attributed: {closes:?}");
    let show = serde_json::to_value(repo.schedules().show().await.unwrap()).unwrap();
    assert_eq!(show["rows"]["builtin"]["outcome"], "ok");
}

/// A failed read-back marks the entry applied and the row stale and holds
/// the bead's later entries until a run's sync re-reads it; the next run
/// writes them (REQ-BDC-0903, through the real store).
#[tokio::test(flavor = "multi_thread")]
async fn test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync() {
    let repo = repo().await;
    add(&repo, "b1", true);
    assert_eq!(step(&repo).await, "ok");
    let first = write(&repo, "b1", "update", r#"{"status": "in_progress"}"#, "k1");
    let second = write(&repo, "b1", "close", r#"{"reason": "PASS"}"#, "k2");
    // Only the first write's read-back is lost.
    let lists = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&lists);
    repo.bd.inject_where(
        move |c| c.op() == FakeOp::ListByIds && counter.fetch_add(1, Ordering::SeqCst) == 1,
        Injection::Unavailable { timed_out: false },
    );
    assert_eq!(step(&repo).await, "ok");
    let e = entry(&repo, first);
    assert_eq!(e.state, JournalState::Applied);
    assert!(e.last_error.is_some(), "the read-back error is kept");
    assert_eq!(
        entry(&repo, second).state,
        JournalState::Pending,
        "held, not written"
    );
    assert_eq!(
        repo.bd.row("b1").unwrap().status.as_deref(),
        Some("in_progress")
    );
    // The run's sync re-read the stale row; the next run writes the held entry.
    assert!(
        repo.cache
            .lock()
            .unwrap()
            .status()
            .unwrap()
            .store
            .stale
            .is_empty()
    );
    assert_eq!(step(&repo).await, "ok");
    assert_eq!(entry(&repo, second).state, JournalState::Applied);
    assert_eq!(repo.bd.row("b1").unwrap().status.as_deref(), Some("closed"));
}

fn ready_ids(repo: &Repo) -> Vec<String> {
    let mut ids: Vec<String> = repo
        .cache
        .lock()
        .unwrap()
        .ready(&scope("x"), None)
        .unwrap()
        .ready
        .iter()
        .map(|v| v.bead.id.as_str().to_owned())
        .collect();
    ids.sort();
    ids
}

/// A journaled close of a blocker, applied by a scheduled run's drain,
/// makes its depender ready in the same run's sync; reopening blocks it
/// again (REQ-SYNC-0010).
#[tokio::test(flavor = "multi_thread")]
async fn test_e2e_blocker_close_and_reopen_update_ready() {
    let repo = repo().await;
    add(&repo, "blk", true);
    add(&repo, "dep", true);
    repo.bd.add_edge("dep", "blk", DepType::Blocks);
    assert_eq!(step(&repo).await, "ok");
    assert_eq!(ready_ids(&repo), ["blk"], "dep is blocked");

    write(&repo, "blk", "close", r#"{"reason": "PASS"}"#, "k-close");
    assert_eq!(step(&repo).await, "ok");
    assert_eq!(ready_ids(&repo), ["dep"], "ready in the same run's sync");

    write(&repo, "blk", "update", r#"{"status": "open"}"#, "k-reopen");
    assert_eq!(step(&repo).await, "ok");
    assert_eq!(ready_ids(&repo), ["blk"], "blocked again");
}

/// During an outage of several runs, each run records an `outage` result
/// for the scope (its rows kept, its `failing_since` kept), reads of cached
/// beads answer, writes wait in the journal, and the first run after the
/// recovery drains them (ADR-BDC-0005). The run itself is `ok` throughout.
#[tokio::test(flavor = "multi_thread")]
async fn an_outage_of_several_runs_through_the_real_store_and_cli() {
    let repo = repo().await;
    add(&repo, "b1", true);
    assert_eq!(step(&repo).await, "ok");
    let seq = write(&repo, "b1", "close", r#"{"reason": "PASS"}"#, "k1");
    repo.down.store(true, Ordering::SeqCst);
    let mut since = None;
    for _ in 0..3 {
        assert_eq!(
            step(&repo).await,
            "ok",
            "an outage is a result, not a failed run"
        );
        let state = repo.cache.lock().unwrap().sync_state(&scope("x")).unwrap();
        let failing = state
            .failing_since
            .clone()
            .expect("failing during the outage");
        assert_eq!(
            since.get_or_insert(failing.clone()),
            &failing,
            "kept across runs"
        );
        assert_eq!(
            entry(&repo, seq).state,
            JournalState::Pending,
            "the write waits"
        );
        assert!(
            repo.cache
                .lock()
                .unwrap()
                .cached(&id("b1"))
                .unwrap()
                .is_some(),
            "reads answer"
        );
    }
    repo.down.store(false, Ordering::SeqCst);
    assert_eq!(step(&repo).await, "ok");
    assert_eq!(
        entry(&repo, seq).state,
        JournalState::Applied,
        "drained on recovery"
    );
    assert_eq!(repo.bd.row("b1").unwrap().status.as_deref(), Some("closed"));
    let state = repo.cache.lock().unwrap().sync_state(&scope("x")).unwrap();
    assert!(state.failing_since.is_none(), "cleared by the ok sync");
}

/// A scheduled run whose bd calls are all slow still ends within its
/// budget: the drain and the syncs are cut at the sync deadline and the
/// rest is deferred (REQ-BDC-0703 as amended, NFR-BDC-0004).
#[tokio::test(flavor = "multi_thread")]
async fn a_scheduled_run_with_a_slow_bd_ends_within_its_budget() {
    let repo = Repo::new(vec![label_scope("x", "area"), label_scope("y", "area")]);
    repo.schedules()
        .set(&every_minute(Some(2_000)))
        .await
        .unwrap();
    add(&repo, "b1", true);
    for _ in 0..16 {
        repo.bd
            .inject_where(|_| true, Injection::Delay(Duration::from_millis(400)));
    }
    let started = Instant::now();
    assert_eq!(step(&repo).await, "ok");
    assert!(
        started.elapsed() < Duration::from_secs(12),
        "{:?}",
        started.elapsed()
    );
}
