//! Announcements (REQ-BDC-0704, 0705, 0706): sync failures once per cause,
//! escalations once with the `resolve` command, undelivered messages refused
//! and retried.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache_model::JournalState;
use bd_cache_sync::{AnnouncementKind, TickOutcome};
use common::{FALLBACK, TEAM, World, id, scope};
use sc_beads::testing::{FakeOp, Injection};

/// A world with phase x (lead `lead-x`) synced once, and an escalated entry
/// on `b1`: written against a cached bead, then deleted from bd before the drain.
fn world_with_escalation() -> (World, common::TestCache, bd_cache_model::Seq) {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.add("b1", Some("x"), |_| {});
    let mut cache = w.cache();
    cache
        .sync(&scope("x"), w.after(Duration::from_secs(20)))
        .unwrap();
    let seq = w.journal_close(&mut cache, "b1", "k1");
    w.bd.remove("b1");
    (w, cache, seq)
}

#[test]
fn test_escalation_is_sent_once_to_the_phase_lead() {
    let (w, mut cache, seq) = world_with_escalation();

    let reply = w.tick(&mut cache);

    assert_eq!(w.entry(seq).state, JournalState::Escalated, "{reply:?}");
    let sent = w.sender.sent_to("lead-x");
    assert_eq!(sent.len(), 1, "{:?}", w.sender.sent());
    assert_eq!(sent[0].team, TEAM);
    // The message names the entry, the error and the recovery.
    let body = &sent[0].body;
    assert!(
        body.contains(&format!("#{seq}")) && body.contains("b1"),
        "{body}"
    );
    assert!(body.contains("close") && body.contains("san"), "{body}");
    assert!(
        body.contains("NOT_FOUND") || body.contains("not found"),
        "{body}"
    );
    assert!(
        body.contains(&format!("resolve {seq} --note")) && body.contains("--actor"),
        "{body}"
    );
    assert_eq!(
        reply.sent[0].kind,
        AnnouncementKind::Escalation { seq },
        "{reply:?}"
    );
    assert!(w.entry(seq).announced_at.is_some(), "marked announced");
    assert_eq!(reply.outcome, TickOutcome::PassThrough);

    // Marked: not sent again, however many ticks follow.
    for _ in 0..3 {
        w.next_tick();
        let again = w.tick(&mut cache);
        assert!(again.sent.is_empty(), "{again:?}");
    }
    assert_eq!(w.sender.sent().len(), 1);
}

#[test]
fn test_undelivered_escalation_is_retried_and_the_tick_refuses() {
    let (w, mut cache, seq) = world_with_escalation();
    w.sender.fail(true);

    let reply = w.tick(&mut cache);

    assert_eq!(reply.outcome, TickOutcome::Refused);
    assert_eq!(reply.refused.len(), 1, "{:?}", reply.refused);
    assert!(
        reply.refused[0].contains(&format!("#{seq}")) && reply.refused[0].contains("not delivered"),
        "{:?}",
        reply.refused
    );
    assert!(reply.sent.is_empty());
    assert!(w.entry(seq).announced_at.is_none(), "still unannounced");
    assert!(cache.open_work().unwrap().escalated_unannounced > 0);

    // Retried every tick while the sender fails...
    w.next_tick();
    assert_eq!(w.tick(&mut cache).outcome, TickOutcome::Refused);
    assert_eq!(w.sender.attempts().len(), 2);

    // ...and delivered, once, when it recovers.
    w.sender.fail(false);
    w.next_tick();
    let ok = w.tick(&mut cache);
    assert_eq!(ok.outcome, TickOutcome::PassThrough, "{ok:?}");
    assert_eq!(w.sender.sent_to("lead-x").len(), 1);
    assert!(w.entry(seq).announced_at.is_some());
}

#[test]
fn an_entry_whose_phase_has_no_lead_is_refused_every_tick_until_it_has_one() {
    let w = World::new();
    w.phase("x", None); // no lead in the phase file
    w.add("b1", Some("x"), |_| {});
    let mut cache = w.cache();
    cache
        .sync(&scope("x"), w.after(Duration::from_secs(20)))
        .unwrap();
    let seq = w.journal_close(&mut cache, "b1", "k1");
    w.bd.remove("b1");

    for _ in 0..2 {
        let reply = w.tick(&mut cache);
        assert_eq!(reply.outcome, TickOutcome::Refused, "{reply:?}");
        assert!(reply.refused[0].contains("no lead"), "{:?}", reply.refused);
        w.next_tick();
    }
    assert!(w.sender.attempts().is_empty(), "nobody to send to");
    assert!(w.entry(seq).announced_at.is_none());

    // The lead is added to the phase file; the next tick delivers.
    w.phase_file("x", "rx", Some("late-lead"));
    let reply = w.tick(&mut cache);
    assert_eq!(reply.outcome, TickOutcome::PassThrough, "{reply:?}");
    assert_eq!(w.sender.sent_to("late-lead").len(), 1);
}

/// An escalation on an entry with no scope goes to `fallback_lead`, and the
/// stop rule still fires once it is announced.
#[test]
fn an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.close_in_bd("rx"); // no active phase
    // `o1` is in no scope: a get miss stores it with no membership.
    w.add("o1", None, |_| {});
    let mut cache = w.cache();
    cache.get(&id("o1"), None).unwrap();
    let seq = w.journal_close(&mut cache, "o1", "k1");
    assert!(w.entry(seq).scope.is_none());
    w.bd.remove("o1");
    let end = w.after(bd_cache_sync::TICK_BUDGET);

    let reply = w.tick(&mut cache);

    assert_eq!(w.entry(seq).state, JournalState::Escalated, "{reply:?}");
    let sent = w.sender.sent_to(FALLBACK);
    assert_eq!(sent.len(), 1, "{:?}", w.sender.sent());
    assert_eq!(sent[0].team, TEAM);
    assert!(sent[0].body.contains("resolve"));
    assert!(w.sender.sent_to("lead-x").is_empty());
    // Announced: no open work, no active phase: the schedule stops.
    assert!(
        bd_cache_sync::stop_reason(&mut cache, &w.config(), &*w.clock, end)
            .unwrap()
            .is_some()
    );
}

#[test]
fn test_sync_failure_is_announced_once_per_cause() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let mut cache = w.cache();
    let x = scope("x");
    let fail_list = |code| w.bd.inject(FakeOp::ListScope, Injection::Failed(code));

    // A reachable bd whose list fails: SYNC_FAILED (BD_LIST_FAILED), told once.
    fail_list(1);
    let first = w.tick(&mut cache);
    assert_eq!(first.outcome, TickOutcome::PassThrough, "{first:?}");
    let sent = w.sender.sent_to("lead-x");
    assert_eq!(sent.len(), 1, "{:?}", w.sender.sent());
    assert!(sent[0].body.contains("BD_LIST_FAILED"), "{}", sent[0].body);
    assert!(sent[0].body.contains("phase x"), "{}", sent[0].body);
    assert!(matches!(
        &first.sent[0].kind,
        AnnouncementKind::SyncFailure { cause, .. } if cause == "BD_CACHE.BD_LIST_FAILED"
    ));
    assert_eq!(
        cache.sync_alert(&x).unwrap().as_deref(),
        Some("BD_CACHE.BD_LIST_FAILED")
    );

    // The same cause again: not told again.
    w.next_tick();
    fail_list(1);
    let second = w.tick(&mut cache);
    assert!(second.sent.is_empty(), "{second:?}");
    assert_eq!(w.sender.sent().len(), 1);

    // A different cause is a new message.
    w.next_tick();
    w.bd.inject(FakeOp::Fingerprint, Injection::Failed(1));
    let third = w.tick(&mut cache);
    assert_eq!(w.sender.sent_to("lead-x").len(), 2, "{third:?}");
    assert!(
        w.sender.sent_to("lead-x")[1]
            .body
            .contains("BD_VERSION_FAILED")
    );
    assert_eq!(
        cache.sync_alert(&x).unwrap().as_deref(),
        Some("BD_CACHE.BD_VERSION_FAILED")
    );

    // A passing sync clears the alert...
    w.next_tick();
    let ok = w.tick(&mut cache);
    assert_eq!(ok.synced.len(), 1, "{ok:?}");
    assert_eq!(cache.sync_alert(&x).unwrap(), None);
    // ...so the same failure is announced again afterwards.
    w.next_tick();
    fail_list(1);
    w.tick(&mut cache);
    assert_eq!(w.sender.sent_to("lead-x").len(), 3);
}

#[test]
fn an_undelivered_sync_failure_is_refused_and_retried_not_recorded() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let mut cache = w.cache();
    w.sender.fail(true);
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    let reply = w.tick(&mut cache);
    assert_eq!(reply.outcome, TickOutcome::Refused, "{reply:?}");
    assert!(reply.refused[0].contains("sync failure of phase x"));
    assert_eq!(cache.sync_alert(&scope("x")).unwrap(), None, "not recorded");

    // The cause persists; the sender is back; the next tick tells the lead.
    w.sender.fail(false);
    w.next_tick();
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    let reply = w.tick(&mut cache);
    assert_eq!(reply.outcome, TickOutcome::PassThrough, "{reply:?}");
    assert_eq!(w.sender.sent_to("lead-x").len(), 1);
}

#[test]
fn a_phase_without_a_lead_hears_about_a_sync_failure_through_the_fallback_lead() {
    let w = World::new();
    w.phase("x", None);
    let mut cache = w.cache();
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    w.tick(&mut cache);
    assert_eq!(w.sender.sent_to(FALLBACK).len(), 1, "{:?}", w.sender.sent());
}
