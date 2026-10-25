//! `resolve`, `prune`, `status` and the announcement passthroughs
//! (REQ-BDC-0601, 0602, 0603; ADR-BDC-0005; deliverable 6).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "test code: a failed unwrap or index is a test failure; w, c, r name the world, cache and reply"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::{store, store_mut};
use bd_cache_model::{CacheError, CacheStore, JournalState, OpenWork, Seq};
use common::{TestCache, World, id, new_entry, ts, x};
use sc_beads::Clock;
use sc_beads::testing::{FakeOp, Injection, actor};

const T30: Duration = Duration::from_secs(30);

fn synced(w: &World) -> TestCache {
    let mut c = w.cache();
    c.sync(&x(), w.after(T30)).unwrap();
    c
}

fn journal(c: &mut TestCache, w: &World, key: &str, bead: &str) -> Seq {
    let at = w.clock.now_utc();
    store_mut(c)
        .insert_entry(new_entry(key, bead, "close", "{}"), &at)
        .unwrap()
}

fn escalate(c: &mut TestCache, w: &World, seq: Seq, reason: &str) {
    let at = w.clock.now_utc();
    store_mut(c).escalate(seq, reason, &at).unwrap();
}

#[test]
fn test_prune_refuses_with_unapplied_writes() {
    let w = World::new();
    let mut c = synced(&w);
    let seq = journal(&mut c, &w, "k1", "b2");
    let e = c.prune(&x()).unwrap_err();
    assert!(matches!(e, CacheError::PruneBlocked { .. }), "{e:?}");
    escalate(&mut c, &w, seq, "precondition no longer holds");
    let e = c.prune(&x()).unwrap_err();
    assert_eq!(
        e.code(),
        "BD_CACHE.PRUNE_BLOCKED",
        "escalated entries block too"
    );
    assert_eq!(
        w.store.row_ids().len(),
        3,
        "a refused prune deletes nothing"
    );
}

#[test]
fn prune_deletes_the_scopes_beads_journal_sync_state_and_ready_base() {
    let w = World::new();
    let mut c = synced(&w);
    let seq = journal(&mut c, &w, "k1", "b2");
    let at = w.clock.now_utc();
    store_mut(&mut c)
        .apply_with_readback(seq, Some(&w.bd_row("b2")), None, &at)
        .unwrap();
    let r = c.prune(&x()).unwrap();
    assert_eq!((r.scope.clone(), r.beads_deleted), (x(), 3));
    assert!(w.store.row_ids().is_empty() && w.store.journal().is_empty());
    assert_eq!(store(&c).sync_state(&x()).unwrap().last_ok_at, None);
    assert_eq!(store(&c).ready_base(&x()).unwrap(), None);
    assert!(
        c.list(&x(), None, None).unwrap().rows.is_empty(),
        "the scope stays known"
    );
}

#[test]
fn test_resolve_closes_only_escalated_entries_and_unblocks_prune() {
    let w = World::new();
    let mut c = synced(&w);
    let pending = journal(&mut c, &w, "k1", "b1");
    let e = c
        .resolve(pending, "set by hand", actor("lead"))
        .unwrap_err();
    assert!(matches!(e, CacheError::NotEscalated { .. }), "{e:?}");
    let e = c
        .resolve(Seq::new(42), "set by hand", actor("lead"))
        .unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.NOT_ESCALATED", "an absent seq too");

    escalate(&mut c, &w, pending, "precondition no longer holds");
    assert_eq!(c.prune(&x()).unwrap_err().code(), "BD_CACHE.PRUNE_BLOCKED");
    c.resolve(pending, "set by hand", actor("lead")).unwrap();
    let entry = c.entry(pending).unwrap();
    assert_eq!(entry.state, JournalState::Resolved);
    assert_eq!(
        (entry.last_error.as_deref(), entry.note.as_deref()),
        (
            Some("precondition no longer holds"),
            Some("resolved by lead: set by hand")
        ),
        "the note has its own field; last_error is left as it was"
    );
    assert_eq!(
        entry.legacy_last_error().as_deref(),
        Some("precondition no longer holds | resolved by lead: set by hand"),
        "the 0.1 rendering appends the note as bd_cache.py does"
    );
    assert_eq!(
        c.get(&id("b1"), None).unwrap().row.bead.status.as_deref(),
        Some("in_progress"),
        "a resolved entry is never replayed"
    );
    let e = c.resolve(pending, "again", actor("lead")).unwrap_err();
    assert_eq!(
        e.code(),
        "BD_CACHE.NOT_ESCALATED",
        "a resolved entry is not escalated"
    );
    c.prune(&x()).unwrap();

    // An applied entry is not escalated either.
    let w = World::new();
    let mut c = synced(&w);
    let applied = journal(&mut c, &w, "k1", "b2");
    let at = w.clock.now_utc();
    store_mut(&mut c)
        .apply_with_readback(applied, Some(&w.bd_row("b2")), None, &at)
        .unwrap();
    let e = c.resolve(applied, "n", actor("lead")).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.NOT_ESCALATED");
}

#[test]
fn test_resolve_keeps_the_note_when_the_entry_had_no_error() {
    let w = World::new();
    let mut c = synced(&w);
    let seq = journal(&mut c, &w, "k1", "b1");
    escalate(&mut c, &w, seq, "");
    c.resolve(seq, "set by hand", actor("lead")).unwrap();
    let entry = c.entry(seq).unwrap();
    assert_eq!(entry.note.as_deref(), Some("resolved by lead: set by hand"));
    assert_eq!(
        entry.legacy_last_error().as_deref(),
        Some(" | resolved by lead: set by hand")
    );
}

#[test]
fn status_reports_each_scopes_last_ok_at_the_journal_escalations_and_the_outage() {
    let w = World::new();
    let start = w.clock.now_utc();
    let mut c = synced(&w);
    let a = journal(&mut c, &w, "k1", "b1");
    journal(&mut c, &w, "k2", "b2");
    escalate(&mut c, &w, a, "retries exhausted");
    let st = c.status().unwrap().store;
    assert_eq!(st.scopes.len(), 1);
    assert_eq!(st.scopes[0].sync.scope, x());
    assert_eq!(st.scopes[0].sync.last_ok_at, Some(start));
    assert_eq!(st.scopes[0].beads, 3);
    assert_eq!(st.journal.get("x:escalated"), Some(&1));
    assert_eq!(st.journal.get("x:pending"), Some(&1));
    assert_eq!(
        st.escalated.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![a]
    );
    assert!(st.meta.contains_key("bd_schema_sha256"));
    assert_eq!(st.outage, None);
    assert!(st.stale.is_empty());

    w.bd.inject(
        FakeOp::ListScope,
        Injection::Unavailable { timed_out: true },
    );
    c.sync(&x(), w.after(T30)).unwrap();
    assert!(c.status().unwrap().store.outage.is_some());
}

#[test]
fn status_reports_stale_rows_until_a_sync_rereads_them() {
    let w = World::new();
    let mut c = synced(&w);
    let seq = journal(&mut c, &w, "k1", "b2");
    let at = w.clock.now_utc();
    store_mut(&mut c)
        .apply_with_readback(seq, None, Some("BD_CACHE.BD_UNAVAILABLE: timeout"), &at)
        .unwrap();
    assert_eq!(c.status().unwrap().store.stale, vec![id("b2")]);
    c.sync(&x(), w.after(T30)).unwrap();
    assert!(c.status().unwrap().store.stale.is_empty());
}

#[test]
fn announcement_state_passes_through_to_the_store() {
    let w = World::new();
    let mut c = synced(&w);
    let seq = journal(&mut c, &w, "k1", "b1");
    journal(&mut c, &w, "k2", "b2");
    escalate(&mut c, &w, seq, "retries exhausted");
    assert_eq!(
        c.open_work().unwrap(),
        OpenWork {
            pending: 1,
            escalated_unannounced: 1
        }
    );
    let unannounced = c.unannounced_escalations().unwrap();
    assert_eq!(
        unannounced.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![seq]
    );
    w.clock.advance(Duration::from_secs(5));
    c.mark_announced(seq).unwrap();
    assert!(c.unannounced_escalations().unwrap().is_empty());
    assert_eq!(
        c.entry(seq).unwrap().announced_at,
        Some(ts("2026-10-23T10:00:08Z")),
        "announced at the clock's time"
    );
    assert_eq!(c.open_work().unwrap().escalated_unannounced, 0);
    assert_eq!(
        c.mark_announced(Seq::new(99)).unwrap_err().code(),
        "BD_CACHE.NOT_FOUND"
    );

    assert_eq!(c.sync_alert(&x()).unwrap(), None);
    c.set_sync_alert(&x(), Some("backend_unavailable")).unwrap();
    assert_eq!(
        c.sync_alert(&x()).unwrap().as_deref(),
        Some("backend_unavailable")
    );
    c.set_sync_alert(&x(), None).unwrap();
    assert_eq!(c.sync_alert(&x()).unwrap(), None);
}

/// PE-RSH-009: a note over `MAX_NOTE_BYTES` is `BAD_ARGS` and leaves the entry
/// escalated; a note at the limit resolves it.
#[test]
fn a_resolve_note_over_the_limit_is_refused_before_the_store_is_touched() {
    use bd_cache_model::MAX_NOTE_BYTES;
    let w = World::new();
    let mut c = synced(&w);
    let seq = journal(&mut c, &w, "k1", "b1");
    escalate(&mut c, &w, seq, "precondition no longer holds");
    let e = c
        .resolve(seq, &"n".repeat(MAX_NOTE_BYTES + 1), actor("lead"))
        .unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BAD_ARGS", "{e}");
    assert!(e.detail().starts_with("note is 16385 bytes"), "{e}");
    let entry = c.entry(seq).unwrap();
    assert_eq!(entry.state, JournalState::Escalated, "nothing changed");
    assert_eq!(
        entry.last_error.as_deref(),
        Some("precondition no longer holds")
    );
    c.resolve(seq, &"n".repeat(MAX_NOTE_BYTES), actor("lead"))
        .unwrap();
    assert_eq!(c.entry(seq).unwrap().state, JournalState::Resolved);
}
