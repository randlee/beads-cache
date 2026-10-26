//! Failure state and compare-and-set marking (REQ-BDC-0728), and the outage
//! recorded per scope with reads still answering (REQ-BDC-0729, REQ-BDC-0001; AC2, AC5).
//!
//! One test per `failing_since` transition: set on the first non-ok result after an
//! ok, kept across consecutive non-ok results (a change between a failure and an
//! outage included), cleared on ok. `scope_announce` marks only the current failure;
//! `first_offered_at` is set once.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache_model::{CacheError, FailingSince};
use common::{TestCache, World, id, x};
use sc_beads::Clock;
use sc_beads::testing::{FakeOp, Injection};

const T30: Duration = Duration::from_secs(30);

fn synced() -> (World, TestCache) {
    let w = World::new();
    let mut c = w.cache();
    c.sync(&x(), w.after(T30)).unwrap();
    (w, c)
}

fn failing_since(c: &TestCache) -> Option<FailingSince> {
    c.sync_state(&x()).unwrap().failing_since
}

fn fail(w: &World, c: &mut TestCache) {
    w.clock.advance(T30);
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    let e = c.sync(&x(), w.after(T30)).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.SYNC_FAILED");
}

fn outage(w: &World, c: &mut TestCache) {
    w.clock.advance(T30);
    w.bd.inject(
        FakeOp::ListScope,
        Injection::Unavailable { timed_out: false },
    );
    let r = c.sync(&x(), w.after(T30)).unwrap();
    assert!(r.outage.is_some());
}

fn pass(w: &World, c: &mut TestCache) {
    w.clock.advance(T30);
    c.sync(&x(), w.after(T30)).unwrap();
}

#[test]
fn failing_since_is_set_on_the_first_non_ok_result_after_an_ok() {
    let (w, mut c) = synced();
    assert_eq!(failing_since(&c), None);
    let at = w.clock.now_utc();
    fail(&w, &mut c);
    let since = failing_since(&c).expect("set");
    assert!(
        since.as_str() >= at.as_str(),
        "the time of the first non-ok result"
    );
}

#[test]
fn failing_since_is_kept_across_consecutive_failures_and_outages() {
    let (w, mut c) = synced();
    fail(&w, &mut c);
    let since = failing_since(&c);
    fail(&w, &mut c);
    assert_eq!(failing_since(&c), since, "kept across failures");
    outage(&w, &mut c);
    assert_eq!(
        failing_since(&c),
        since,
        "kept across a change to an outage"
    );
    fail(&w, &mut c);
    assert_eq!(failing_since(&c), since, "and back");
}

#[test]
fn failing_since_is_cleared_on_ok() {
    let (w, mut c) = synced();
    outage(&w, &mut c);
    assert!(failing_since(&c).is_some());
    pass(&w, &mut c);
    assert_eq!(failing_since(&c), None);
    // A later failure starts a new failure.
    fail(&w, &mut c);
    assert!(failing_since(&c).is_some());
}

#[test]
fn scope_announce_marks_only_the_current_failure() {
    let (w, mut c) = synced();
    fail(&w, &mut c);
    let first = failing_since(&c).unwrap();
    c.scope_announce(&x(), &first).unwrap();
    assert!(c.sync_state(&x()).unwrap().notified());
    pass(&w, &mut c);
    fail(&w, &mut c);
    // A mark meant for the old failure never silences the new one.
    let stale = c.scope_announce(&x(), &first).unwrap_err();
    assert!(
        matches!(stale, CacheError::Precondition { .. }),
        "{stale:?}"
    );
    assert!(!c.sync_state(&x()).unwrap().notified());
    let current = failing_since(&c).unwrap();
    assert_ne!(current, first);
    c.scope_announce(&x(), &current).unwrap();
    assert!(c.sync_state(&x()).unwrap().notified());
}

#[test]
fn first_offered_at_is_set_once_through_the_core() {
    let (w, mut c) = synced();
    let seq = c
        .write(
            &id("b2"),
            "close",
            "{}",
            sc_beads::Guards::default(),
            bd_cache_model::IdemKey::new("k1").unwrap(),
            sc_beads::testing::actor("san"),
        )
        .unwrap()
        .seq;
    c.mark_offered(&[seq]).unwrap();
    let first = c.entry(seq).unwrap().first_offered_at.expect("set");
    w.clock.advance(T30);
    c.mark_offered(&[seq]).unwrap();
    assert_eq!(c.entry(seq).unwrap().first_offered_at, Some(first));
}

/// An outage is recorded for the affected scope (result, error, `failing_since`),
/// and a cached read of one of its beads still answers from the store with no bd
/// process started (REQ-BDC-0001).
#[test]
fn during_an_outage_a_cached_read_answers_from_the_store_with_no_bd_call() {
    let (w, mut c) = synced();
    outage(&w, &mut c);
    let st = c.sync_state(&x()).unwrap();
    assert!(st.failing_since.is_some());
    assert!(
        st.last_error
            .as_deref()
            .is_some_and(|e| e.starts_with("BD_CACHE.BD_UNAVAILABLE")),
        "{st:?}"
    );
    let calls = w.bd.calls().len();
    let got = c.get(&id("b1"), None).unwrap();
    assert_eq!(got.row.bead.id, id("b1"));
    assert!(got.outage.is_some(), "the read reports the outage");
    let listed = c.list(&x(), None, None).unwrap();
    assert_eq!(listed.rows.len(), 3);
    let ready = c.ready(&x(), None).unwrap();
    assert!(ready.read_fallback.is_none());
    assert_eq!(w.bd.calls().len(), calls, "no bd process for cached reads");
}
