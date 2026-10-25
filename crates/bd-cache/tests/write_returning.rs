//! `write_returning`, `entry_by_key` and `journal_counts` (REQ-BDC-0809,
//! 0004, 0908; contract §5.1, §5.3, §6; sprint b-1).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "test code: a failed unwrap or index is a test failure; w, c name the world and cache"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::{store, store_mut};
use bd_cache_model::{
    CacheError, CacheStore, IdemKey, JournalCounts, JournalState, NewEntry, StateCounts,
    WriteOutcome,
};
use common::{TestCache, World, id, new_entry, x};
use sc_beads::testing::actor;
use sc_beads::{Clock, Guards};

const T30: Duration = Duration::from_secs(30);

fn synced(w: &World) -> TestCache {
    let mut c = w.cache();
    c.sync(&x(), w.after(T30)).unwrap();
    c
}

fn key(k: &str) -> IdemKey {
    IdemKey::new(k).unwrap()
}

fn status_is(s: &str) -> Guards {
    Guards {
        status: Some(s.into()),
        assignee: None,
    }
}

fn returning(
    c: &mut TestCache,
    bead: &str,
    op: &str,
    args: &str,
    pre: Guards,
    k: &str,
    who: &str,
) -> WriteOutcome {
    c.write_returning(&id(bead), op, args, pre, key(k), actor(who))
        .unwrap_or_else(|e| panic!("{op} {args} on {bead}: {e}"))
}

/// REQ-BDC-0809: one call answers the entry as stored, `before` as a `get`
/// just before the write, and `after` as a `get` right after it, with no bd
/// call for a cached bead.
#[test]
fn write_returning_answers_the_stored_entry_and_both_views() {
    let w = World::new();
    let mut c = synced(&w);
    let before_get = c.get(&id("b2"), None).unwrap().row;
    w.bd.clear_calls();

    let out = returning(
        &mut c,
        "b2",
        "update",
        r#"{"status": "in_progress"}"#,
        Guards::default(),
        "k1",
        "san",
    );

    assert!(w.bd.calls().is_empty(), "a cached bead needs no bd call");
    assert!(!out.duplicate);
    assert_eq!(
        out.changed,
        Some(true),
        "open -> in_progress changes the view"
    );
    assert_eq!(out.entry.changed, Some(true), "changed is stored");
    assert_eq!(out.entry.state, JournalState::Pending);
    assert_eq!(out.entry.scope, Some(x()));
    assert_eq!(
        out.entry,
        c.entry_by_key(&key("k1")).unwrap(),
        "the entry as stored"
    );
    assert_eq!(out.before, before_get, "before is the row before the write");
    assert_eq!(out.before.bead.status.as_deref(), Some("open"));
    let after_get = c.get(&id("b2"), None).unwrap().row;
    assert_eq!(out.after, after_get, "after is a subsequent get's row");
    assert_eq!(out.after.bead.status.as_deref(), Some("in_progress"));
    assert_eq!(
        out.after.blocked,
        Some(vec![]),
        "the blocked state is read as get reads it"
    );
}

/// Contract §5.3: a close stores `!already_closed`, where `already_closed`
/// describes the view before the write; other ops store whether the overlay
/// changes the view.
#[test]
fn changed_is_not_already_closed_for_a_close_and_the_overlay_otherwise() {
    let w = World::new();
    let mut c = synced(&w);
    let first = returning(&mut c, "b1", "close", "{}", Guards::default(), "k1", "san");
    assert_eq!(first.changed, Some(true), "b1 was in progress");
    assert_eq!(first.before.bead.status.as_deref(), Some("in_progress"));
    assert_eq!(first.after.bead.status.as_deref(), Some("closed"));

    let again = returning(
        &mut c,
        "b1",
        "close",
        r#"{"reason": "again"}"#,
        Guards::default(),
        "k2",
        "san",
    );
    assert!(!again.duplicate, "another key: a new entry");
    assert_eq!(
        again.changed,
        Some(false),
        "already closed in the view before the write"
    );
    assert_eq!(again.entry.changed, Some(false));
    assert_eq!(again.before.bead.status.as_deref(), Some("closed"));

    let same = returning(
        &mut c,
        "b2",
        "update",
        r#"{"assignee": ""}"#,
        Guards::default(),
        "k3",
        "san",
    );
    assert_eq!(same.changed, Some(false), "b2 is already unassigned");
    assert_eq!(same.after, same.before);

    let edge = returning(
        &mut c,
        "b2",
        "dep_add",
        r#"{"depends_on": "s1", "type": "blocks"}"#,
        Guards::default(),
        "k4",
        "san",
    );
    assert_eq!(edge.changed, Some(true), "a new edge");
    assert_eq!(edge.after.bead.dependencies.len(), 1);
    let twice = returning(
        &mut c,
        "b2",
        "dep_add",
        r#"{"depends_on": "s1", "type": "blocks"}"#,
        Guards::default(),
        "k5",
        "san",
    );
    assert_eq!(
        twice.changed,
        Some(false),
        "the edge is already in the view"
    );
}

/// REQ-BDC-0004, contract §5.1: a retry with the same key, another actor and
/// another guard is a duplicate. It journals nothing and answers the original
/// seq and the stored `changed`: recomputing it would give `false`, because
/// the original entry's overlay is already in the view.
#[test]
fn a_retry_with_another_actor_and_guard_returns_the_stored_changed() {
    let w = World::new();
    let mut c = synced(&w);
    let first = returning(
        &mut c,
        "b2",
        "update",
        r#"{"status": "in_progress"}"#,
        status_is("open"),
        "k1",
        "san",
    );
    assert!(first.changed == Some(true) && !first.duplicate);

    // The guard no longer holds (the view is in progress) and the actor differs.
    let retry = returning(
        &mut c,
        "b2",
        "update",
        r#"{"status": "in_progress"}"#,
        status_is("closed"),
        "k1",
        "lead",
    );
    assert!(retry.duplicate);
    assert_eq!(retry.entry.seq, first.entry.seq, "the original seq");
    assert_eq!(
        retry.changed,
        Some(true),
        "the original changed, not a recomputed false"
    );
    assert_eq!(retry.entry, first.entry, "the stored entry, unchanged");
    assert_eq!(retry.entry.actor, actor("san"), "the original actor");
    assert_eq!(retry.before, retry.after, "both the current view");
    assert_eq!(retry.after, c.get(&id("b2"), None).unwrap().row);
    assert_eq!(w.store.journal().len(), 1, "nothing more journaled");

    // The 0.1 verb over the same journaling answers the same duplicate.
    let old = c
        .write(
            &id("b2"),
            "update",
            r#"{"status": "in_progress"}"#,
            status_is("blocked"),
            key("k1"),
            actor("other"),
        )
        .unwrap();
    assert!(old.duplicate);
    assert_eq!(old.seq, first.entry.seq);

    // Another op under the key is still a conflict.
    let e = c
        .write_returning(
            &id("b2"),
            "close",
            "{}",
            Guards::default(),
            key("k1"),
            actor("san"),
        )
        .unwrap_err();
    assert!(matches!(e, CacheError::IdemConflict { .. }), "{e:?}");
}

/// An entry journaled without `changed` (as `bd_cache.py` writes it) answers
/// on a duplicate the prediction against the view that write saw.
#[test]
fn a_duplicate_of_an_entry_without_changed_predicts_it() {
    let w = World::new();
    let mut c = synced(&w);
    let at = w.clock.now_utc();
    let seq = store_mut(&mut c)
        .insert_entry(
            NewEntry {
                changed: None,
                ..new_entry("py-1", "b2", "update", r#"{"status": "in_progress"}"#)
            },
            &at,
        )
        .unwrap();
    let dup = returning(
        &mut c,
        "b2",
        "update",
        r#"{"status": "in_progress"}"#,
        Guards::default(),
        "py-1",
        "san",
    );
    assert!(dup.duplicate);
    assert_eq!(dup.entry.seq, seq);
    assert_eq!(dup.entry.changed, None, "nothing is written on a duplicate");
    assert_eq!(dup.changed, Some(true), "b2 was open before that write");
}

/// REQ-BDC-0908: `entry_by_key` answers the entry in every state; an unknown
/// key is `NOT_FOUND`, never a default entry.
#[test]
fn entry_by_key_answers_every_state_and_not_found_for_an_unknown_key() {
    let w = World::new();
    let mut c = synced(&w);
    let at = w.clock.now_utc();
    let mut journal = |k: &str, bead: &str| {
        store_mut(&mut c)
            .insert_entry(new_entry(k, bead, "close", "{}"), &at)
            .unwrap()
    };
    let pending = journal("k-pending", "b1");
    let applied = journal("k-applied", "b2");
    let escalated = journal("k-escalated", "s1");
    let resolved = journal("k-resolved", "s1");
    let s = store_mut(&mut c);
    s.apply_with_readback(applied, None, None, &at).unwrap();
    s.escalate(escalated, "BD_CACHE.PRECONDITION: no", &at)
        .unwrap();
    s.escalate(resolved, "BD_CACHE.PRECONDITION: no", &at)
        .unwrap();
    c.resolve(resolved, "set by hand", actor("lead")).unwrap();

    for (k, seq, state) in [
        ("k-pending", pending, JournalState::Pending),
        ("k-applied", applied, JournalState::Applied),
        ("k-escalated", escalated, JournalState::Escalated),
        ("k-resolved", resolved, JournalState::Resolved),
    ] {
        let e = c.entry_by_key(&key(k)).unwrap();
        assert_eq!((e.seq, e.state), (seq, state), "{k}");
        assert_eq!(e, c.entry(seq).unwrap(), "{k}: the same entry as by seq");
    }
    let e = c.entry_by_key(&key("k-unknown")).unwrap_err();
    assert!(matches!(e, CacheError::NotFound { .. }), "{e:?}");
    assert_eq!(e.code(), "BD_CACHE.NOT_FOUND");
    assert!(e.detail().contains("k-unknown"), "{}", e.detail());
}

/// Contract §6 `journal`: the facade's counts over a journal in four states,
/// two scopes and unscoped entries equal a hand count, and `total` is the sum
/// of the buckets.
#[test]
fn journal_counts_equal_a_hand_count_with_unscoped_entries() {
    let w = World::new();
    let mut c = synced(&w);
    let y = bd_cache_model::ScopeId::new("y").unwrap();
    let at = w.clock.now_utc();
    let mut n = 0;
    let mut journal = |c: &mut TestCache, scope: Option<bd_cache_model::ScopeId>| {
        n += 1;
        store_mut(c)
            .insert_entry(
                NewEntry {
                    scope,
                    ..new_entry(&format!("k{n}"), "b1", "close", "{}")
                },
                &at,
            )
            .unwrap()
    };
    let x1 = journal(&mut c, Some(x()));
    journal(&mut c, Some(x()));
    let x3 = journal(&mut c, Some(x()));
    let y1 = journal(&mut c, Some(y.clone()));
    let u1 = journal(&mut c, None);
    journal(&mut c, None);
    store_mut(&mut c)
        .apply_with_readback(x1, None, None, &at)
        .unwrap();
    store_mut(&mut c).escalate(x3, "why", &at).unwrap();
    store_mut(&mut c).escalate(y1, "why", &at).unwrap();
    c.resolve(y1, "by hand", actor("lead")).unwrap();
    store_mut(&mut c).escalate(u1, "why", &at).unwrap();

    let counts = c.journal_counts().unwrap();
    let sc = |pending, applied, escalated, resolved| StateCounts {
        pending,
        applied,
        escalated,
        resolved,
    };
    let mut expected = JournalCounts {
        total: sc(2, 1, 2, 1),
        unscoped: sc(1, 0, 1, 0),
        ..JournalCounts::default()
    };
    expected.by_scope.insert(x(), sc(1, 1, 1, 0));
    expected.by_scope.insert(y, sc(0, 0, 0, 1));
    assert_eq!(counts, expected, "the hand count");
    let buckets: u64 =
        counts.by_scope.values().map(StateCounts::sum).sum::<u64>() + counts.unscoped.sum();
    assert_eq!(
        counts.total.sum(),
        buckets,
        "total is the sum of the buckets"
    );
    assert_eq!(
        counts.total.sum(),
        store(&c).status().unwrap().journal.values().sum::<u64>(),
        "every entry counted once"
    );
}

/// Lead ruling S68 (RCR-001): an applied entry stored without `changed` (as
/// `bd_cache.py` or a pre-b-1 store wrote it) answers `None` on a duplicate.
/// Its effect is already in the stored row (the applied close left it
/// closed), so a prediction would say `false` where the write changed the
/// view: unknown, never a guess.
#[test]
fn a_duplicate_of_an_applied_entry_without_changed_is_unknown() {
    let w = World::new();
    let mut c = synced(&w);
    let at = w.clock.now_utc();
    let seq = store_mut(&mut c)
        .insert_entry(
            NewEntry {
                changed: None,
                ..new_entry("py-close", "b2", "close", "{}")
            },
            &at,
        )
        .unwrap();
    let mut closed = w.bd_row("b2");
    closed.status = Some("closed".into());
    store_mut(&mut c)
        .apply_with_readback(seq, Some(&closed), None, &at)
        .unwrap();
    let dup = returning(
        &mut c,
        "b2",
        "close",
        "{}",
        Guards::default(),
        "py-close",
        "san",
    );
    assert!(dup.duplicate);
    assert_eq!(dup.entry.seq, seq);
    assert_eq!(dup.entry.state, JournalState::Applied);
    assert_eq!(
        dup.changed, None,
        "unknown: the stored row already carries the close"
    );
    assert_eq!(dup.after.bead.status.as_deref(), Some("closed"));
}
