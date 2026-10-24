//! The drain's start gate and hard deadline: deferred, deferred in flight, no
//! attempt and no outage for a cut (REQ-BDC-0402; NFR-BDC-0004).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::store;
use bd_cache_model::{CacheStore, DrainBudget, IdemKey, JournalEntry, JournalState, Seq};
use common::{TestCache, World, id};
use sc_beads::testing::{FakeCall, FakeOp, Injection, actor};
use sc_beads::{Clock, Guards};

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

fn budget(w: &World, gate_s: u64, hard_s: u64) -> DrainBudget {
    DrainBudget {
        start_gate: w.after(Duration::from_secs(gate_s)),
        hard: w.after(Duration::from_secs(hard_s)),
    }
}

fn entry(c: &TestCache, seq: Seq) -> JournalEntry {
    store(c).entry_by_seq(seq).unwrap().unwrap()
}

fn closes(w: &World) -> usize {
    w.bd.writes()
        .iter()
        .filter(|c| c.op() == FakeOp::Close)
        .count()
}

#[test]
fn test_drain_deadline_defers_unstarted_entries_without_attempts() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "close", "{}", "k1");
    let e2 = journal(&mut c, "b2", "close", "{}", "k2");
    let e3 = journal(&mut c, "s1", "close", "{}", "k3");
    w.bd.clear_calls();
    // Every call costs 1 s: check (t=1), pre-read (t=2), e1 starts at t=2 and
    // ends at t=4, after the 3 s start gate, so e2 and e3 never start.
    w.bd.set_cost(Duration::from_secs(1));
    let r = c.drain(&budget(&w, 3, 20)).unwrap();
    assert_eq!(r.applied, vec![e1]);
    assert_eq!(r.deferred, vec![e2, e3]);
    assert!(r.deferred_in_flight.is_empty() && r.retrying.is_empty());
    assert_eq!(closes(&w), 1);
    for seq in [e2, e3] {
        let e = entry(&c, seq);
        assert_eq!((e.state, e.attempts), (JournalState::Pending, 0));
    }
    let r = c.drain(&budget(&w, 20, 23)).unwrap();
    assert_eq!(r.applied, vec![e2, e3]);
}

#[test]
fn a_start_gate_already_passed_defers_everything_without_a_bd_call() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "close", "{}", "k1");
    let e2 = journal(&mut c, "b2", "close", "{}", "k2");
    w.bd.clear_calls();
    let r = c.drain(&budget(&w, 0, 20)).unwrap();
    assert_eq!(r.deferred, vec![e1, e2]);
    assert!(r.applied.is_empty());
    assert!(w.bd.calls().is_empty(), "not even the backend check");
    assert_eq!(entry(&c, e1).attempts, 0);
}

#[test]
fn a_write_cut_by_the_hard_deadline_is_deferred_in_flight_and_found_achieved_next_drain() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "close", "{}", "k1");
    let e2 = journal(&mut c, "b2", "close", "{}", "k2");
    w.bd.inject(FakeOp::Close, Injection::Delay(Duration::from_secs(10)));
    let start = w.clock.now_mono();
    let r = c.drain(&budget(&w, 4, 5)).unwrap();
    assert_eq!(
        w.clock.now_mono() - start,
        Duration::from_secs(5),
        "the call stopped at the hard deadline"
    );
    assert_eq!(r.deferred_in_flight, vec![e1]);
    assert_eq!(r.deferred, vec![e2], "nothing starts after a cut");
    assert!(r.applied.is_empty() && r.retrying.is_empty() && r.escalated.is_empty());
    assert_eq!(r.outage, None, "a cut is no outage");
    assert_eq!(store(&c).outage().unwrap(), None);
    let e = entry(&c, e1);
    assert_eq!(
        (e.state, e.attempts),
        (JournalState::Pending, 0),
        "no attempt"
    );

    // The cut write had in fact landed in bd.
    w.bd.edit("b1", |b| b.status = Some("closed".into()));
    let r = c.drain(&budget(&w, 20, 23)).unwrap();
    assert_eq!(r.applied, vec![e1, e2]);
    assert_eq!(
        closes(&w),
        2,
        "the cut close and b2's; b1's is not repeated"
    );
}

#[test]
fn a_read_back_cut_by_the_hard_deadline_leaves_the_landed_write_pending() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "close", "{}", "k1");
    let e2 = journal(&mut c, "b2", "close", "{}", "k2");
    w.bd.inject_where(
        |c| matches!(c, FakeCall::ListByIds(ids) if *ids == [id("b1")]),
        Injection::Delay(Duration::from_secs(10)),
    );
    let r = c.drain(&budget(&w, 4, 5)).unwrap();
    assert_eq!(r.deferred_in_flight, vec![e1]);
    assert_eq!(r.deferred, vec![e2]);
    assert_eq!(r.outage, None);
    let e = entry(&c, e1);
    assert_eq!((e.state, e.attempts), (JournalState::Pending, 0));
    assert_eq!(
        w.bd_row("b1").status.as_deref(),
        Some("closed"),
        "the write landed"
    );
    assert!(
        store(&c).stale_rows().unwrap().is_empty(),
        "not a read-back failure"
    );

    let r = c.drain(&budget(&w, 20, 23)).unwrap();
    assert_eq!(r.applied, vec![e1, e2]);
    assert_eq!(closes(&w), 2, "b1 closed once");
}

#[test]
fn a_cut_backend_check_defers_the_whole_drain() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "close", "{}", "k1");
    w.bd.clear_calls();
    w.bd.set_cost_for(FakeOp::Fingerprint, Duration::from_secs(10));
    let r = c.drain(&budget(&w, 4, 5)).unwrap();
    assert_eq!(r.deferred, vec![e1]);
    assert!(r.deferred_in_flight.is_empty());
    assert_eq!(r.outage, None);
    assert_eq!(w.bd.calls(), vec![FakeCall::Fingerprint]);
    assert_eq!(entry(&c, e1).attempts, 0);
}

#[test]
fn a_cut_pre_read_defers_the_whole_drain() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "close", "{}", "k1");
    w.bd.clear_calls();
    w.bd.set_cost_for(FakeOp::ListByIds, Duration::from_secs(10));
    let r = c.drain(&budget(&w, 4, 5)).unwrap();
    assert_eq!(r.deferred, vec![e1]);
    assert_eq!(r.outage, None);
    assert!(w.bd.writes().is_empty());
}

#[test]
fn a_held_beads_unreached_entry_is_deferred_when_the_gate_passes_or_a_call_is_cut() {
    // Start gate: b1's write fails at t=3 (check, pre-read and write cost 1 s
    // each), so the gate at 3 s has passed when b1's next entry comes up.
    let w = World::new();
    let mut c = w.cache();
    let failed = journal(&mut c, "b1", "update", r#"{"append_notes": "a"}"#, "k1");
    let next = journal(&mut c, "b1", "close", "{}", "k2");
    w.bd.inject(FakeOp::Update, Injection::Failed(1));
    w.bd.set_cost(Duration::from_secs(1));
    let r = c.drain(&budget(&w, 3, 20)).unwrap();
    assert_eq!(r.retrying, vec![failed]);
    assert_eq!(r.deferred, vec![next], "unreached: deferred, not retrying");
    assert_eq!(entry(&c, failed).attempts, 1);
    assert_eq!(entry(&c, next).attempts, 0);

    // Cut: b1 is held after its failed write, b2's write is cut, and b1's next
    // entry is never reached.
    let w = World::new();
    let mut c = w.cache();
    let failed = journal(&mut c, "b1", "update", r#"{"append_notes": "a"}"#, "k1");
    let cut = journal(&mut c, "b2", "close", "{}", "k2");
    let next = journal(&mut c, "b1", "close", "{}", "k3");
    w.bd.inject(FakeOp::Update, Injection::Failed(1));
    w.bd.inject(FakeOp::Close, Injection::Delay(Duration::from_secs(10)));
    let r = c.drain(&budget(&w, 4, 5)).unwrap();
    assert_eq!(r.retrying, vec![failed]);
    assert_eq!(r.deferred_in_flight, vec![cut]);
    assert_eq!(r.deferred, vec![next], "unreached: deferred, not retrying");
    for (seq, attempts) in [(failed, 1), (cut, 0), (next, 0)] {
        let e = entry(&c, seq);
        assert_eq!((e.state, e.attempts), (JournalState::Pending, attempts));
    }
}

#[test]
fn a_passed_start_gate_defers_a_stale_held_entry_with_no_bd_call() {
    let w = World::new();
    let mut c = w.cache();
    let held = journal(&mut c, "b1", "close", "{}", "k1");
    // Stale-mark b1 as a failed read-back of an earlier entry would.
    let at = w.clock.now_utc();
    let mut seed = w.store.clone();
    let earlier = seed
        .insert_entry(
            common::new_entry("k0", "b1", "update", r#"{"append_notes": "x"}"#),
            &at,
        )
        .unwrap();
    seed.apply_with_readback(earlier, None, Some("read-back failed"), &at)
        .unwrap();
    assert_eq!(store(&c).stale_rows().unwrap(), vec![id("b1")]);
    w.bd.clear_calls();

    let r = c.drain(&budget(&w, 0, 20)).unwrap();
    assert_eq!(
        r.deferred,
        vec![held],
        "the gate stops the pass before it reaches the entry"
    );
    assert!(r.retrying.is_empty());
    assert!(w.bd.calls().is_empty());
    assert_eq!(entry(&c, held).attempts, 0);
}
