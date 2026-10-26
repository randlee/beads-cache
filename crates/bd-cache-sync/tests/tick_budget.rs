//! The tick budget (REQ-BDC-0703, NFR-BDC-0004): a 25 s budget ending at `T`,
//! a drain with start gate `T - 5 s` and hard deadline `T - 2 s`, and a sync
//! of each phase deferred when not started by `T - 2 s`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache_sync::{TICK_BUDGET, TickOutcome};
use common::{World, scope};
use sc_beads::testing::FakeOp;

const SECOND: Duration = Duration::from_secs(1);

/// A world with phase x synced once (so its root is cached) and `n` journaled closes.
fn world_with_entries(n: usize) -> (World, common::TestCache, Vec<bd_cache_model::Seq>) {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let mut cache = w.cache();
    let mut seqs = Vec::new();
    for i in 1..=n {
        w.add(&format!("b{i}"), Some("x"), |_| {});
    }
    cache
        .sync_legacy(&scope("x"), w.after(20 * SECOND))
        .unwrap();
    for i in 1..=n {
        seqs.push(w.journal_close(&mut cache, &format!("b{i}"), &format!("k{i}")));
    }
    (w, cache, seqs)
}

#[test]
fn test_tick_budget_defers_drain_entries() {
    let (w, mut cache, seqs) = world_with_entries(3);
    // Every bd call takes 6 s: the backend check ends at 6 s, the pre-read at
    // 12 s, the first write at 18 s, which is before the start gate (T - 5 s
    // = 20 s) but its read-back would end at 24 s, past the hard deadline
    // (T - 2 s = 23 s).
    w.bd.set_cost(6 * SECOND);
    w.bd.clear_calls(); // forget the setup's sync
    let start = w.clock.elapsed();

    let reply = w.tick(&mut cache);

    let drain = reply.drain.as_ref().unwrap();
    assert_eq!(drain.deferred_in_flight, [seqs[0]], "{drain:?}");
    assert_eq!(drain.deferred, [seqs[1], seqs[2]], "{drain:?}");
    assert!(drain.applied.is_empty() && drain.escalated.is_empty());
    for seq in &seqs {
        let e = w.entry(*seq);
        assert_eq!(e.attempts, 0, "a deferred entry costs no attempt");
        assert_eq!(e.state, bd_cache_model::JournalState::Pending);
    }
    // The drain used the clock up to the hard deadline; the phase's sync was
    // then not started (T - 2 s has passed) and is reported deferred.
    assert_eq!(reply.deferred, [scope("x").scope_name().clone()]);
    assert!(reply.synced.is_empty());
    assert!(
        w.clock.elapsed().saturating_sub(start) <= TICK_BUDGET,
        "the tick took {:?}",
        w.clock.elapsed().saturating_sub(start)
    );
    // The deferred sync made no bd call at all: only the drain's backend check
    // and none of the sync's.
    let calls = w.bd.calls();
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.op() == FakeOp::Fingerprint)
            .count(),
        1,
        "{calls:?}"
    );
    assert!(
        calls.iter().all(|c| c.op() != FakeOp::ListScope),
        "{calls:?}"
    );
    // A deferral is not a refusal.
    assert_eq!(reply.outcome, TickOutcome::PassThrough, "{reply:?}");
}

#[test]
fn a_roomy_budget_drains_and_syncs_every_active_phase() {
    let (w, mut cache, seqs) = world_with_entries(2);
    let reply = w.tick(&mut cache);
    let drain = reply.drain.as_ref().unwrap();
    assert_eq!(drain.applied, seqs);
    assert!(drain.deferred.is_empty());
    assert_eq!(reply.synced.len(), 1);
    assert!(reply.deferred.is_empty());
}

#[test]
fn the_drain_runs_before_the_sync() {
    let (w, mut cache, _) = world_with_entries(1);
    w.bd.clear_calls();
    w.tick(&mut cache);
    let ops: Vec<FakeOp> =
        w.bd.calls()
            .iter()
            .map(sc_beads::testing::FakeCall::op)
            .collect();
    let write = ops.iter().position(|o| o.is_write()).unwrap();
    let list_scope = ops.iter().position(|o| *o == FakeOp::ListScope).unwrap();
    assert!(write < list_scope, "{ops:?}");
}

/// Every bd call times out: no tick runs past its 25 s budget, with the
/// roots cached or not, with entries pending or not, over several ticks.
#[test]
fn no_tick_exceeds_25_s_with_every_bd_call_timing_out() {
    for cached_roots in [true, false] {
        for entries in [0, 3] {
            let w = World::new();
            w.phase("x", Some("lead-x"));
            w.phase("y", Some("lead-y"));
            w.add("b1", Some("x"), |_| {});
            let mut cache = w.cache();
            if cached_roots {
                cache
                    .sync_legacy(&scope("x"), w.after(20 * SECOND))
                    .unwrap();
                cache
                    .sync_legacy(&scope("y"), w.after(20 * SECOND))
                    .unwrap();
            }
            for i in 0..entries {
                let bead = format!("e{i}");
                w.add(&bead, Some("x"), |_| {});
                w.bd.edit(&bead, |_| {});
                w.journal_close(&mut cache, &bead, &format!("k{i}"));
            }
            // Calls take longer than the per-call cap and the whole budget.
            w.bd.set_cost(60 * SECOND);
            for tick in 0..3 {
                let start = w.clock.elapsed();
                let reply = w.tick(&mut cache);
                let took = w.clock.elapsed().saturating_sub(start);
                assert!(
                    took <= TICK_BUDGET,
                    "tick {tick} (cached roots: {cached_roots}, entries: {entries}) took {took:?}: {reply:?}"
                );
                w.next_tick();
            }
            // A timeout at the deadline is a cut, not an outage, and costs no attempt.
            let status = cache.status().unwrap();
            assert!(status.store.outage.is_none(), "{status:?}");
            assert!(status.store.escalated.is_empty());
        }
    }
}

/// The drain keeps its own budget and does not look at the stop flag (S20): a
/// stop that arrives while it runs is seen when it returns. The worst-case delay
/// is the drain's hard deadline, `T - 2 s`: the start gate stops new entries at
/// `T - 5 s` and the entry in flight is capped at `T - 2 s`. After it the tick
/// starts no phase and sends nothing.
#[test]
fn a_stop_that_arrives_mid_drain_is_seen_within_the_drains_hard_deadline() {
    let (w, mut cache, seqs) = world_with_entries(4);
    let stop = bd_cache_sync::StopFlag::new();
    {
        let stop = stop.clone();
        w.switch.on_fingerprint(move || stop.stop());
    }
    w.bd.set_cost(4 * SECOND);
    w.bd.clear_calls();
    let start = w.clock.elapsed();
    let settings = bd_cache_sync::TickSettings::from_config(&w.config()).unwrap();

    let reply = bd_cache_sync::tick_with(
        &mut cache,
        &settings,
        &*w.sender,
        w.after(TICK_BUDGET),
        &stop,
    )
    .unwrap();

    let took = w.clock.elapsed().saturating_sub(start);
    assert!(reply.stopped, "{reply:?}");
    assert!(reply.drain.is_some(), "the drain ran to its own end");
    assert!(
        reply.synced.is_empty() && reply.active.is_empty(),
        "no phase after the stop"
    );
    assert!(reply.sent.is_empty());
    assert!(
        took + bd_cache_model::HARD_MARGIN <= TICK_BUDGET,
        "the stop took {took:?} to be seen; the bound is the drain's hard deadline"
    );
    let _ = seqs;
}
