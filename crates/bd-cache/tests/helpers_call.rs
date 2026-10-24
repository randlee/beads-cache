//! The `call` helper: per-call cap, cut vs outage vs error, and who clears the outage (ADR-BDC-0005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::{call, note_unavailable, store};
use bd_cache_model::{CacheConfig, CacheStore, CallFailure, CallKind};
use common::{World, id};
use sc_beads::testing::{FakeBackend, FakeOp, Injection};
use sc_beads::{BeadsCacheError, CacheBackend, Clock, ProjectRef};

fn outage_of(c: &common::TestCache) -> Option<bd_cache_model::Outage> {
    store(c).outage().unwrap()
}

#[test]
fn call_hands_the_earlier_of_the_hard_deadline_and_the_call_cap() {
    let w = World::new();
    let mut c = w.cache();
    let clock = std::sync::Arc::clone(&w.clock);
    let seen = |c: &mut common::TestCache, hard| {
        let clock = std::sync::Arc::clone(&clock);
        call(c, CallKind::PreRead, hard, move |_, _, d| {
            Ok::<_, BeadsCacheError>(d.remaining(&*clock))
        })
        .unwrap()
    };
    assert_eq!(
        seen(&mut c, w.after(Duration::from_secs(5))),
        Duration::from_secs(5),
        "hard is earlier"
    );
    assert_eq!(
        seen(&mut c, w.after(Duration::from_secs(600))),
        Duration::from_secs(30),
        "the 30 s cap is earlier"
    );
    let mut short = w.cache_with(CacheConfig {
        call_timeout: Duration::from_millis(250),
        ..w.config.clone()
    });
    assert_eq!(
        seen(&mut short, w.after(Duration::from_secs(5))),
        Duration::from_millis(250)
    );
}

#[test]
fn a_timeout_after_the_hard_deadline_is_a_cut_and_records_no_outage() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.set_cost_for(FakeOp::ListByIds, Duration::from_secs(10));
    let hard = w.after(Duration::from_secs(1));
    let r = call(&mut c, CallKind::PreRead, hard, |b, p, d| {
        b.list_by_ids(p, &[id("b1")], d)
    });
    assert_eq!(r.unwrap_err(), CallFailure::Cut);
    assert!(hard.expired(&*w.clock));
    assert_eq!(outage_of(&c), None, "a cut is not an outage");
    // The same cut on a write is not an attempt either: it is a Cut, not an Error.
    w.bd.set_cost_for(FakeOp::Close, Duration::from_secs(10));
    let hard = w.after(Duration::from_secs(1));
    let r = call(&mut c, CallKind::Write, hard, |b, p, d| {
        b.close_guarded(
            p,
            &id("b2"),
            None,
            &sc_beads::Guards::default(),
            &"san".into(),
            d,
        )
    });
    assert_eq!(r.unwrap_err(), CallFailure::Cut);
}

#[test]
fn a_timeout_before_the_hard_deadline_is_an_outage() {
    let w = World::new();
    let mut c = w.cache_with(CacheConfig {
        call_timeout: Duration::from_secs(1),
        ..w.config.clone()
    });
    w.bd.set_cost_for(FakeOp::ListByIds, Duration::from_secs(10));
    let r = call(
        &mut c,
        CallKind::PreRead,
        w.after(Duration::from_secs(60)),
        |b, p, d| b.list_by_ids(p, &[id("b1")], d),
    );
    assert!(
        matches!(r, Err(CallFailure::Outage(ref e)) if e.code() == "BD_CACHE.BD_UNAVAILABLE"),
        "{r:?}"
    );
    assert_eq!(
        outage_of(&c).unwrap().since.as_str(),
        "2026-10-23T10:00:04Z"
    );
}

fn unreachable(w: &World, op: FakeOp) {
    w.bd.inject(op, Injection::Unavailable { timed_out: false });
}

#[test]
fn unavailable_on_a_check_or_list_records_the_outage() {
    for kind in [
        CallKind::Check,
        CallKind::PreRead,
        CallKind::Sync,
        CallKind::Refresh,
        CallKind::MissFill,
        CallKind::Ready,
    ] {
        let w = World::new();
        let mut c = w.cache();
        unreachable(&w, FakeOp::Fingerprint);
        let r = call(&mut c, kind, w.after(Duration::from_secs(5)), |b, p, d| {
            b.fingerprint(p, d)
        });
        assert!(matches!(r, Err(CallFailure::Outage(_))), "{kind:?}: {r:?}");
        let o = outage_of(&c).unwrap_or_else(|| panic!("{kind:?} records the outage"));
        assert!(o.last_error.starts_with("BD_CACHE.BD_UNAVAILABLE"), "{o:?}");
    }
}

#[test]
fn unavailable_on_a_write_is_an_error_that_counts_an_attempt_not_an_outage() {
    for kind in [CallKind::Write, CallKind::ReadBack] {
        let w = World::new();
        let mut c = w.cache();
        unreachable(&w, FakeOp::Close);
        let r = call(&mut c, kind, w.after(Duration::from_secs(5)), |b, p, d| {
            b.close_guarded(
                p,
                &id("b2"),
                None,
                &sc_beads::Guards::default(),
                &"san".into(),
                d,
            )
        });
        match r {
            Err(CallFailure::Error(e)) => {
                assert_eq!(e.code(), "BD_CACHE.BD_UNAVAILABLE", "{kind:?}");
            }
            other => panic!("{kind:?}: {other:?}"),
        }
        assert_eq!(outage_of(&c), None, "{kind:?}");
    }
}

#[test]
fn a_reachable_failure_is_a_typed_error_and_leaves_the_outage_alone() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.inject(FakeOp::ListByIds, Injection::Failed(1));
    let r = call(
        &mut c,
        CallKind::PreRead,
        w.after(Duration::from_secs(5)),
        |b, p, d| b.list_by_ids(p, &[id("b1")], d),
    );
    assert!(
        matches!(r, Err(CallFailure::Error(ref e)) if e.code() == "BD_CACHE.BD_LIST_FAILED"),
        "{r:?}"
    );
    assert_eq!(outage_of(&c), None);
    note_unavailable(&mut c, "BD_CACHE.BD_UNAVAILABLE: earlier").unwrap();
    w.bd.inject(FakeOp::ListByIds, Injection::Failed(1));
    let _ = call(
        &mut c,
        CallKind::PreRead,
        w.after(Duration::from_secs(5)),
        |b, p, d| b.list_by_ids(p, &[id("b1")], d),
    );
    assert!(
        outage_of(&c).is_some(),
        "a failed list does not clear the outage"
    );
}

#[test]
fn only_an_answering_list_clears_the_outage() {
    let w = World::new();
    let mut c = w.cache();
    note_unavailable(&mut c, "BD_CACHE.BD_UNAVAILABLE: refused").unwrap();
    let hard = w.after(Duration::from_secs(5));
    call(&mut c, CallKind::Check, hard, FakeBackend::fingerprint).unwrap();
    assert!(
        outage_of(&c).is_some(),
        "a passing backend check does not clear it"
    );
    call(&mut c, CallKind::Write, hard, |b, p, d| {
        b.close_guarded(
            p,
            &id("b2"),
            None,
            &sc_beads::Guards::default(),
            &"san".into(),
            d,
        )
    })
    .unwrap();
    assert!(outage_of(&c).is_some(), "a write does not clear it");
    for kind in [
        CallKind::PreRead,
        CallKind::ReadBack,
        CallKind::Sync,
        CallKind::Refresh,
        CallKind::MissFill,
        CallKind::Ready,
    ] {
        note_unavailable(&mut c, "BD_CACHE.BD_UNAVAILABLE: refused").unwrap();
        call(&mut c, kind, hard, |b, p, d| {
            b.list_by_ids(p, &[id("b1")], d)
        })
        .unwrap();
        assert_eq!(outage_of(&c), None, "{kind:?} clears it");
    }
}

#[test]
fn every_call_reaches_the_backend_with_the_cache_project() {
    let w = World::new();
    let mut c = w.cache();
    let p = call(
        &mut c,
        CallKind::Ready,
        w.after(Duration::from_secs(1)),
        |_, p, _| Ok::<_, BeadsCacheError>(p.clone()),
    )
    .unwrap();
    assert_eq!(p, ProjectRef::local(None));
    assert_eq!(w.clock.now_utc().as_str(), "2026-10-23T10:00:03Z");
}
