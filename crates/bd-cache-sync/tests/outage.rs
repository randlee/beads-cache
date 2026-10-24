//! A slow or unreachable backend (REQ-BDC-0903, REQ-BDC-0704; ADR-BDC-0005):
//! an outage is announced once per active scope through the sync alert
//! `backend_unavailable`, reset when it ends, never mistaken for a sync
//! failure, and never cleared by the backend check alone.
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
use bd_cache_sync::{AnnouncementKind, OUTAGE_CAUSE, RefusalCode, TickOutcome};
use common::{FALLBACK, TEAM, World, scope};

/// Phases x (lead-x) and y (lead-y), each synced once so their roots are cached.
fn two_phases() -> (World, common::TestCache) {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.phase("y", Some("lead-y"));
    w.add("bx", Some("x"), |_| {});
    w.add("by", Some("y"), |_| {});
    let mut cache = w.cache();
    for s in ["x", "y"] {
        cache
            .sync(&scope(s), w.after(Duration::from_secs(20)))
            .unwrap();
    }
    (w, cache)
}

fn alert(cache: &common::TestCache, s: &str) -> Option<String> {
    cache.sync_alert(&scope(s)).unwrap()
}

#[test]
fn test_backend_outage_is_announced_once_and_sync_failures_are_not() {
    let (w, mut cache) = two_phases();
    w.switch.set_down(true);

    let first = w.tick(&mut cache);

    // Each active scope's lead hears it once, under the stable cause key.
    assert_eq!(first.outcome, TickOutcome::PassThrough, "{first:?}");
    assert_eq!(w.sender.sent().len(), 2, "{:?}", w.sender.sent());
    for lead in ["lead-x", "lead-y"] {
        let sent = w.sender.sent_to(lead);
        assert_eq!(sent.len(), 1, "{lead}");
        assert_eq!(sent[0].team, TEAM);
        assert!(sent[0].body.contains("unreachable"), "{}", sent[0].body);
    }
    assert!(matches!(
        &first.sent[0].kind,
        AnnouncementKind::Outage { .. }
    ));
    assert_eq!(alert(&cache, "x").as_deref(), Some(OUTAGE_CAUSE));
    assert_eq!(alert(&cache, "y").as_deref(), Some(OUTAGE_CAUSE));
    assert!(cache.status().unwrap().store.outage.is_some());
    // Not a sync failure: none of the messages is about a sync.
    assert!(
        first
            .sent
            .iter()
            .all(|s| matches!(s.kind, AnnouncementKind::Outage { .. }))
    );

    // Further ticks of the same outage say nothing.
    for _ in 0..3 {
        w.next_tick();
        let again = w.tick(&mut cache);
        assert!(
            again.sent.is_empty() && again.refused.is_empty(),
            "{again:?}"
        );
    }
    assert_eq!(w.sender.sent().len(), 2);
    let since = cache.status().unwrap().store.outage.unwrap().since;

    // The backend returns: the alerts are reset and no message is sent.
    w.switch.set_down(false);
    w.next_tick();
    let back = w.tick(&mut cache);
    assert!(back.sent.is_empty(), "{back:?}");
    assert_eq!(w.sender.sent().len(), 2, "no message when an outage ends");
    assert!(cache.status().unwrap().store.outage.is_none());
    assert_eq!(alert(&cache, "x"), None);
    assert_eq!(alert(&cache, "y"), None);

    // A new outage is announced again, with a new start.
    w.switch.set_down(true);
    w.next_tick();
    w.tick(&mut cache);
    assert_eq!(w.sender.sent().len(), 4);
    assert!(cache.status().unwrap().store.outage.unwrap().since > since);
}

#[test]
fn test_outage_with_uncached_roots_still_tells_every_configured_lead() {
    // Nothing is cached: the roots must be read from bd, and bd is down, so
    // no phase can be shown active. Every configured lead still hears of it.
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.phase("y", Some("lead-y"));
    w.phase("z", None); // no lead: the fallback lead
    let mut cache = w.cache();
    w.switch.set_down(true);

    let reply = w.tick(&mut cache);

    assert!(reply.active.is_empty(), "{reply:?}");
    assert_eq!(reply.unreadable_roots.len(), 3);
    let mut told: Vec<String> = w.sender.sent().into_iter().map(|s| s.lead).collect();
    told.sort();
    assert_eq!(
        told,
        ["lead-x", "lead-y", FALLBACK],
        "{:?}",
        w.sender.sent()
    );
    assert_eq!(alert(&cache, "z").as_deref(), Some(OUTAGE_CAUSE));

    // Once each, however long it lasts.
    w.next_tick();
    assert!(w.tick(&mut cache).sent.is_empty());
    assert_eq!(w.sender.sent().len(), 3);
}

#[test]
fn test_outage_is_not_cleared_while_only_the_backend_check_answers() {
    let (w, mut cache) = two_phases();
    w.switch.set_down(true);
    w.tick(&mut cache);
    let outage = cache.status().unwrap().store.outage.unwrap();
    w.sender.clear();

    // `bd version` passes, but every list is unreachable: still an outage.
    w.switch.set_down(false);
    w.switch.set_lists_down(true);
    for _ in 0..2 {
        w.next_tick();
        let reply = w.tick(&mut cache);
        assert!(reply.sent.is_empty(), "no second announcement: {reply:?}");
        let now = cache.status().unwrap().store.outage.unwrap();
        assert_eq!(now.since, outage.since, "the outage keeps its start");
        assert_eq!(alert(&cache, "x").as_deref(), Some(OUTAGE_CAUSE));
        assert_eq!(alert(&cache, "y").as_deref(), Some(OUTAGE_CAUSE));
    }

    // A list that answers ends it, and the alerts with it.
    w.switch.set_lists_down(false);
    w.next_tick();
    w.tick(&mut cache);
    assert!(cache.status().unwrap().store.outage.is_none());
    assert_eq!(alert(&cache, "x"), None);
    assert!(w.sender.sent().is_empty(), "recovery is silent");
}

#[test]
fn test_undelivered_outage_announcement_is_retried() {
    let (w, mut cache) = two_phases();
    w.switch.set_down(true);
    w.sender.fail(true);

    let reply = w.tick(&mut cache);

    assert_eq!(reply.outcome, TickOutcome::Refused, "{reply:?}");
    assert_eq!(reply.refused.len(), 2, "{:?}", reply.refused);
    assert!(
        reply
            .refused
            .iter()
            .all(|r| r.code == RefusalCode::OutageUndelivered)
    );
    assert_eq!(
        alert(&cache, "x"),
        None,
        "not recorded: it was not delivered"
    );

    // Only the lead whose message failed is retried.
    w.next_tick();
    assert_eq!(w.tick(&mut cache).outcome, TickOutcome::Refused);
    w.sender.fail(false);
    w.next_tick();
    let ok = w.tick(&mut cache);
    assert_eq!(ok.outcome, TickOutcome::PassThrough, "{ok:?}");
    assert_eq!(w.sender.sent().len(), 2);
    w.next_tick();
    assert!(
        w.tick(&mut cache).sent.is_empty(),
        "once delivered, never again"
    );
}

#[test]
fn a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once() {
    let w = World::new();
    w.phase("x", None);
    w.phase("y", Some("shared"));
    w.phase("z", Some("shared"));
    let mut cache = w.cache();
    for s in ["x", "y", "z"] {
        cache
            .sync(&scope(s), w.after(Duration::from_secs(20)))
            .unwrap();
    }
    w.switch.set_down(true);

    let reply = w.tick(&mut cache);

    assert_eq!(w.sender.sent_to(FALLBACK).len(), 1);
    assert_eq!(
        w.sender.sent_to("shared").len(),
        1,
        "two scopes, one message"
    );
    for s in ["x", "y", "z"] {
        assert_eq!(alert(&cache, s).as_deref(), Some(OUTAGE_CAUSE), "{s}");
    }
    let shared = reply
        .sent
        .iter()
        .find(|a| a.lead.as_str() == "shared")
        .map(|a| a.kind.clone());
    assert!(matches!(shared, Some(AnnouncementKind::Outage { phases }) if phases.len() == 2));
}

/// During an outage of several ticks: reads of cached beads answer with
/// `outage` set, no escalation is sent, nothing is lost, and the journal
/// drains on recovery.
#[test]
fn during_an_outage_of_several_ticks_reads_report_it_writes_wait_and_recovery_drains() {
    let (w, mut cache) = two_phases();
    let seq = w.journal_close(&mut cache, "bx", "k1");
    w.switch.set_down(true);

    // More ticks than MAX_ATTEMPTS: the attempts rule must hold.
    for _ in 0..8 {
        let reply = w.tick(&mut cache);
        assert!(reply.refused.is_empty(), "{reply:?}");
        let got = cache.get(&common::id("bx"), None).unwrap();
        assert!(got.outage.is_some(), "reads say so");
        assert_eq!(
            got.row.bead.status.as_deref(),
            Some("closed"),
            "the journaled close is visible"
        );
        assert_eq!(got.pending, 1);
        let list = cache.list(&scope("x"), None, None).unwrap();
        assert!(list.outage.is_some());
        w.next_tick();
    }
    let entry = w.entry(seq);
    assert_eq!(entry.state, JournalState::Pending);
    assert_eq!(
        entry.attempts, 0,
        "an unreachable backend counts no attempt"
    );
    assert!(cache.status().unwrap().store.escalated.is_empty());
    // Exactly the two outage messages, nothing about the write.
    assert_eq!(w.sender.sent().len(), 2);

    w.switch.set_down(false);
    let back = w.tick(&mut cache);
    assert_eq!(back.drain.as_ref().unwrap().applied, [seq], "{back:?}");
    assert_eq!(w.bd.row("bx").unwrap().status.as_deref(), Some("closed"));
    assert!(cache.get(&common::id("bx"), None).unwrap().outage.is_none());
    assert_eq!(w.sender.sent().len(), 2, "recovery is silent");
}

/// The alert is reset when the outage ends even for a scope that is not
/// synced that tick (here its root was closed by a journaled write, so the
/// phase is no longer active), and the lead is not told.
#[test]
fn the_alert_of_a_scope_that_is_not_synced_is_reset_when_the_outage_ends() {
    let (w, mut cache) = two_phases();
    w.switch.set_down(true);
    w.tick(&mut cache);
    assert_eq!(alert(&cache, "x").as_deref(), Some(OUTAGE_CAUSE));
    w.sender.clear();

    // During the outage the lead of phase x closes its root: journaled, visible at once.
    let seq = w.journal_close(&mut cache, "rx", "k-root");
    w.switch.set_down(false);
    w.next_tick();
    let back = w.tick(&mut cache);

    assert_eq!(back.drain.as_ref().unwrap().applied, [seq], "{back:?}");
    assert_eq!(
        back.active,
        [scope("y").phase_name().clone()],
        "x is no longer active"
    );
    assert!(cache.status().unwrap().store.outage.is_none());
    assert_eq!(alert(&cache, "x"), None, "reset although x was not synced");
    assert_eq!(alert(&cache, "y"), None);
    assert!(w.sender.sent().is_empty(), "recovery is silent");
}

/// Outage targets are the active phases and those whose root could not be read,
/// never a phase known to be closed (ADR-SYNC-0003).
#[test]
fn an_outage_is_not_told_to_the_lead_of_a_phase_known_to_be_closed() {
    let w = World::new();
    w.phase("x", Some("lead-x")); // open and cached: active
    w.phase("y", Some("lead-y")); // closed and cached: inactive
    w.phase("z", Some("lead-z")); // root never read: unreadable in the outage
    w.close_in_bd("ry");
    let mut cache = w.cache();
    for s in ["x", "y"] {
        cache
            .sync(&scope(s), w.after(Duration::from_secs(20)))
            .unwrap();
    }
    assert!(
        cache
            .list(&scope("y"), None, None)
            .unwrap()
            .rows
            .iter()
            .any(|r| r.bead.id == common::id("ry") && r.bead.status.as_deref() == Some("closed"))
    );
    w.switch.set_down(true);

    let reply = w.tick(&mut cache);

    let mut told: Vec<String> = w.sender.sent().into_iter().map(|s| s.lead).collect();
    told.sort();
    assert_eq!(told, ["lead-x", "lead-z"], "{reply:?}");
    assert_eq!(alert(&cache, "y"), None);
    assert_eq!(reply.unreadable_roots, [scope("z").phase_name().clone()]);
}

/// An outage alert in `bd_cache.py`'s form (`"backend_unavailable: <detail>"`)
/// already told the lead: the tick reads the cause, not the raw text, so it
/// does not announce the same outage again (RBP-F031, S21).
#[test]
fn an_outage_alert_in_bd_cache_pys_form_is_not_announced_again() {
    let (w, mut cache) = two_phases();
    cache
        .set_sync_alert(
            &scope("x"),
            Some("backend_unavailable: BD_CACHE.BD_UNAVAILABLE: bd did not answer"),
        )
        .unwrap();
    w.switch.set_down(true);

    let reply = w.tick(&mut cache);

    let told: Vec<String> = w.sender.sent().into_iter().map(|s| s.lead).collect();
    assert_eq!(told, ["lead-y"], "{reply:?}");
    // The Python alert is left as it is; y now carries this crate's key.
    assert!(
        alert(&cache, "x")
            .unwrap()
            .starts_with("backend_unavailable: ")
    );
    assert_eq!(alert(&cache, "y").as_deref(), Some(OUTAGE_CAUSE));

    // When the outage ends, the Python-form alert is reset with the rest.
    w.switch.set_down(false);
    w.next_tick();
    w.tick(&mut cache);
    assert_eq!(alert(&cache, "x"), None);
}
