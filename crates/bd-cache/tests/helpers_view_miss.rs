//! `view::current_view` replays pending entries; `miss::fill` is bounded (REQ-BDC-0202; ADR-BDC-0005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::{current_view, miss_fill, store, store_mut};
use bd_cache_model::{CacheConfig, CacheStore, Freshness};
use common::{World, id, new_entry, x};
use sc_beads::testing::{FakeOp, Injection};

const AT: &str = "2026-10-23T10:00:00Z";

#[test]
fn current_view_replays_pending_entries_and_leaves_the_row_untouched() {
    let w = World::new();
    let mut c = w.cache();
    assert_eq!(
        current_view(&c, &id("b1")).unwrap(),
        None,
        "not cached: no view, no bd call"
    );
    assert!(w.bd.calls().is_empty());
    miss_fill(&mut c, &id("b1"), None).unwrap();
    let s = store_mut(&mut c);
    s.insert_entry(
        new_entry("k1", "b1", "update", r#"{"append_notes": "one"}"#),
        AT,
    )
    .unwrap();
    let esc = s
        .insert_entry(
            new_entry("k2", "b1", "update", r#"{"assignee": "lead"}"#),
            AT,
        )
        .unwrap();
    s.insert_entry(new_entry("k3", "b1", "close", r#"{"reason": "PASS"}"#), AT)
        .unwrap();
    s.escalate(esc, "precondition no longer holds", AT).unwrap();
    let (view, freshness) = current_view(&c, &id("b1")).unwrap().unwrap();
    assert_eq!(view.status.as_deref(), Some("closed"));
    assert_eq!(view.close_reason.as_deref(), Some("PASS"));
    assert_eq!(view.notes.as_deref(), Some("one"));
    assert_eq!(
        view.assignee.as_deref(),
        Some("dev"),
        "escalated entries are not replayed"
    );
    assert_eq!(freshness, Freshness::Fresh);
    let (row, _) = store(&c).get_row(&id("b1")).unwrap().unwrap();
    assert_eq!(
        row.status.as_deref(),
        Some("in_progress"),
        "the stored row is untouched"
    );
    assert_eq!(w.bd.calls().len(), 1, "views start no bd call");
}

#[test]
fn current_view_reports_stale_rows() {
    let w = World::new();
    let mut c = w.cache();
    miss_fill(&mut c, &id("b2"), None).unwrap();
    let seq = store_mut(&mut c)
        .insert_entry(new_entry("k1", "b2", "close", "{}"), AT)
        .unwrap();
    store_mut(&mut c)
        .apply_with_readback(seq, None, Some("read-back failed"), AT)
        .unwrap();
    assert_eq!(
        current_view(&c, &id("b2")).unwrap().unwrap().1,
        Freshness::Stale
    );
}

#[test]
fn miss_fill_stores_the_row_with_no_scope() {
    let w = World::new();
    let mut c = w.cache();
    let row = miss_fill(&mut c, &id("b1"), None).unwrap().unwrap();
    assert_eq!(row.id, id("b1"));
    assert_eq!(row.dependency_count, None, "the stored (normalized) form");
    assert!(store(&c).get_row(&id("b1")).unwrap().is_some());
    assert!(
        store(&c).rows_in_scope(&x()).unwrap().is_empty(),
        "stored with None: no membership"
    );
    assert_eq!(w.bd.reads().len(), 1);
    assert_eq!(
        miss_fill(&mut c, &id("nope"), None).unwrap(),
        None,
        "unknown in bd"
    );
}

#[test]
fn miss_fill_stops_at_the_miss_fill_timeout_with_bd_unavailable() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.set_cost_for(FakeOp::ListByIds, Duration::from_secs(5));
    let start = w.clock.elapsed();
    let e = miss_fill(&mut c, &id("b1"), None).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BD_UNAVAILABLE");
    assert_eq!(
        w.clock.elapsed().saturating_sub(start),
        Duration::from_secs(2),
        "the default 2 s"
    );
    assert_eq!(
        store(&c).outage().unwrap(),
        None,
        "a cut fill records no outage"
    );
    let start = w.clock.elapsed();
    assert_eq!(
        miss_fill(&mut c, &id("b1"), Some(Duration::from_millis(300)))
            .unwrap_err()
            .code(),
        "BD_CACHE.BD_UNAVAILABLE"
    );
    assert_eq!(
        w.clock.elapsed().saturating_sub(start),
        Duration::from_millis(300),
        "the caller's timeout"
    );
    let mut short = w.cache_with(CacheConfig {
        miss_fill_timeout: Duration::from_millis(700),
        ..w.config.clone()
    });
    let start = w.clock.elapsed();
    assert!(miss_fill(&mut short, &id("b1"), None).is_err());
    assert_eq!(
        w.clock.elapsed().saturating_sub(start),
        Duration::from_millis(700),
        "the configured timeout"
    );
    assert!(
        store(&c).get_row(&id("b1")).unwrap().is_none(),
        "nothing stored"
    );
}

#[test]
fn miss_fill_on_an_unreachable_backend_is_bd_unavailable_and_an_outage() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.inject(
        FakeOp::ListByIds,
        Injection::Unavailable { timed_out: false },
    );
    assert_eq!(
        miss_fill(&mut c, &id("b1"), None).unwrap_err().code(),
        "BD_CACHE.BD_UNAVAILABLE"
    );
    assert!(store(&c).outage().unwrap().is_some());
    miss_fill(&mut c, &id("b1"), None).unwrap();
    assert!(
        store(&c).outage().unwrap().is_none(),
        "the answering fill clears it"
    );
}

#[test]
fn a_failed_miss_fill_is_bd_list_failed_and_stores_nothing() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.inject(FakeOp::ListByIds, Injection::Failed(1));
    assert_eq!(
        miss_fill(&mut c, &id("b1"), None).unwrap_err().code(),
        "BD_CACHE.BD_LIST_FAILED"
    );
    assert!(store(&c).get_row(&id("b1")).unwrap().is_none());
}
