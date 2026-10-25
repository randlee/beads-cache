//! `drain` applies in seq order with one pre-read, achieved checks, preconditions,
//! escalations and read-back, under the single-writer lock
//! (REQ-BDC-0005, 0401, 0403, 0404, 0405, 0406; REQ-CORE-0102).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use bd_cache::testing::store;
use bd_cache_model::{
    CacheStore, DrainBudget, DrainLock, DrainReport, IdemKey, JournalEntry, JournalState, ScopeId,
    Seq, normalize,
};
use common::{TestCache, World, id};
use sc_beads::testing::{FakeCall, FakeOp, Injection, actor};
use sc_beads::{DepType, Guards, Refusal};

fn journal(c: &mut TestCache, bead: &str, op: &str, args: &str, pre: Guards, key: &str) -> Seq {
    c.write(
        &id(bead),
        op,
        args,
        pre,
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

/// The fake's command plan as short strings.
fn plan(w: &World) -> Vec<String> {
    w.bd.calls()
        .iter()
        .map(|c| match c {
            FakeCall::ListByIds(ids) => format!(
                "list {}",
                ids.iter()
                    .map(sc_beads::BeadId::as_str)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            other => match other.write_target() {
                Some(t) => format!("{:?} {t}", other.op()).to_lowercase(),
                None => format!("{:?}", other.op()).to_lowercase(),
            },
        })
        .collect()
}

/// Every seq the report names, sorted; panics when one is named twice.
fn accounted(r: &DrainReport) -> Vec<Seq> {
    let mut all: Vec<Seq> = r
        .applied
        .iter()
        .chain(&r.retrying)
        .chain(r.escalated.iter().map(|e| &e.seq))
        .chain(&r.deferred)
        .chain(&r.deferred_in_flight)
        .copied()
        .collect();
    all.sort();
    let n = all.len();
    all.dedup();
    assert_eq!(n, all.len(), "a seq is in two report lists: {r:?}");
    all
}

#[test]
fn test_drain_writes_in_order_reads_back_and_marks_applied() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(
        &mut c,
        "b1",
        "update",
        r#"{"status": "open", "append_notes": "n1"}"#,
        Guards::default(),
        "k1",
    );
    // Met only by e1's read-back: the read-back row is the fresh state for the next entry.
    let pre = Guards {
        status: Some("open".into()),
        assignee: None,
    };
    let e2 = journal(&mut c, "b1", "close", r#"{"reason": "PASS"}"#, pre, "k2");
    let e3 = journal(
        &mut c,
        "b2",
        "update",
        r#"{"assignee": "lead"}"#,
        Guards::default(),
        "k3",
    );
    w.bd.clear_calls();

    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![e1, e2, e3]);
    assert!(!r.busy);
    assert!(r.retrying.is_empty() && r.escalated.is_empty() && r.deferred.is_empty());
    assert!(r.deferred_in_flight.is_empty() && r.outage.is_none());
    assert_eq!(
        plan(&w),
        [
            "fingerprint",
            "list b1,b2",
            "update b1",
            "list b1",
            "close b1",
            "list b1",
            "update b2",
            "list b2"
        ]
    );
    for seq in [e1, e2, e3] {
        let e = entry(&c, seq);
        assert_eq!(e.state, JournalState::Applied);
        assert!(e.applied_at.is_some());
        assert_eq!(e.last_error, None);
        assert_eq!(e.attempts, 0);
    }
    for b in ["b1", "b2"] {
        let (row, _) = store(&c).get_row(&id(b)).unwrap().unwrap();
        assert_eq!(
            row,
            normalize(&w.bd_row(b)),
            "{b}: the read-back row is stored"
        );
    }
    let b1 = w.bd_row("b1");
    assert_eq!(b1.status.as_deref(), Some("closed"));
    assert_eq!(b1.notes.as_deref(), Some("n1"));
    assert_eq!(w.bd_row("b2").assignee.as_deref(), Some("lead"));
    assert!(store(&c).pending_entries().unwrap().is_empty());
}

#[test]
fn test_drain_pre_reads_all_pending_beads_in_one_call() {
    let w = World::new();
    let mut c = w.cache();
    let none = Guards::default;
    let seqs = [
        journal(
            &mut c,
            "b1",
            "update",
            r#"{"append_notes": "a"}"#,
            none(),
            "k1",
        ),
        journal(&mut c, "b2", "close", "{}", none(), "k2"),
        journal(
            &mut c,
            "b1",
            "update",
            r#"{"append_notes": "b"}"#,
            none(),
            "k3",
        ),
        journal(&mut c, "s1", "close", "{}", none(), "k4"),
    ];
    w.bd.clear_calls();
    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, seqs);
    let p = plan(&w);
    assert_eq!(p[0], "fingerprint");
    assert_eq!(
        p[1], "list b1,b2,s1",
        "one batched pre-read, beads once each"
    );
    assert!(
        p[2].starts_with("update"),
        "the pre-read precedes every write"
    );
    let lists =
        w.bd.reads()
            .iter()
            .filter(|c| c.op() == FakeOp::ListByIds)
            .count();
    assert_eq!(
        lists,
        1 + seqs.len(),
        "the pre-read plus one read-back per entry"
    );
}

#[test]
fn test_write_that_landed_before_a_crash_is_not_repeated() {
    let w = World::new();
    let mut c = w.cache();
    let close = journal(&mut c, "b1", "close", "{}", Guards::default(), "k1");
    let dep = journal(
        &mut c,
        "b2",
        "dep_add",
        r#"{"depends_on": "b1", "type": "blocks"}"#,
        Guards::default(),
        "k2",
    );
    let notes = journal(
        &mut c,
        "s1",
        "update",
        r#"{"append_notes": "done"}"#,
        Guards::default(),
        "k3",
    );
    // A crashed drain had already run all three commands.
    w.bd.edit("b1", |b| b.status = Some("closed".into()));
    w.bd.add_edge("b2", "b1", DepType::Blocks);
    w.bd.edit("s1", |b| b.notes = Some("done".into()));
    w.bd.clear_calls();

    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![close, dep, notes]);
    assert!(w.bd.writes().is_empty(), "no write is repeated");
    assert_eq!(plan(&w), ["fingerprint", "list b1,b2,s1"]);
    let (row, _) = store(&c).get_row(&id("b1")).unwrap().unwrap();
    assert_eq!(
        row.status.as_deref(),
        Some("closed"),
        "the fresh row is stored"
    );
    for seq in [close, dep, notes] {
        assert_eq!(entry(&c, seq).state, JournalState::Applied);
    }
}

#[test]
fn test_drain_on_absent_bead_escalates_not_found() {
    let w = World::new();
    let mut c = w.cache();
    let gone = journal(
        &mut c,
        "b2",
        "update",
        r#"{"assignee": "lead"}"#,
        Guards::default(),
        "k1",
    );
    let kept = journal(&mut c, "s1", "close", "{}", Guards::default(), "k2");
    w.bd.remove("b2");
    w.bd.clear_calls();

    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![kept]);
    assert_eq!(r.escalated.len(), 1);
    let item = &r.escalated[0];
    assert_eq!(item.seq, gone);
    assert_eq!(item.bead, id("b2"));
    assert_eq!(item.op, "update");
    assert_eq!(item.actor, actor("san"));
    assert_eq!(item.scope, Some(ScopeId::new("x").unwrap()));
    assert!(
        item.reason.starts_with("BD_CACHE.NOT_FOUND"),
        "{}",
        item.reason
    );
    let e = entry(&c, gone);
    assert_eq!(e.state, JournalState::Escalated);
    assert_eq!(e.last_error.as_deref(), Some(item.reason.as_str()));
    assert!(
        !plan(&w).contains(&"update b2".to_owned()),
        "no write for an absent bead"
    );
    assert_eq!(store(&c).unannounced_escalations().unwrap()[0].seq, gone);
}

#[test]
fn test_concurrent_backend_change_escalates_instead_of_overwriting() {
    let w = World::new();
    let mut c = w.cache();
    let pre = Guards {
        status: Some("in_progress".into()),
        assignee: None,
    };
    let seq = journal(&mut c, "b1", "close", r#"{"reason": "PASS"}"#, pre, "k1");
    // Another agent reopens and reassigns the bead in bd directly.
    w.bd.edit("b1", |b| {
        b.status = Some("open".into());
        b.assignee = Some("other".into());
    });
    w.bd.clear_calls();

    let r = c.drain(&roomy(&w)).unwrap();
    assert!(r.applied.is_empty());
    assert_eq!(r.escalated.len(), 1);
    let reason = &r.escalated[0].reason;
    assert!(reason.starts_with("BD_CACHE.PRECONDITION"), "{reason}");
    assert!(
        reason.contains("\"in_progress\"") && reason.contains("\"open\""),
        "{reason}"
    );
    assert!(w.bd.writes().is_empty(), "no write over the other change");
    assert_eq!(w.bd_row("b1").status.as_deref(), Some("open"));
    let (row, _) = store(&c).get_row(&id("b1")).unwrap().unwrap();
    assert_eq!(
        row.assignee.as_deref(),
        Some("other"),
        "the fresh row is stored"
    );
    let e = entry(&c, seq);
    assert_eq!(e.state, JournalState::Escalated);
    assert_eq!(e.attempts, 0);
}

#[test]
fn test_bd_guard_refusal_exit_13_escalates() {
    let w = World::new();
    let mut c = w.cache();
    let refused = journal(&mut c, "b1", "close", "{}", Guards::default(), "k1");
    let other = journal(&mut c, "b2", "close", "{}", Guards::default(), "k2");
    w.bd.inject(FakeOp::Close, Injection::Refused(Refusal::Precondition));

    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![other]);
    assert_eq!(r.escalated.len(), 1);
    assert_eq!(r.escalated[0].seq, refused);
    assert!(
        r.escalated[0].reason.starts_with("BD_CACHE.PRECONDITION"),
        "{}",
        r.escalated[0].reason
    );
    let e = entry(&c, refused);
    assert_eq!(e.state, JournalState::Escalated);
    assert_eq!(e.attempts, 0, "a refusal is not retried");
    assert_eq!(w.bd_row("b1").status.as_deref(), Some("in_progress"));
}

#[test]
fn test_drain_is_single_writer() {
    let w = World::new();
    let mut c = w.cache();
    let seq = journal(&mut c, "b1", "close", "{}", Guards::default(), "k1");
    w.bd.clear_calls();

    // Another process holds the lock.
    let guard = w.lock.try_acquire().unwrap().unwrap();
    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(
        r,
        DrainReport {
            busy: true,
            ..DrainReport::default()
        }
    );
    assert!(w.bd.calls().is_empty(), "a busy drain makes no bd call");
    assert_eq!(entry(&c, seq).state, JournalState::Pending);
    drop(guard);

    // The lock is held for every bd call of a drain, and released after it.
    let lock = w.lock.clone();
    let held_throughout = Arc::new(AtomicBool::new(true));
    let calls = Arc::new(AtomicUsize::new(0));
    let (held, n) = (Arc::clone(&held_throughout), Arc::clone(&calls));
    w.bd.inject_where(
        move |_| {
            n.fetch_add(1, Ordering::SeqCst);
            if !lock.is_held() {
                held.store(false, Ordering::SeqCst);
            }
            false
        },
        Injection::Failed(1),
    );
    let mut second = w.cache();
    let r = second.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![seq]);
    assert!(calls.load(Ordering::SeqCst) >= 4);
    assert!(held_throughout.load(Ordering::SeqCst));
    assert!(!w.lock.is_held(), "released when the drain returns");
}

#[test]
fn an_empty_drain_makes_no_bd_call() {
    let w = World::new();
    let mut c = w.cache();
    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r, DrainReport::default());
    assert!(w.bd.calls().is_empty());
}

#[test]
fn every_pending_entry_is_in_exactly_one_report_list() {
    let w = World::new();
    w.add("b3", |_| {});
    w.add("b4", |_| {});
    let mut c = w.cache();
    let none = Guards::default;
    let applied = journal(&mut c, "b1", "close", "{}", none(), "k1");
    let retrying = journal(&mut c, "b2", "close", "{}", none(), "k2");
    let held = journal(
        &mut c,
        "b2",
        "update",
        r#"{"append_notes": "x"}"#,
        none(),
        "k3",
    );
    let escalated = journal(&mut c, "s1", "close", "{}", none(), "k4");
    let in_flight = journal(&mut c, "b3", "close", "{}", none(), "k5");
    let deferred = journal(&mut c, "b4", "close", "{}", none(), "k6");
    w.bd.inject_where(
        |c| c.op() == FakeOp::Close && c.write_target() == Some(&id("b2")),
        Injection::Failed(1),
    );
    w.bd.inject_where(
        |c| c.op() == FakeOp::Close && c.write_target() == Some(&id("s1")),
        Injection::Refused(Refusal::Precondition),
    );
    w.bd.inject_where(
        |c| c.op() == FakeOp::Close && c.write_target() == Some(&id("b3")),
        Injection::Delay(Duration::from_secs(60)),
    );
    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![applied]);
    assert_eq!(r.retrying, vec![retrying, held]);
    assert_eq!(
        r.escalated.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![escalated]
    );
    assert_eq!(r.deferred_in_flight, vec![in_flight]);
    assert_eq!(r.deferred, vec![deferred]);
    let mut all = vec![applied, retrying, held, escalated, in_flight, deferred];
    all.sort();
    assert_eq!(accounted(&r), all);
}
