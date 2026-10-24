//! End to end through the real composition (REQ-BDC-0003, 0903, 0904):
//! journaled writes applied by a drain that `sc-cron` schedules, through
//! `SyncJob`, `Cache`, `SqliteStore`, `FlockDrainLock` and `BdCli`, with the
//! `bd` process replaced by `FakeBd`, which records every argv. The same flow
//! against a scratch `bd` workspace is `tests/live.rs` (testbed).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bd_cache_model::{IdemKey, JournalState, Seq};
use bd_cache_sync::TICK_BUDGET;
use common::real::Real;
use common::{id, scope};
use sc_beads::testing::{FakeOp, Injection, actor};
use sc_beads::{DepType, Guards};
use sc_cron::RunOutcome;

fn close(r: &Real, bead: &str, key: &str) -> Seq {
    r.job.with_cache(|c| {
        c.write(
            &id(bead),
            "close",
            r#"{"reason": "PASS"}"#,
            Guards::default(),
            IdemKey::new(key).unwrap(),
            actor("san"),
        )
        .unwrap()
        .seq
    })
}

fn update_status(r: &Real, bead: &str, status: &str, key: &str) -> Seq {
    r.job.with_cache(|c| {
        c.write(
            &id(bead),
            "update",
            &format!(r#"{{"status": "{status}"}}"#),
            Guards::default(),
            IdemKey::new(key).unwrap(),
            actor("san"),
        )
        .unwrap()
        .seq
    })
}

fn entry(r: &Real, seq: Seq) -> bd_cache_model::JournalEntry {
    r.job.with_cache(|c| c.entry(seq).unwrap())
}

/// A write journaled through the core lands in bd via the scheduled drain and
/// is verified by read-back, end to end.
#[tokio::test(flavor = "multi_thread")]
async fn a_write_journaled_through_the_core_lands_in_bd_via_the_scheduled_drain() {
    let r = Real::new();
    r.add("b1", Some("x"), |b| b.assignee = Some("dev".into()));
    let worker = r.start().await;
    // First tick: the phase is synced (the root and b1 are cached).
    assert_eq!(r.step().await, RunOutcome::Ok);

    let seq = close(&r, "b1", "k1");
    // Visible to reads at once, not yet in bd.
    assert_eq!(
        r.job
            .with_cache(|c| c.get(&id("b1"), None).unwrap().row.bead.status),
        Some("closed".into())
    );
    assert_eq!(
        r.fake.backend().row("b1").unwrap().status.as_deref(),
        Some("open")
    );
    assert_eq!(entry(&r, seq).state, JournalState::Pending);

    let outcome = r.step().await;
    r.shutdown(worker).await;

    assert_eq!(outcome, RunOutcome::Ok, "{outcome:?}");
    // In bd, written by the real argv builders and checked by read-back.
    let row = r.fake.backend().row("b1").unwrap();
    assert_eq!(row.status.as_deref(), Some("closed"));
    assert_eq!(row.close_reason.as_deref(), Some("PASS"));
    let e = entry(&r, seq);
    assert_eq!(e.state, JournalState::Applied);
    assert!(e.applied_at.is_some() && e.last_error.is_none(), "{e:?}");
    // The recorded argv: one `bd close b1 --reason PASS`, attributed.
    let closes: Vec<_> = r
        .fake
        .calls()
        .into_iter()
        .filter(|a| a.first().map(String::as_str) == Some("close"))
        .collect();
    assert_eq!(closes.len(), 1, "{:?}", r.fake.calls());
    for want in ["b1", "--reason", "PASS", "--actor", "san"] {
        assert!(closes[0].iter().any(|a| a == want), "{:?}", closes[0]);
    }
    // The scheduler recorded the run.
    assert_eq!(r.schedule().last_outcome, Some(RunOutcome::Ok));
    // Nothing was told to anyone.
    assert!(r.sender.sent().is_empty());
}

/// A failed read-back marks the entry applied and the row stale, holds the
/// bead's later entries until a sync re-reads it, then they drain, through
/// the real store.
#[tokio::test(flavor = "multi_thread")]
async fn test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync() {
    let r = Real::new();
    r.add("b1", Some("x"), |b| b.assignee = Some("dev".into()));
    let worker = r.start().await;
    assert_eq!(r.step().await, RunOutcome::Ok);
    let first = update_status(&r, "b1", "in_progress", "k1");
    let second = close(&r, "b1", "k2");

    // The pass: fingerprint, pre-read, write 1, read-back 1. Count the
    // `list_by_ids` calls so only the second (the read-back) is lost.
    let lists = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&lists);
    r.fake.backend().inject_where(
        move |c| c.op() == FakeOp::ListByIds && counter.fetch_add(1, Ordering::SeqCst) == 1,
        Injection::Unavailable { timed_out: false },
    );
    let budget = bd_cache_model::DrainBudget::before_tick_end(sc_beads::Deadline::after(
        &*r.clock,
        TICK_BUDGET,
    ));
    let report = r.job.with_cache(|c| c.drain(&budget).unwrap());

    assert_eq!(report.applied, [first], "{report:?}");
    assert_eq!(report.retrying, [second], "held, not written: {report:?}");
    let e = entry(&r, first);
    assert_eq!(e.state, JournalState::Applied);
    assert!(e.last_error.is_some(), "the read-back error is kept");
    assert_eq!(
        r.job.with_cache(|c| c.status().unwrap().store.stale),
        [id("b1")]
    );
    assert_eq!(
        r.fake.backend().row("b1").unwrap().status.as_deref(),
        Some("in_progress")
    );
    assert_eq!(entry(&r, second).state, JournalState::Pending);
    assert_eq!(entry(&r, second).attempts, 0);

    // The scheduled tick drains (the bead is still held), then syncs: the
    // sync re-reads the stale row and clears the mark...
    assert_eq!(r.step().await, RunOutcome::Ok);
    assert!(
        r.job
            .with_cache(|c| c.status().unwrap().store.stale)
            .is_empty()
    );
    assert_eq!(
        entry(&r, second).state,
        JournalState::Pending,
        "held for that pass"
    );
    // ...so the next tick writes the held entry.
    assert_eq!(r.step().await, RunOutcome::Ok);
    r.shutdown(worker).await;
    assert_eq!(entry(&r, second).state, JournalState::Applied);
    assert_eq!(
        r.fake.backend().row("b1").unwrap().status.as_deref(),
        Some("closed")
    );
}

fn ready_ids(r: &Real) -> Vec<String> {
    r.job.with_cache(|c| {
        let mut ids: Vec<String> = c
            .ready(&scope("x"), None)
            .unwrap()
            .ready
            .iter()
            .map(|v| v.bead.id.as_str().to_owned())
            .collect();
        ids.sort();
        ids
    })
}

/// A journaled close of a blocker, applied by a scheduled drain, makes the
/// depender ready in the same tick's sync; reopening blocks it again; the
/// same for a blocker outside every scope.
#[tokio::test(flavor = "multi_thread")]
async fn test_e2e_blocker_close_and_reopen_update_ready() {
    let r = Real::new();
    r.add("blk", Some("x"), |_| {});
    r.add("dep", Some("x"), |_| {});
    r.add("out", None, |_| {}); // a blocker in no scope
    r.add("dep2", Some("x"), |_| {});
    r.fake.backend().add_edge("dep", "blk", DepType::Blocks);
    r.fake.backend().add_edge("dep2", "out", DepType::Blocks);
    let worker = r.start().await;
    assert_eq!(r.step().await, RunOutcome::Ok);
    assert_eq!(ready_ids(&r), ["blk", "rx"], "dep and dep2 are blocked");

    // Cached blocker: close, applied by the next scheduled tick.
    close(&r, "blk", "k-close");
    assert_eq!(r.step().await, RunOutcome::Ok);
    let reply = r.job.with_cache(|c| c.ready(&scope("x"), None).unwrap());
    assert!(
        !reply.locally_adjusted,
        "bd's own answer, fetched after the drain: {reply:?}"
    );
    assert_eq!(ready_ids(&r), ["dep", "rx"]);

    // Reopen: blocked again within the same tick.
    update_status(&r, "blk", "open", "k-reopen");
    assert_eq!(r.step().await, RunOutcome::Ok);
    assert_eq!(ready_ids(&r), ["blk", "rx"]);

    // A blocker outside every scope: the journaled close is fetched and
    // applied; the sync's `bd blocked` / `bd ready` see it.
    r.job.with_cache(|c| c.get(&id("out"), None).unwrap());
    close(&r, "out", "k-out");
    assert_eq!(r.step().await, RunOutcome::Ok);
    assert_eq!(ready_ids(&r), ["blk", "dep2", "rx"]);
    update_status(&r, "out", "open", "k-out-reopen");
    assert_eq!(r.step().await, RunOutcome::Ok);
    assert_eq!(ready_ids(&r), ["blk", "rx"]);
    r.shutdown(worker).await;
}

/// During an outage of several ticks reads of cached beads answer with
/// `outage` set, no escalation is sent, and the journal drains on recovery.
#[tokio::test(flavor = "multi_thread")]
async fn an_outage_of_several_ticks_through_the_real_store_and_cli() {
    let r = Real::new();
    r.add("b1", Some("x"), |_| {});
    let worker = r.start().await;
    assert_eq!(r.step().await, RunOutcome::Ok);
    let seq = close(&r, "b1", "k1");
    r.down.store(true, Ordering::SeqCst);

    // More ticks than the attempts rule allows.
    for _ in 0..7 {
        assert_eq!(r.step().await, RunOutcome::Ok);
        let got = r.job.with_cache(|c| c.get(&id("b1"), None).unwrap());
        assert!(got.outage.is_some(), "{got:?}");
        assert_eq!(got.row.bead.status.as_deref(), Some("closed"));
        let list = r
            .job
            .with_cache(|c| c.list(&scope("x"), None, None).unwrap());
        assert!(list.outage.is_some());
    }
    let e = entry(&r, seq);
    assert_eq!((e.state, e.attempts), (JournalState::Pending, 0), "{e:?}");
    // The lead heard of the outage once, and of nothing else.
    let sent = r.sender.sent();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].lead, "lead-x");
    assert!(r.job.unannounced_escalations().unwrap().is_empty());

    r.down.store(false, Ordering::SeqCst);
    assert_eq!(r.step().await, RunOutcome::Ok);
    r.shutdown(worker).await;
    assert_eq!(entry(&r, seq).state, JournalState::Applied);
    assert_eq!(
        r.fake.backend().row("b1").unwrap().status.as_deref(),
        Some("closed")
    );
    assert!(
        r.job
            .with_cache(|c| c.status().unwrap().store.outage)
            .is_none()
    );
    assert_eq!(r.sender.sent().len(), 1, "recovery is silent");
}

/// The escalation path through the real store: announced once, and the
/// store keeps `announced_at`.
#[tokio::test(flavor = "multi_thread")]
async fn an_escalation_is_announced_once_and_the_store_remembers_it() {
    let r = Real::new();
    r.add("b1", Some("x"), |_| {});
    let worker = r.start().await;
    assert_eq!(r.step().await, RunOutcome::Ok);
    let seq = close(&r, "b1", "k1");
    r.fake.backend().remove("b1");

    assert_eq!(r.step().await, RunOutcome::Ok);
    assert_eq!(r.step().await, RunOutcome::Ok);
    r.shutdown(worker).await;

    assert_eq!(entry(&r, seq).state, JournalState::Escalated);
    assert!(entry(&r, seq).announced_at.is_some());
    assert_eq!(r.sender.sent_to("lead-x").len(), 1, "{:?}", r.sender.sent());
}

/// A message the sender cannot deliver is a `refused` run for the scheduler
/// too: `sc-cron` records it, and the next run retries.
#[tokio::test(flavor = "multi_thread")]
async fn an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried() {
    let r = Real::new();
    r.add("b1", Some("x"), |_| {});
    let worker = r.start().await;
    assert_eq!(r.step().await, RunOutcome::Ok);
    let seq = close(&r, "b1", "k1");
    r.fake.backend().remove("b1");
    r.sender.fail(true);

    let refused = r.step().await;
    assert!(
        matches!(&refused, RunOutcome::Refused(why) if why.contains(&format!("#{seq}"))),
        "{refused:?}"
    );
    // Stopping the worker waits for the run's outcome to be recorded.
    r.shutdown(worker).await;
    assert!(matches!(
        r.schedule().last_outcome,
        Some(RunOutcome::Refused(_))
    ));
    assert!(entry(&r, seq).announced_at.is_none());

    r.sender.fail(false);
    let worker = r.start().await;
    assert_eq!(r.step().await, RunOutcome::Ok);
    r.shutdown(worker).await;
    assert!(entry(&r, seq).announced_at.is_some());
    assert_eq!(r.sender.sent_to("lead-x").len(), 1);
}
