//! `Cache::pending_entries` and `Cache::backend_check` (lead ruling S66,
//! a one-time ownership exception for sprint b-2).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::store_mut;
use bd_cache_model::{CacheStore, FingerprintOutcome, JournalState};
use common::{TestCache, World, id, new_entry, ts, x};
use sc_beads::Guards;
use sc_beads::testing::{FakeOp, Injection, actor};

const T30: Duration = Duration::from_secs(30);

fn synced(w: &World) -> TestCache {
    let mut c = w.cache();
    c.sync(&x(), w.after(T30)).unwrap();
    c
}

#[test]
fn pending_entries_lists_only_the_pending_ones_by_seq() {
    let w = World::new();
    let mut c = synced(&w);
    let key = |k: &str| bd_cache_model::IdemKey::new(k).unwrap();
    let a = c
        .write(
            &id("b2"),
            "update",
            r#"{"add_labels": ["a"]}"#,
            Guards::default(),
            key("k1"),
            actor("san"),
        )
        .unwrap();
    let b = c
        .write(
            &id("s1"),
            "close",
            "{}",
            Guards::default(),
            key("k2"),
            actor("san"),
        )
        .unwrap();
    let esc = store_mut(&mut c)
        .insert_entry(new_entry("k3", "b1", "close", "{}"), &ts(common::START))
        .unwrap();
    store_mut(&mut c)
        .escalate(esc, "BD_CACHE.PRECONDITION: no", &ts(common::START))
        .unwrap();
    let pending = c.pending_entries().unwrap();
    let seqs: Vec<_> = pending.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, [a.seq, b.seq]);
    assert!(pending.iter().all(|e| e.state == JournalState::Pending));
}

#[test]
fn backend_check_fingerprints_and_applies_the_rebuild_rule() {
    let w = World::new();
    let mut c = synced(&w);
    w.bd.clear_calls();
    let (outcome, current) = c.backend_check(w.after(T30)).unwrap();
    assert_eq!((outcome, current), (FingerprintOutcome::Unchanged, true));
    assert_eq!(w.bd.calls().len(), 1, "one fingerprint call");

    w.bd.set_schema("{\"schema_version\": 2}");
    let (outcome, current) = c.backend_check(w.after(T30)).unwrap();
    assert_eq!((outcome, current), (FingerprintOutcome::Rebuilt, false));
    assert!(
        c.cached(&id("b1")).unwrap().is_none(),
        "a rebuild drops the rows"
    );
}

#[test]
fn an_unreachable_backend_is_bd_unavailable_and_an_outage() {
    let w = World::new();
    let mut c = synced(&w);
    w.bd.inject(
        FakeOp::Fingerprint,
        Injection::Unavailable { timed_out: false },
    );
    let e = c.backend_check(w.after(T30)).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BD_UNAVAILABLE");
    assert!(c.status().unwrap().store.outage.is_some());
}
