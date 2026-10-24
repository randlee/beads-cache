//! An unreachable backend at the check or pre-read is an outage: the drain stops,
//! counts no attempt, and an answering list clears it (ADR-BDC-0005; REQ-BDC-0403, 0404).
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
use bd_cache_model::{CacheStore, DrainBudget, IdemKey, JournalState, Seq};
use common::{TestCache, World, id};
use sc_beads::testing::{FakeOp, Injection, actor};
use sc_beads::{Clock, Guards};

fn journal(c: &mut TestCache, bead: &str, key: &str) -> Seq {
    c.write(
        &id(bead),
        "close",
        "{}",
        Guards::default(),
        IdemKey::new(key).unwrap(),
        actor("san"),
    )
    .unwrap_or_else(|e| panic!("write {bead}: {e}"))
    .seq
}

fn budget(w: &World, gate_s: u64, hard_s: u64) -> DrainBudget {
    DrainBudget {
        start_gate: w.after(Duration::from_secs(gate_s)),
        hard: w.after(Duration::from_secs(hard_s)),
    }
}

#[test]
fn test_unreachable_backend_is_an_outage_that_counts_no_attempt() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "k1");
    let e2 = journal(&mut c, "b2", "k2");
    let drains = w.config.max_attempts + 2;
    let mut since = None;
    for n in 0..drains {
        // Alternate: unreachable at the backend check, then at the pre-read
        // (the check passing does not clear the outage).
        let at = if n % 2 == 0 {
            FakeOp::Fingerprint
        } else {
            FakeOp::ListByIds
        };
        w.bd.inject(at, Injection::Unavailable { timed_out: false });
        w.bd.clear_calls();
        let r = c.drain(&budget(&w, 20, 23)).unwrap();
        assert_eq!(
            r.deferred,
            vec![e1, e2],
            "drain {n}: stopped, every entry deferred"
        );
        assert!(r.applied.is_empty() && r.retrying.is_empty() && r.escalated.is_empty());
        assert!(w.bd.writes().is_empty(), "drain {n}: no write");
        let outage = r
            .outage
            .clone()
            .unwrap_or_else(|| panic!("drain {n}: no outage"));
        assert!(outage.last_error.starts_with("BD_CACHE.BD_UNAVAILABLE"));
        let first = since.get_or_insert_with(|| outage.since.clone());
        assert_eq!(
            &outage.since, first,
            "drain {n}: the outage keeps its start"
        );
        assert_eq!(store(&c).outage().unwrap(), Some(outage));
        for seq in [e1, e2] {
            let e = store(&c).entry_by_seq(seq).unwrap().unwrap();
            assert_eq!(
                (e.state, e.attempts),
                (JournalState::Pending, 0),
                "drain {n}"
            );
        }
        w.clock.advance(Duration::from_secs(30));
    }

    // bd answers again: the pre-read clears the outage and the entries apply.
    let r = c.drain(&budget(&w, 20, 23)).unwrap();
    assert_eq!(r.applied, vec![e1, e2]);
    assert_eq!(r.outage, None);
    assert_eq!(store(&c).outage().unwrap(), None);
}

#[test]
fn a_pre_read_timing_out_at_the_call_cap_is_an_outage_not_a_cut() {
    let w = World::new();
    let mut c = w.cache();
    let e1 = journal(&mut c, "b1", "k1");
    // 31 s exceeds the 30 s call cap but not the 120 s hard deadline.
    w.bd.inject(FakeOp::ListByIds, Injection::Delay(Duration::from_secs(31)));
    let start = w.clock.now_mono();
    let r = c.drain(&budget(&w, 100, 120)).unwrap();
    assert_eq!(w.clock.now_mono() - start, w.config.call_timeout);
    assert_eq!(r.deferred, vec![e1]);
    assert!(r.deferred_in_flight.is_empty());
    assert!(r.outage.is_some());
    assert_eq!(store(&c).entry_by_seq(e1).unwrap().unwrap().attempts, 0);
}

#[test]
fn a_drain_that_calls_no_bd_still_reports_the_standing_outage() {
    let w = World::new();
    let mut c = w.cache();
    let seq = journal(&mut c, "b1", "k1");
    w.bd.inject(
        FakeOp::Fingerprint,
        Injection::Unavailable { timed_out: true },
    );
    assert!(c.drain(&budget(&w, 20, 23)).unwrap().outage.is_some());
    // Start gate already passed: no bd call, so nothing can clear the outage.
    w.bd.clear_calls();
    let r = c.drain(&budget(&w, 0, 0)).unwrap();
    assert_eq!(r.deferred, vec![seq]);
    assert!(w.bd.calls().is_empty());
    assert_eq!(r.outage, store(&c).outage().unwrap());
    assert!(r.outage.is_some());
}
