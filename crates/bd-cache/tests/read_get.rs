//! `get`, `list`, `refresh` and `entry` (REQ-BDC-0001, 0202, 0203, 0902,
//! 0907, 0908; ADR-BDC-0005).
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
use bd_cache_model::{CacheError, CacheStore, JournalState, Scope, ScopeId};
use common::{TestCache, World, id, new_entry, sx, x};
use sc_beads::testing::{FakeCall, FakeOp, Injection};
use sc_beads::{BeadId, Clock};

const T30: Duration = Duration::from_secs(30);

fn synced(w: &World) -> TestCache {
    let mut c = w.cache();
    c.sync(&x(), w.after(T30)).unwrap();
    c
}

fn row_ids(rows: &[bd_cache_model::RowView]) -> Vec<BeadId> {
    rows.iter().map(|r| r.bead.id.clone()).collect()
}

#[test]
fn test_miss_fetches_once_then_serves_from_cache() {
    let w = World::new();
    let mut c = w.cache();
    let first = c.get(&id("b1"), None).unwrap();
    assert_eq!(w.bd.calls(), vec![FakeCall::ListByIds(vec![id("b1")])]);
    assert_eq!(first.row.bead, w.bd_row("b1"));
    assert!(
        w.store.memberships(&id("b1")).is_empty(),
        "a miss is stored with no membership"
    );
    let second = c.get(&id("b1"), None).unwrap();
    assert_eq!(second, first);
    assert_eq!(w.bd.calls().len(), 1, "a cached get starts no bd call");
}

#[test]
fn test_miss_on_unknown_bead_is_typed() {
    let w = World::new();
    let mut c = w.cache();
    let e = c.get(&id("nope"), None).unwrap_err();
    assert!(matches!(e, CacheError::NotFound { .. }), "{e:?}");
    assert_eq!(e.code(), "BD_CACHE.NOT_FOUND");
    assert!(w.store.row_ids().is_empty());
}

#[test]
fn a_miss_is_bounded_by_the_callers_timeout() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.inject(FakeOp::ListByIds, Injection::Delay(Duration::from_secs(5)));
    let t0 = w.clock.elapsed();
    let e = c
        .get(&id("b1"), Some(Duration::from_millis(300)))
        .unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BD_UNAVAILABLE");
    assert_eq!(
        w.clock.elapsed().saturating_sub(t0),
        Duration::from_millis(300)
    );
    assert_eq!(store(&c).outage().unwrap(), None, "a cut is not an outage");
}

#[test]
fn get_and_list_replies_carry_stale_per_row_and_the_scopes_last_ok_at() {
    let w = World::new();
    let start = w.clock.now_utc();
    let mut c = synced(&w);
    let calls = w.bd.calls().len();

    let g = c.get(&id("b2"), None).unwrap();
    assert_eq!(g.scope_last_ok_at, Some(start.clone()));
    assert!(!g.row.stale);
    assert_eq!(g.outage, None);

    let at = w.clock.now_utc();
    let s = store_mut(&mut c);
    let seq = s
        .insert_entry(new_entry("k1", "b2", "close", "{}"), &at)
        .unwrap();
    s.apply_with_readback(seq, None, Some("read-back failed"), &at)
        .unwrap();
    assert!(c.get(&id("b2"), None).unwrap().row.stale);

    let l = c.list(&x(), None, None).unwrap();
    assert_eq!(l.last_ok_at, Some(start));
    assert_eq!(row_ids(&l.rows), vec![id("b1"), id("b2"), id("s1")]);
    let stale: Vec<bool> = l.rows.iter().map(|r| r.stale).collect();
    assert_eq!(stale, vec![false, true, false]);
    assert_eq!(w.bd.calls().len(), calls, "cached reads start no bd call");
}

#[test]
fn replies_carry_the_outage_while_one_holds() {
    let w = World::new();
    let mut c = synced(&w);
    w.bd.inject(
        FakeOp::ListScope,
        Injection::Unavailable { timed_out: false },
    );
    c.sync(&x(), w.after(T30)).unwrap();
    let outage = store(&c).outage().unwrap();
    assert!(outage.is_some());
    assert_eq!(c.get(&id("b1"), None).unwrap().outage, outage);
    assert_eq!(c.list(&x(), None, None).unwrap().outage, outage);
    assert_eq!(c.status().unwrap().store.outage, outage);
}

#[test]
fn get_replays_pending_entries_and_counts_them() {
    let w = World::new();
    let mut c = synced(&w);
    let at = w.clock.now_utc();
    let s = store_mut(&mut c);
    s.insert_entry(
        new_entry("k1", "b1", "update", r#"{"append_notes": "n1"}"#),
        &at,
    )
    .unwrap();
    s.insert_entry(new_entry("k2", "b1", "close", r#"{"reason": "PASS"}"#), &at)
        .unwrap();
    let g = c.get(&id("b1"), None).unwrap();
    assert_eq!(g.pending, 2);
    assert_eq!(g.row.bead.status.as_deref(), Some("closed"));
    assert_eq!(g.row.bead.notes.as_deref(), Some("n1"));
}

#[test]
fn list_filters_by_status_and_label_on_the_view() {
    let w = World::new();
    w.add("b3", |b| b.labels.push("urgent".into()));
    let mut c = synced(&w);
    let open = c.list(&x(), Some("open"), None).unwrap();
    assert_eq!(row_ids(&open.rows), vec![id("b2"), id("b3"), id("s1")]);
    let urgent = c.list(&x(), None, Some("urgent")).unwrap();
    assert_eq!(row_ids(&urgent.rows), vec![id("b3")]);
    let both = c.list(&x(), Some("in_progress"), Some("urgent")).unwrap();
    assert!(both.rows.is_empty());
    // A pending close moves b2 out of `--status open` at once.
    let at = w.clock.now_utc();
    store_mut(&mut c)
        .insert_entry(new_entry("k1", "b2", "close", "{}"), &at)
        .unwrap();
    let open = c.list(&x(), Some("open"), None).unwrap();
    assert_eq!(row_ids(&open.rows), vec![id("b3"), id("s1")]);
    let closed = c.list(&x(), Some("closed"), None).unwrap();
    assert_eq!(row_ids(&closed.rows), vec![id("b2")]);
}

#[test]
fn list_of_a_scope_never_configured_or_synced_is_scope_unknown_naming_the_known_scopes() {
    let w = World::new();
    let c = w.cache();
    let e = c.list(&ScopeId::new("z").unwrap(), None, None).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.SCOPE_UNKNOWN");
    let CacheError::ScopeUnknown { known, .. } = e else {
        panic!("{e:?}");
    };
    assert_eq!(known, vec![x()]);
    // A configured scope that never synced lists (empty), it is not refused.
    let l = c.list(&x(), None, None).unwrap();
    assert!(l.rows.is_empty() && l.last_ok_at.is_none());
}

#[test]
fn a_get_miss_outside_every_scope_is_stored_with_no_membership_and_swept_by_prune_once_it_has_no_open_entry()
 {
    let w = World::new();
    w.bd.insert(sc_beads::testing::bead("o1"));
    let mut c = synced(&w);
    let g = c.get(&id("o1"), None).unwrap();
    assert_eq!((g.scope_last_ok_at, g.row.blocked), (None, None));
    assert!(w.store.memberships(&id("o1")).is_empty());
    assert!(
        c.list(&x(), None, None)
            .unwrap()
            .rows
            .iter()
            .all(|r| r.bead.id != id("o1"))
    );

    let at = w.clock.now_utc();
    let mut entry = new_entry("k1", "o1", "update", r#"{"append_notes": "n"}"#);
    entry.scope = None;
    let seq = store_mut(&mut c).insert_entry(entry, &at).unwrap();
    let r = c.prune(&x()).unwrap();
    assert_eq!(r.out_of_scope_swept, 0, "kept while it has a pending entry");
    assert!(w.store.row_ids().contains(&id("o1")));

    let row = w.bd_row("o1");
    store_mut(&mut c)
        .apply_with_readback(seq, Some(&row), None, &at)
        .unwrap();
    let r = c.prune(&x()).unwrap();
    assert_eq!(r.out_of_scope_swept, 1);
    assert!(!w.store.row_ids().contains(&id("o1")));
}

#[test]
fn refresh_reads_fresh_rows_past_a_stale_cache_and_returns_them_with_the_overlay() {
    let w = World::new();
    let mut c = synced(&w);
    // bd changed b2 (an agent's own `bd update`); the cache still lags it,
    // and even holds a row it believes newer.
    w.clock.advance(Duration::from_secs(60));
    assert!(w.bd.edit("b2", |b| b.title = "fresh".into()));
    let mut newer = w.bd_row("b1");
    newer.updated_at = "2026-10-24T00:00:00Z".into();
    newer.title = "cached".into();
    store_mut(&mut c).upsert_rows(None, &[newer], true).unwrap();
    assert_eq!(c.get(&id("b2"), None).unwrap().row.bead.title, "b2");
    let at = w.clock.now_utc();
    store_mut(&mut c)
        .insert_entry(
            new_entry("k1", "b2", "update", r#"{"assignee": "san"}"#),
            &at,
        )
        .unwrap();

    w.bd.clear_calls();
    let rows = c
        .refresh(
            &[id("b2"), id("nope"), id("b1"), id("b2")],
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(
        w.bd.calls(),
        vec![FakeCall::ListByIds(vec![
            id("b2"),
            id("nope"),
            id("b1"),
            id("b2")
        ])],
        "one bd list --id"
    );
    assert_eq!(
        row_ids(&rows),
        vec![id("b2"), id("b1")],
        "request order, unknown ids omitted"
    );
    assert_eq!(rows[0].bead.title, "fresh");
    assert_eq!(
        rows[0].bead.assignee.as_deref(),
        Some("san"),
        "overlay applied"
    );
    assert_eq!(rows[1].bead.title, "b1", "stored with force");
    assert_eq!(c.get(&id("b2"), None).unwrap().row.bead.title, "fresh");
    assert_eq!(rows[0].blocked, Some(vec![]));
    assert_eq!(
        w.store.memberships(&id("b2")),
        vec![x()],
        "membership unchanged"
    );
}

#[test]
fn refresh_is_bounded_by_its_timeout_and_an_empty_refresh_calls_nothing() {
    let w = World::new();
    let mut c = w.cache();
    assert!(c.refresh(&[], Duration::from_secs(1)).unwrap().is_empty());
    assert!(w.bd.calls().is_empty());
    w.bd.inject(FakeOp::ListByIds, Injection::Delay(Duration::from_secs(5)));
    let t0 = w.clock.elapsed();
    let e = c
        .refresh(&[id("b1")], Duration::from_millis(400))
        .unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BD_UNAVAILABLE");
    assert_eq!(
        w.clock.elapsed().saturating_sub(t0),
        Duration::from_millis(400)
    );
    assert!(w.store.row_ids().is_empty());
    w.bd.inject(FakeOp::ListByIds, Injection::Failed(1));
    let e = c.refresh(&[id("b1")], Duration::from_secs(1)).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BD_LIST_FAILED");
}

#[test]
fn entry_returns_an_entry_in_any_state() {
    let w = World::new();
    let mut c = synced(&w);
    let at = w.clock.now_utc();
    let s = store_mut(&mut c);
    let pending = s
        .insert_entry(new_entry("k1", "b1", "close", "{}"), &at)
        .unwrap();
    let applied = s
        .insert_entry(new_entry("k2", "b2", "close", "{}"), &at)
        .unwrap();
    let escalated = s
        .insert_entry(new_entry("k3", "s1", "close", "{}"), &at)
        .unwrap();
    let resolved = s
        .insert_entry(
            new_entry("k4", "s1", "update", r#"{"status": "open"}"#),
            &at,
        )
        .unwrap();
    s.apply_with_readback(applied, Some(&w.bd_row("b2")), None, &at)
        .unwrap();
    s.escalate(escalated, "precondition", &at).unwrap();
    s.escalate(resolved, "precondition", &at).unwrap();
    s.resolve(resolved, "fixed", &sc_beads::testing::actor("lead"), &at)
        .unwrap();
    for (seq, state) in [
        (pending, JournalState::Pending),
        (applied, JournalState::Applied),
        (escalated, JournalState::Escalated),
        (resolved, JournalState::Resolved),
    ] {
        let e = c.entry(seq).unwrap();
        assert_eq!((e.seq, e.state), (seq, state));
    }
    let e = c.entry(bd_cache_model::Seq::new(99)).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.NOT_FOUND");
}

/// Contract §5.3 `issues get`: `GetReply.scope` is the bead's scope, and
/// `None` for a row outside every scope. A second configured scope is listed
/// first, so "the first configured scope" is never the answer by accident.
#[test]
fn get_reply_scope_is_the_beads_scope_and_none_outside_every_scope() {
    let w = World::new();
    let a = ScopeId::new("a").unwrap();
    let mut config = w.config.clone();
    config.scopes = vec![Scope::label(a.clone()), sx()];
    let mut c = w.cache_with(config);
    c.sync(&x(), w.after(T30)).unwrap();
    w.add("o1", |b| b.labels.clear());
    w.add("a1", |b| b.labels = vec!["a".into()]);

    let scoped = c.get(&id("b1"), None).unwrap();
    assert_eq!(scoped.scope, Some(x()), "b1 is in x");
    assert!(scoped.scope_last_ok_at.is_some());
    let other = c.get(&id("a1"), None).unwrap();
    assert_eq!(other.scope, Some(a), "a1 is in a");
    let outside = c.get(&id("o1"), None).unwrap();
    assert_eq!(outside.scope, None, "o1 is in no scope");
    assert_eq!(outside.row.blocked, None);
    assert_eq!(
        c.cached(&id("o1")).unwrap().unwrap().scope,
        None,
        "cached answers the same"
    );
}
