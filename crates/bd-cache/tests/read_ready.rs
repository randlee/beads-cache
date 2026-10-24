//! `ready`: the base adjusted for our journal, one test per rule of the
//! read-path table, and the bounded fresh answer (REQ-BDC-0904, 0911, 0912;
//! ADR-BDC-0002, 0005).
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
use bd_cache_model::{CacheError, CacheStore, ReadyBasis, ReadyReply, ScopeId, Seq};
use common::{TestCache, World, id, new_entry, ts, x};
use sc_beads::testing::{FakeOp, Injection};
use sc_beads::{BeadId, Clock, DepType};

const T30: Duration = Duration::from_secs(30);
const LIMIT: Duration = Duration::from_millis(500);

fn synced(w: &World) -> TestCache {
    let mut c = w.cache();
    c.sync(&x(), w.after(T30)).unwrap();
    c
}

fn ids(r: &ReadyReply) -> Vec<BeadId> {
    r.ready.iter().map(|v| v.bead.id.clone()).collect()
}

fn journal(c: &mut TestCache, w: &World, key: &str, bead: &str, op: &str, args: &str) -> Seq {
    let at = w.clock.now_utc();
    store_mut(c)
        .insert_entry(new_entry(key, bead, op, args), &at)
        .unwrap()
}

/// The `ready` answer and the blocked state of `bead` in it.
fn blocked_of(c: &mut TestCache, bead: &str) -> Option<Vec<BeadId>> {
    c.get(&id(bead), None).unwrap().row.blocked
}

/// Makes every later backend call fail, to show `ready` never calls bd.
fn unreachable(w: &World) {
    for _ in 0..8 {
        w.bd.inject_where(|_| true, Injection::Unavailable { timed_out: false });
    }
}

#[test]
fn a_scope_with_no_ready_base_reports_unknown() {
    let w = World::new();
    let mut c = w.cache();
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(r.basis, ReadyBasis::Unknown);
    assert_eq!(r.last_ok_at, None, "never synced");
    assert!(r.ready.is_empty() && r.missing_rows.is_empty() && !r.locally_adjusted);
    assert!(w.bd.calls().is_empty(), "no limit, no bd call");
    // Rows read before any base carry no blocked state.
    c.get(&id("s1"), None).unwrap();
    assert_eq!(blocked_of(&mut c, "s1"), None);
}

#[test]
fn ready_replies_carry_the_scopes_last_ok_at() {
    let w = World::new();
    let first = w.clock.now_utc();
    let mut c = synced(&w);
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(r.last_ok_at, Some(first.clone()));
    // A failed sync keeps it; a fresh answer does not move it (only a sync does).
    w.clock.advance(Duration::from_secs(60));
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    c.sync(&x(), w.after(T30)).unwrap_err();
    assert_eq!(c.ready(&x(), Some(LIMIT)).unwrap().last_ok_at, Some(first));
    let second = w.clock.now_utc();
    c.sync(&x(), w.after(T30)).unwrap();
    assert_eq!(c.ready(&x(), None).unwrap().last_ok_at, Some(second));
}

#[test]
fn ready_of_an_unknown_scope_is_scope_unknown() {
    let w = World::new();
    let mut c = w.cache();
    let e = c.ready(&ScopeId::phase("nope").unwrap(), None).unwrap_err();
    assert!(
        matches!(&e, CacheError::ScopeUnknown { known, .. } if *known == vec![x()]),
        "{e:?}"
    );
}

#[test]
fn with_bd_unreachable_a_journaled_close_of_the_only_blocker_makes_the_depender_ready_at_once() {
    let w = World::new();
    let mut c = synced(&w);
    let before = c.ready(&x(), None).unwrap();
    assert_eq!(ids(&before), vec![id("b2")]);
    assert!(!before.locally_adjusted);
    unreachable(&w);
    let calls = w.bd.calls().len();

    journal(&mut c, &w, "k1", "b1", "close", r#"{"reason": "PASS"}"#);
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(ids(&r), vec![id("b2"), id("s1")]);
    assert!(r.locally_adjusted);
    assert_eq!(r.read_fallback, None);
    assert_eq!(r.ready[1].blocked, Some(vec![]), "s1 is no longer blocked");
    assert_eq!(blocked_of(&mut c, "s1"), Some(vec![]));

    // A journaled reopen blocks it again.
    journal(&mut c, &w, "k2", "b1", "update", r#"{"status": "open"}"#);
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(ids(&r), vec![id("b2")]);
    assert_eq!(blocked_of(&mut c, "s1"), Some(vec![id("b1")]));
    assert_eq!(w.bd.calls().len(), calls, "no bd call");
    assert_eq!(store(&c).outage().unwrap(), None);
}

#[test]
fn rule_update_to_closed_is_a_close() {
    let w = World::new();
    let mut c = synced(&w);
    journal(&mut c, &w, "k1", "b1", "update", r#"{"status": "closed"}"#);
    assert_eq!(ids(&c.ready(&x(), None).unwrap()), vec![id("b2"), id("s1")]);
}

#[test]
fn rule_close_or_status_change_of_the_bead_itself_leaves_ready() {
    let w = World::new();
    let mut c = synced(&w);
    journal(
        &mut c,
        &w,
        "k1",
        "b2",
        "update",
        r#"{"status": "in_progress"}"#,
    );
    assert!(ids(&c.ready(&x(), None).unwrap()).is_empty());

    let w = World::new();
    let mut c = synced(&w);
    journal(&mut c, &w, "k1", "b2", "close", "{}");
    assert!(ids(&c.ready(&x(), None).unwrap()).is_empty());

    // Other fields, or the status it already has, change nothing.
    let w = World::new();
    let mut c = synced(&w);
    journal(
        &mut c,
        &w,
        "k1",
        "b2",
        "update",
        r#"{"status": "open", "assignee": "dev"}"#,
    );
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(ids(&r), vec![id("b2")]);
    assert!(!r.locally_adjusted);
}

#[test]
fn rule_reopen_from_closed_blocks_the_dependers_again() {
    // bd closed b1 before the base was fetched; our journal reopens it.
    let w = World::new();
    assert!(w.bd.edit("b1", |b| b.status = Some("closed".into())));
    let mut c = synced(&w);
    assert_eq!(ids(&c.ready(&x(), None).unwrap()), vec![id("b2"), id("s1")]);
    journal(&mut c, &w, "k1", "b1", "update", r#"{"status": "open"}"#);
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(ids(&r), vec![id("b2")], "b1 is not ready until bd says so");
    assert_eq!(blocked_of(&mut c, "s1"), Some(vec![id("b1")]));
}

#[test]
fn rule_reopen_brings_back_the_beads_own_blockers() {
    let w = World::new();
    w.add("s2", |b| b.status = Some("closed".into()));
    assert!(w.bd.add_edge("s2", "b1", DepType::Blocks));
    let mut c = synced(&w);
    journal(&mut c, &w, "k1", "s2", "update", r#"{"status": "open"}"#);
    assert_eq!(blocked_of(&mut c, "s2"), Some(vec![id("b1")]));
    assert!(!ids(&c.ready(&x(), None).unwrap()).contains(&id("s2")));
}

#[test]
fn rule_blocking_dep_add_onto_an_open_target_blocks_the_bead() {
    let w = World::new();
    let mut c = synced(&w);
    journal(
        &mut c,
        &w,
        "k1",
        "b2",
        "dep_add",
        r#"{"depends_on": "b1", "type": "blocks"}"#,
    );
    let r = c.ready(&x(), None).unwrap();
    assert!(ids(&r).is_empty());
    assert_eq!(blocked_of(&mut c, "b2"), Some(vec![id("b1")]));
}

#[test]
fn rule_blocking_dep_add_onto_an_uncached_target_blocks_the_bead() {
    let w = World::new();
    let mut c = synced(&w);
    journal(
        &mut c,
        &w,
        "k1",
        "b2",
        "dep_add",
        r#"{"depends_on": "zz-9", "type": "waits-for"}"#,
    );
    assert!(ids(&c.ready(&x(), None).unwrap()).is_empty());
    assert_eq!(blocked_of(&mut c, "b2"), Some(vec![id("zz-9")]));
}

#[test]
fn rule_dep_add_onto_a_closed_target_or_of_a_non_blocking_type_changes_nothing() {
    let w = World::new();
    w.add("d1", |b| b.status = Some("closed".into()));
    let mut c = synced(&w);
    journal(
        &mut c,
        &w,
        "k1",
        "b2",
        "dep_add",
        r#"{"depends_on": "d1", "type": "blocks"}"#,
    );
    journal(
        &mut c,
        &w,
        "k2",
        "b2",
        "dep_add",
        r#"{"depends_on": "b1", "type": "related"}"#,
    );
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(ids(&r), vec![id("b2")]);
    assert!(!r.locally_adjusted);
}

#[test]
fn rule_parent_unblock_releases_the_children() {
    let w = World::new();
    w.add("c1", |_| {});
    assert!(w.bd.add_edge("c1", "s1", DepType::ParentChild));
    let mut c = synced(&w);
    assert_eq!(blocked_of(&mut c, "c1"), Some(vec![id("s1")]));
    journal(&mut c, &w, "k1", "b1", "close", "{}");
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(ids(&r), vec![id("b2"), id("s1"), id("c1")]);
    assert_eq!(blocked_of(&mut c, "c1"), Some(vec![]));
}

#[test]
fn rule_parent_block_blocks_the_children() {
    let w = World::new();
    w.add("p1", |_| {});
    w.add("c2", |_| {});
    assert!(w.bd.add_edge("c2", "p1", DepType::ParentChild));
    let mut c = synced(&w);
    assert!(ids(&c.ready(&x(), None).unwrap()).contains(&id("c2")));
    journal(
        &mut c,
        &w,
        "k1",
        "p1",
        "dep_add",
        r#"{"depends_on": "b1", "type": "blocks"}"#,
    );
    let r = c.ready(&x(), None).unwrap();
    assert!(
        !ids(&r).contains(&id("p1")) && !ids(&r).contains(&id("c2")),
        "{:?}",
        ids(&r)
    );
    assert_eq!(blocked_of(&mut c, "c2"), Some(vec![id("p1")]));
    // A parent-child dep_add onto a blocked parent blocks the new child.
    journal(
        &mut c,
        &w,
        "k2",
        "b2",
        "dep_add",
        r#"{"depends_on": "p1", "type": "parent-child"}"#,
    );
    assert_eq!(blocked_of(&mut c, "b2"), Some(vec![id("p1")]));
}

#[test]
fn a_child_that_also_blocks_on_its_parent_stays_blocked_when_the_parent_is_unblocked() {
    let w = World::new();
    w.add("c1", |_| {});
    assert!(w.bd.add_edge("c1", "s1", DepType::ParentChild));
    assert!(w.bd.add_edge("c1", "s1", DepType::Blocks));
    let mut c = synced(&w);
    journal(&mut c, &w, "k1", "b1", "close", "{}");
    assert_eq!(blocked_of(&mut c, "c1"), Some(vec![id("s1")]), "s1 is open");
}

#[test]
fn a_pending_blocks_dep_add_keeps_a_child_blocked_when_its_parent_is_unblocked() {
    // QA-001: the pending edge is in the child's view but not yet in the
    // store's edge index.
    let w = World::new();
    w.add("c1", |_| {});
    assert!(w.bd.add_edge("c1", "s1", DepType::ParentChild));
    let mut c = synced(&w);
    journal(
        &mut c,
        &w,
        "k1",
        "c1",
        "dep_add",
        r#"{"depends_on": "s1", "type": "blocks"}"#,
    );
    journal(&mut c, &w, "k2", "b1", "close", "{}");
    let r = c.ready(&x(), None).unwrap();
    assert!(!ids(&r).contains(&id("c1")), "{:?}", ids(&r));
    assert!(ids(&r).contains(&id("s1")));
    assert_eq!(blocked_of(&mut c, "c1"), Some(vec![id("s1")]), "s1 is open");
}

#[test]
fn a_pending_parent_child_dep_add_carries_a_later_parent_block_to_the_child() {
    let w = World::new();
    w.add("p1", |_| {});
    w.add("c3", |_| {});
    let mut c = synced(&w);
    journal(
        &mut c,
        &w,
        "k1",
        "c3",
        "dep_add",
        r#"{"depends_on": "p1", "type": "parent-child"}"#,
    );
    assert!(
        ids(&c.ready(&x(), None).unwrap()).contains(&id("c3")),
        "p1 is not blocked yet"
    );
    journal(
        &mut c,
        &w,
        "k2",
        "p1",
        "dep_add",
        r#"{"depends_on": "b1", "type": "blocks"}"#,
    );
    let r = c.ready(&x(), None).unwrap();
    assert!(!ids(&r).contains(&id("c3")), "{:?}", ids(&r));
    assert_eq!(blocked_of(&mut c, "c3"), Some(vec![id("p1")]));
}

#[test]
fn escalated_entries_do_not_adjust_the_base() {
    let w = World::new();
    let mut c = synced(&w);
    let seq = journal(&mut c, &w, "k1", "b1", "close", "{}");
    let at = w.clock.now_utc();
    store_mut(&mut c)
        .escalate(seq, "precondition", &at)
        .unwrap();
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(ids(&r), vec![id("b2")]);
    assert!(!r.locally_adjusted);
}

/// A world whose base was fetched at 10:00:03 and whose close of b1 was
/// applied (read back) at `applied`.
fn applied_close_at(applied: &str) -> (World, TestCache) {
    let w = World::new();
    let mut c = synced(&w);
    assert_eq!(
        store(&c).ready_base(&x()).unwrap().unwrap().fetched_at,
        ts("2026-10-23T10:00:03Z")
    );
    let seq = journal(&mut c, &w, "k1", "b1", "close", "{}");
    let mut row = w.bd_row("b1");
    row.status = Some("closed".into());
    store_mut(&mut c)
        .apply_with_readback(seq, Some(&row), None, &ts(applied))
        .unwrap();
    (w, c)
}

#[test]
fn only_entries_pending_or_applied_at_or_after_fetched_at_adjust_the_base() {
    let (_w, mut c) = applied_close_at("2026-10-23T10:00:02Z");
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(
        ids(&r),
        vec![id("b2")],
        "applied before the fetch: bd's base stands"
    );
    assert!(!r.locally_adjusted);

    let (_w, mut c) = applied_close_at("2026-10-23T10:00:03Z");
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(
        ids(&r),
        vec![id("b2"), id("s1")],
        "applied at the fetch instant"
    );

    let (_w, mut c) = applied_close_at("2026-10-23T10:05:00Z");
    assert_eq!(ids(&c.ready(&x(), None).unwrap()), vec![id("b2"), id("s1")]);
}

#[test]
fn rows_carry_blocked_from_the_adjusted_base_and_a_ready_id_without_a_row_is_only_in_missing_rows()
{
    let w = World::new();
    let mut c = synced(&w);
    let at = w.clock.now_utc();
    store_mut(&mut c)
        .replace_ready_base(&x(), &[], &[id("b2"), id("ghost")], &at)
        .unwrap();
    let r = c.ready(&x(), None).unwrap();
    assert_eq!(ids(&r), vec![id("b2")]);
    assert_eq!(r.missing_rows, vec![id("ghost")]);
    assert_eq!(r.ready[0].blocked, Some(vec![]));
    assert_eq!(
        r.basis,
        ReadyBasis::Cached {
            fetched_at: at,
            stale: false
        }
    );
    // list and get read the same adjusted base.
    let l = c.list(&x(), None, None).unwrap();
    assert!(l.rows.iter().all(|row| row.blocked == Some(vec![])));
    // A bead outside every scope is in no base.
    w.bd.insert(sc_beads::testing::bead("o1"));
    assert_eq!(c.get(&id("o1"), None).unwrap().row.blocked, None);
    // Nor is a listed bead whose view a pending write moves out of the scope.
    journal(
        &mut c,
        &w,
        "k1",
        "b2",
        "update",
        r#"{"remove_labels": ["phase-x"]}"#,
    );
    let l = c.list(&x(), None, None).unwrap();
    let b2 = l.rows.iter().find(|r| r.bead.id == id("b2")).unwrap();
    assert_eq!(b2.blocked, None);
}

#[test]
fn bounded_fresh_ready_returns_bds_answer_and_refreshes_the_base() {
    let w = World::new();
    let mut c = synced(&w);
    let old = store(&c).ready_base(&x()).unwrap().unwrap();
    w.clock.advance(Duration::from_secs(60));
    assert!(w.bd.edit("b1", |b| b.status = Some("closed".into())));
    w.bd.set_cost_for(FakeOp::ListBlocked, Duration::from_millis(50));
    w.bd.set_cost_for(FakeOp::ListReady, Duration::from_millis(50));
    let start = w.clock.now_utc();
    let t0 = w.clock.elapsed();
    let r = c.ready(&x(), Some(LIMIT)).unwrap();
    assert_eq!(
        w.clock.elapsed().saturating_sub(t0),
        Duration::from_millis(100)
    );
    assert_eq!(
        r.basis,
        ReadyBasis::Fresh {
            fetched_at: start.clone()
        },
        "fetched_at is the instant before bd's answer was asked for"
    );
    assert_eq!(r.read_fallback, None);
    assert_eq!(ids(&r), vec![id("b2"), id("s1")]);
    let new = store(&c).ready_base(&x()).unwrap().unwrap();
    assert_eq!(new.fetched_at, start);
    assert_ne!(new.fetched_at, old.fetched_at);
    assert!(new.blocked.is_empty());

    // bd's answer is still adjusted for entries pending.
    journal(&mut c, &w, "k1", "b2", "close", "{}");
    w.clock.advance(Duration::from_secs(5));
    let again = w.clock.now_utc();
    let r = c.ready(&x(), Some(LIMIT)).unwrap();
    assert_eq!(
        (r.basis.clone(), ids(&r)),
        (ReadyBasis::Fresh { fetched_at: again }, vec![id("s1")])
    );
    assert!(r.locally_adjusted);
}

#[test]
fn bounded_fresh_ready_falls_back_to_the_cached_answer_at_the_limit_without_an_outage() {
    let w = World::new();
    let mut c = synced(&w);
    let base = store(&c).ready_base(&x()).unwrap().unwrap();
    journal(&mut c, &w, "k1", "b1", "close", "{}");
    w.bd.inject(
        FakeOp::ListBlocked,
        Injection::Delay(Duration::from_secs(5)),
    );
    let t0 = w.clock.elapsed();
    let r = c.ready(&x(), Some(LIMIT)).unwrap();
    assert_eq!(
        w.clock.elapsed().saturating_sub(t0),
        LIMIT,
        "never past the limit"
    );
    assert_eq!(
        r.basis,
        ReadyBasis::Cached {
            fetched_at: base.fetched_at.clone(),
            stale: false
        }
    );
    assert!(
        r.read_fallback
            .as_deref()
            .is_some_and(|t| t.starts_with("BD_CACHE.BD_UNAVAILABLE: ")
                && t.contains("500ms")
                && t.ends_with("; retry, or run sync")),
        "{:?}",
        r.read_fallback
    );
    assert_eq!(
        ids(&r),
        vec![id("b2"), id("s1")],
        "the cached, adjusted answer"
    );
    assert_eq!(
        store(&c).outage().unwrap(),
        None,
        "the expired limit is no outage"
    );
    assert_eq!(
        store(&c).ready_base(&x()).unwrap(),
        Some(base),
        "the base is untouched"
    );

    // The second call is under the same limit.
    w.bd.inject(FakeOp::ListReady, Injection::Delay(Duration::from_secs(5)));
    w.bd.set_cost_for(FakeOp::ListBlocked, Duration::from_millis(300));
    let t0 = w.clock.elapsed();
    let r = c.ready(&x(), Some(LIMIT)).unwrap();
    assert_eq!(w.clock.elapsed().saturating_sub(t0), LIMIT);
    assert!(r.read_fallback.is_some());
}

#[test]
fn bounded_fresh_ready_on_an_unreachable_backend_falls_back_and_records_the_outage() {
    let w = World::new();
    let mut c = synced(&w);
    w.bd.inject(
        FakeOp::ListBlocked,
        Injection::Unavailable { timed_out: false },
    );
    let r = c.ready(&x(), Some(LIMIT)).unwrap();
    assert!(
        r.read_fallback
            .as_deref()
            .is_some_and(|t| t.starts_with("BD_CACHE.BD_UNAVAILABLE")),
        "{:?}",
        r.read_fallback
    );
    assert!(matches!(r.basis, ReadyBasis::Cached { .. }));
    assert!(store(&c).outage().unwrap().is_some());
}

#[test]
fn no_base_and_no_fresh_answer_is_unknown() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.inject(
        FakeOp::ListBlocked,
        Injection::Delay(Duration::from_secs(5)),
    );
    let r = c.ready(&x(), Some(LIMIT)).unwrap();
    assert_eq!(r.basis, ReadyBasis::Unknown);
    assert!(r.read_fallback.is_some() && r.ready.is_empty());
}
