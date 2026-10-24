//! The stop rule (REQ-BDC-0707): stop when no phase is active and no entry is
//! pending or escalated-and-unannounced; never stop on uncertainty.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache_sync::{TICK_BUDGET, stop_reason};
use common::{World, scope};
use sc_beads::testing::{FakeOp, Injection};

fn stop(w: &World, cache: &mut common::TestCache) -> Option<String> {
    stop_reason(cache, &w.config(), w.after(TICK_BUDGET)).unwrap()
}

#[test]
fn test_stop_only_when_no_active_phase_and_no_open_work() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.add("b1", Some("x"), |_| {});
    let mut cache = w.cache();
    cache
        .sync(&scope("x"), w.after(Duration::from_secs(20)))
        .unwrap();

    // An active phase: keep going.
    assert_eq!(stop(&w, &mut cache), None);

    // The root closes and a sync records it: no active phase, no work: stop.
    w.close_in_bd("rx");
    w.next_tick();
    cache
        .sync(&scope("x"), w.after(Duration::from_secs(20)))
        .unwrap();
    assert!(stop(&w, &mut cache).is_some());

    // A pending entry keeps the schedule alive.
    let seq = w.journal_close(&mut cache, "b1", "k1");
    assert_eq!(stop(&w, &mut cache), None, "a pending entry is open work");

    // Escalated and not yet announced: still open work.
    w.bd.remove("b1");
    w.sender.fail(true);
    w.tick(&mut cache);
    assert_eq!(w.entry(seq).state, bd_cache_model::JournalState::Escalated);
    assert_eq!(
        stop(&w, &mut cache),
        None,
        "an unannounced escalation is open work"
    );

    // Announced: the lead has it; nothing is left for the cache to do.
    w.sender.fail(false);
    w.next_tick();
    w.tick(&mut cache);
    assert!(w.entry(seq).announced_at.is_some());
    assert!(
        stop(&w, &mut cache).is_some(),
        "announced escalations are not open work"
    );
}

#[test]
fn a_phase_whose_root_cannot_be_read_never_ends_the_schedule() {
    let w = World::new();
    // An uncached root during an outage counts as inactive for the tick, but
    // the schedule must not stop on it.
    w.phase("x", Some("lead-x"));
    let mut cache = w.cache();
    w.bd.inject(
        FakeOp::ListByIds,
        Injection::Unavailable { timed_out: false },
    );
    assert_eq!(stop(&w, &mut cache), None);
    // Once the root is readable and open, still going; closed: stop.
    assert_eq!(stop(&w, &mut cache), None);
    w.close_in_bd("rx");
    assert!(stop(&w, &mut cache).is_some());
}

#[test]
fn configuration_the_tick_would_refuse_never_ends_the_schedule() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.close_in_bd("rx");
    let mut cache = w.cache();
    assert!(stop(&w, &mut cache).is_some());

    let end = w.after(TICK_BUDGET);
    for mutate in [
        (|c: &mut bd_cache_model::TickConfig| c.team = None) as fn(&mut _),
        |c| c.fallback_lead = None,
        |c| c.phases = Some(vec!["bad name".into()]),
        |c| c.phases_dir = Some(c.phases_dir.clone().unwrap().join("missing")),
    ] {
        let mut cfg = w.config();
        mutate(&mut cfg);
        assert_eq!(stop_reason(&mut cache, &cfg, end).unwrap(), None);
    }
    // A broken phase file may be a phase we cannot see.
    std::fs::write(w.dir.path().join("phase-y.toml"), "oops").unwrap();
    assert_eq!(stop(&w, &mut cache), None);
}
