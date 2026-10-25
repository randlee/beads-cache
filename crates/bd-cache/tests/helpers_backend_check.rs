//! `backend_check::run`: first check records, schema change rebuilds and keeps the journal (REQ-BDC-0105).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::{backend_check, current_view, miss_fill, store, store_mut};
use bd_cache_model::{CacheStore, CallFailure, FingerprintOutcome};
use common::{SCHEMA_TEXT, World, id, new_entry, sx, x};
use sc_beads::BD_SCHEMA_SHA256;
use sc_beads::testing::{FakeOp, Injection};

#[test]
fn test_schema_change_rebuilds_rows_and_keeps_the_journal() {
    let w = World::new();
    let mut c = w.cache();
    let rows = w.bd.rows();
    store_mut(&mut c)
        .upsert_rows(Some(&sx()), &rows, false)
        .unwrap();
    store_mut(&mut c)
        .insert_entry(
            new_entry("k1", "b2", "close", r#"{"reason": "done"}"#),
            &common::ts("2026-10-23T10:00:00Z"),
        )
        .unwrap();
    let hard = w.after(Duration::from_secs(30));
    assert_eq!(
        backend_check(&mut c, hard).unwrap(),
        (FingerprintOutcome::Recorded, true)
    );
    assert_eq!(
        backend_check(&mut c, hard).unwrap(),
        (FingerprintOutcome::Unchanged, true),
        "not rebuilt"
    );
    assert_eq!(store(&c).rows_in_scope(&x()).unwrap().len(), 3);
    w.bd.set_schema(SCHEMA_TEXT.replace("\"schema_version\": 1", "\"schema_version\": 1 "));
    assert_eq!(
        backend_check(&mut c, hard).unwrap(),
        (FingerprintOutcome::Rebuilt, false)
    );
    assert!(
        store(&c).rows_in_scope(&x()).unwrap().is_empty(),
        "rows dropped"
    );
    assert!(store(&c).get_row(&id("b2")).unwrap().is_none());
    let pending = store(&c).pending_entries().unwrap();
    assert_eq!(pending.len(), 1, "the journal is kept");
    assert_eq!(pending[0].bead, id("b2"));
    // The next fill brings the row back and the pending close replays on it.
    miss_fill(&mut c, &id("b2"), None).unwrap();
    let (view, _) = current_view(&c, &id("b2")).unwrap().unwrap();
    assert_eq!(view.status.as_deref(), Some("closed"));
}

#[test]
fn test_model_current_when_live_schema_matches_pin() {
    let w = World::new();
    let mut c = w.cache();
    let (_, model_current) = backend_check(&mut c, w.after(Duration::from_secs(30))).unwrap();
    assert!(model_current);
}

#[test]
fn test_first_backend_check_records_schema_without_dropping_cached_rows() {
    let w = World::new();
    let mut c = w.cache();
    miss_fill(&mut c, &id("b1"), None).unwrap();
    assert!(
        !store(&c)
            .status()
            .unwrap()
            .meta
            .contains_key("bd_schema_sha256")
    );
    let (outcome, _) = backend_check(&mut c, w.after(Duration::from_secs(30))).unwrap();
    assert_eq!(
        outcome,
        FingerprintOutcome::Recorded,
        "rebuilt is false on the first check"
    );
    assert!(
        store(&c).get_row(&id("b1")).unwrap().is_some(),
        "the cached row survives"
    );
    assert_eq!(
        store(&c).status().unwrap().meta["bd_schema_sha256"],
        BD_SCHEMA_SHA256
    );
    assert_eq!(store(&c).status().unwrap().meta["bd_version"], "1.3.1");
}

#[test]
fn a_failed_or_cut_check_records_nothing_in_meta() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.inject(FakeOp::Fingerprint, Injection::Failed(1));
    let r = backend_check(&mut c, w.after(Duration::from_secs(30)));
    assert!(
        matches!(r, Err(CallFailure::Error(ref e)) if e.code() == "BD_CACHE.BD_VERSION_FAILED"),
        "{r:?}"
    );
    w.bd.set_cost_for(FakeOp::Fingerprint, Duration::from_secs(5));
    assert_eq!(
        backend_check(&mut c, w.after(Duration::from_secs(1))),
        Err(CallFailure::Cut)
    );
    assert!(
        !store(&c)
            .status()
            .unwrap()
            .meta
            .contains_key("bd_schema_sha256")
    );
    assert_eq!(store(&c).outage().unwrap(), None);
}
