//! The outage record keeps its start and the latest error (ADR-BDC-0005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::{backend_check, note_ok, store};
use bd_cache_model::{CacheStore, CallFailure};
use common::World;
use sc_beads::testing::{FakeOp, Injection};

#[test]
fn test_outage_keeps_its_start_time_and_latest_error() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.inject(
        FakeOp::Fingerprint,
        Injection::Unavailable { timed_out: false },
    );
    let first = backend_check(&mut c, w.after(Duration::from_secs(30)));
    assert!(matches!(first, Err(CallFailure::Outage(_))), "{first:?}");
    let since = store(&c).outage().unwrap().unwrap().since;
    w.clock.advance(Duration::from_secs(60));
    w.bd.inject(
        FakeOp::Fingerprint,
        Injection::Unavailable { timed_out: true },
    );
    let _ = backend_check(&mut c, w.after(Duration::from_secs(30)));
    let o = store(&c).outage().unwrap().unwrap();
    assert_eq!(o.since, since, "the start is kept");
    assert!(
        o.last_error.contains("timed out"),
        "the latest error is stored: {o:?}"
    );
    assert_eq!(
        since.as_str(),
        "2026-10-23T10:00:03Z",
        "stamped at the first failure"
    );
    note_ok(&mut c).unwrap();
    assert_eq!(store(&c).outage().unwrap(), None);
}
