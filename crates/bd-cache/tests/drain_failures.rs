//! Drain failures on a reachable backend: attempts, the per-bead hold, the retry
//! cap, read-back failure and the stale hold (REQ-BDC-0003, 0403, 0404, 0405).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::{store, store_mut};
use bd_cache_model::{
    CacheConfig, CacheStore, DrainBudget, Freshness, IdemKey, JournalEntry, JournalState, Seq,
};
use common::{TestCache, World, id, x};
use sc_beads::Guards;
use sc_beads::testing::{FakeCall, FakeOp, Injection, actor};

fn journal(c: &mut TestCache, bead: &str, op: &str, args: &str, key: &str) -> Seq {
    c.write(
        &id(bead),
        op,
        args,
        Guards::default(),
        IdemKey::new(key).unwrap(),
        actor("san"),
    )
    .unwrap_or_else(|e| panic!("write {bead} {op}: {e}"))
    .seq
}

fn roomy(w: &World) -> DrainBudget {
    DrainBudget {
        start_gate: w.after(Duration::from_secs(20)),
        hard: w.after(Duration::from_secs(23)),
    }
}

fn entry(c: &TestCache, seq: Seq) -> JournalEntry {
    store(c).entry_by_seq(seq).unwrap().unwrap()
}

fn write_to(op: FakeOp, bead: &'static str) -> impl Fn(&FakeCall) -> bool + Send + Sync {
    move |c| c.op() == op && c.write_target() == Some(&id(bead))
}

#[test]
fn test_transient_write_failure_stays_pending_and_holds_later_entries_of_that_bead_only() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "update", r#"{"append_notes": "a"}"#, "k1");
    let e2 = journal(&mut c, "b1", "close", "{}", "k2");
    let e3 = journal(&mut c, "b2", "update", r#"{"assignee": "lead"}"#, "k3");
    w.bd.inject_where(write_to(FakeOp::Update, "b1"), Injection::Failed(1));
    w.bd.clear_calls();

    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![e3], "the other bead is not held");
    assert_eq!(r.retrying, vec![e1, e2]);
    assert!(r.escalated.is_empty() && r.outage.is_none());
    let failed = entry(&c, e1);
    assert_eq!(failed.state, JournalState::Pending);
    assert_eq!(failed.attempts, 1);
    assert!(
        failed
            .last_error
            .as_deref()
            .is_some_and(|e| e.starts_with("BD_CACHE.BD_WRITE_FAILED")),
        "{:?}",
        failed.last_error
    );
    let held = entry(&c, e2);
    assert_eq!((held.state, held.attempts), (JournalState::Pending, 0));
    assert!(
        !w.bd.writes().iter().any(|c| c.op() == FakeOp::Close),
        "the held entry never ran"
    );

    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![e1, e2]);
    assert_eq!(w.bd_row("b1").status.as_deref(), Some("closed"));
}

#[test]
fn test_a_reachable_backend_refusing_the_write_still_counts_attempts() {
    let w = World::new();
    let mut c = w.cache();
    let seq = journal(&mut c, "b1", "close", "{}", "k1");
    // Exit 1, a write timing out at the 30 s call cap (not the hard deadline),
    // and a write that cannot reach bd: each may have reached bd, each counts.
    let budget = |w: &World| DrainBudget {
        start_gate: w.after(Duration::from_secs(20)),
        hard: w.after(Duration::from_secs(120)),
    };
    let failures = [
        Injection::Failed(1),
        Injection::Delay(Duration::from_secs(31)),
        Injection::Unavailable { timed_out: false },
    ];
    for (n, failure) in (1..).zip(failures) {
        w.bd.inject(FakeOp::Close, failure.clone());
        let r = c.drain(&budget(&w)).unwrap();
        assert_eq!(r.retrying, vec![seq], "{failure:?}");
        assert!(r.deferred_in_flight.is_empty(), "{failure:?} is no cut");
        assert_eq!(r.outage, None, "{failure:?}: a write failure is no outage");
        assert_eq!(entry(&c, seq).attempts, n, "{failure:?}");
    }
    assert_eq!(store(&c).outage().unwrap(), None);
}

#[test]
fn test_retries_exhausted_escalate_and_never_silently_drop() {
    let w = World::new();
    let mut c = w.cache();
    let seq = journal(&mut c, "b1", "close", "{}", "k1");
    let max = w.config.max_attempts;
    assert_eq!(max, 5);
    for _ in 0..max {
        w.bd.inject(FakeOp::Close, Injection::Failed(1));
    }
    for n in 1..max {
        let r = c.drain(&roomy(&w)).unwrap();
        assert_eq!(r.retrying, vec![seq]);
        assert!(r.escalated.is_empty());
        assert_eq!(entry(&c, seq).attempts, n);
    }
    let r = c.drain(&roomy(&w)).unwrap();
    assert!(r.retrying.is_empty());
    assert_eq!(r.escalated.len(), 1);
    assert_eq!(r.escalated[0].seq, seq);
    assert!(
        r.escalated[0]
            .reason
            .starts_with("retries exhausted after 5 attempts"),
        "{}",
        r.escalated[0].reason
    );
    let e = entry(&c, seq);
    assert_eq!((e.state, e.attempts), (JournalState::Escalated, 5));
    assert_eq!(c.unannounced_escalations().unwrap()[0].seq, seq);

    // Never dropped, never retried past the cap.
    w.bd.clear_calls();
    let r = c.drain(&roomy(&w)).unwrap();
    assert!(r.applied.is_empty() && r.retrying.is_empty() && r.escalated.is_empty());
    assert!(w.bd.writes().is_empty());
    assert_eq!(entry(&c, seq).state, JournalState::Escalated);
    assert_eq!(c.open_work().unwrap().escalated_unannounced, 1);
}

#[test]
fn the_retry_cap_is_the_configured_max_attempts() {
    let w = World::new();
    let mut c = w.cache_with(CacheConfig {
        max_attempts: 2,
        ..w.config.clone()
    });
    let seq = journal(&mut c, "b1", "close", "{}", "k1");
    w.bd.inject(FakeOp::Close, Injection::Failed(1));
    w.bd.inject(FakeOp::Close, Injection::Failed(1));
    assert_eq!(c.drain(&roomy(&w)).unwrap().retrying, vec![seq]);
    assert_eq!(c.drain(&roomy(&w)).unwrap().escalated[0].seq, seq);
    assert_eq!(entry(&c, seq).attempts, 2);
}

#[test]
fn test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "update", r#"{"append_notes": "a"}"#, "k1");
    let e2 = journal(&mut c, "b1", "close", "{}", "k2");
    let e3 = journal(&mut c, "b2", "update", r#"{"assignee": "lead"}"#, "k3");
    // The pre-read lists b1 and b2; the first list of b1 alone is e1's read-back.
    w.bd.inject_where(
        |c| matches!(c, FakeCall::ListByIds(ids) if *ids == [id("b1")]),
        Injection::Failed(1),
    );

    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(
        r.applied,
        vec![e1, e3],
        "a failed read-back is still applied"
    );
    assert_eq!(r.retrying, vec![e2], "the bead's later entry is held");
    let applied = entry(&c, e1);
    assert_eq!(applied.state, JournalState::Applied);
    assert!(applied.applied_at.is_some());
    assert!(
        applied
            .last_error
            .as_deref()
            .is_some_and(|e| e.starts_with("BD_CACHE.BD_LIST_FAILED")),
        "{:?}",
        applied.last_error
    );
    assert_eq!(store(&c).stale_rows().unwrap(), vec![id("b1")]);
    assert_eq!(
        store(&c).get_row(&id("b1")).unwrap().unwrap().1,
        Freshness::Stale
    );
    assert_eq!(entry(&c, e2).attempts, 0);
    assert_eq!(
        w.bd_row("b1").notes.as_deref(),
        Some("a"),
        "the write landed"
    );

    // Later drains skip the stale bead's entries and make no bd call for them.
    for _ in 0..2 {
        w.bd.clear_calls();
        let r = c.drain(&roomy(&w)).unwrap();
        assert_eq!(r.retrying, vec![e2]);
        assert!(r.applied.is_empty());
        assert!(w.bd.calls().is_empty(), "{:?}", w.bd.calls());
        assert_eq!(entry(&c, e2).state, JournalState::Pending);
    }

    // A sync re-reads the row, which clears the stale mark; the entry then drains.
    let row = w.bd_row("b1");
    store_mut(&mut c)
        .upsert_rows(Some(&x()), &[row], false)
        .unwrap();
    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![e2]);
    assert_eq!(w.bd_row("b1").status.as_deref(), Some("closed"));
    assert!(store(&c).stale_rows().unwrap().is_empty());
}

#[test]
fn an_unreachable_bd_at_read_back_is_a_failed_read_back_and_the_pass_goes_on() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "close", "{}", "k1");
    let e2 = journal(&mut c, "b2", "close", "{}", "k2");
    // The pre-read lists b1 and b2; the first list of b1 alone is e1's read-back.
    w.bd.inject_where(
        |c| matches!(c, FakeCall::ListByIds(ids) if *ids == [id("b1")]),
        Injection::Unavailable { timed_out: false },
    );
    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(
        r.applied,
        vec![e1, e2],
        "applied, and the pass goes on to b2"
    );
    assert!(r.deferred.is_empty() && r.deferred_in_flight.is_empty());
    let e = entry(&c, e1);
    assert_eq!(e.state, JournalState::Applied);
    assert!(
        e.last_error
            .as_deref()
            .is_some_and(|m| m.starts_with("BD_CACHE.BD_UNAVAILABLE")),
        "{:?}",
        e.last_error
    );
    assert_eq!(store(&c).stale_rows().unwrap(), vec![id("b1")]);
    assert_eq!(r.outage, None, "a read-back records no outage");
    assert_eq!(w.bd_row("b2").status.as_deref(), Some("closed"));
}
