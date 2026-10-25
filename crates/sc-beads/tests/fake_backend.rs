//! `FakeBackend`'s own knobs: injections, per-call cost, deadlines, command plan.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::sync::Arc;
use std::time::Duration;

use sc_beads::testing::{bead, id, FakeBackend, FakeCall, FakeOp, Injection, ManualClock};
use sc_beads::{
    BeadId, BeadsCacheError, CacheBackend, Clock, Deadline, Guards, ProjectRef, Refusal,
};

fn setup() -> (Arc<ManualClock>, FakeBackend) {
    let clock = Arc::new(ManualClock::default());
    let fake = FakeBackend::new(Arc::clone(&clock));
    fake.insert(bead("b1"));
    (clock, fake)
}

fn ids(id: &str) -> Vec<BeadId> {
    vec![sc_beads::testing::id(id)]
}

#[test]
fn a_call_slower_than_its_deadline_is_cut_at_the_deadline() {
    let (clock, fake) = setup();
    let p = ProjectRef::local(None);
    fake.set_cost_for(FakeOp::ListByIds, Duration::from_secs(5));
    let d = Deadline::after(&*clock, Duration::from_millis(500));
    let start = clock.elapsed();
    let r = fake.list_by_ids(&p, &ids("b1"), d);
    assert!(matches!(
        r,
        Err(BeadsCacheError::Unavailable {
            timed_out: true,
            ..
        })
    ));
    assert_eq!(
        clock.elapsed().saturating_sub(start),
        Duration::from_millis(500)
    );
    // A fast call costs its time and answers.
    fake.set_cost_for(FakeOp::ListByIds, Duration::from_millis(100));
    let d = Deadline::after(&*clock, Duration::from_millis(500));
    assert_eq!(fake.list_by_ids(&p, &ids("b1"), d).unwrap().len(), 1);
    assert_eq!(
        clock.elapsed().saturating_sub(start),
        Duration::from_millis(600)
    );
}

#[test]
fn injections_are_consumed_in_order_by_the_first_matching_call() {
    let (clock, fake) = setup();
    let p = ProjectRef::local(None);
    let d = Deadline::after(&*clock, Duration::from_secs(30));
    let a = sc_beads::testing::actor("san");
    fake.inject(FakeOp::Close, Injection::Refused(Refusal::Precondition));
    fake.inject(FakeOp::Close, Injection::Failed(1));
    fake.inject_where(
        |c| matches!(c, FakeCall::ListByIds(v) if v.len() == 2),
        Injection::Unavailable { timed_out: false },
    );
    let id = id("b1");
    let g = Guards::default();
    assert!(matches!(
        fake.close_guarded(&p, &id, None, &g, &a, d),
        Err(BeadsCacheError::Refused(Refusal::Precondition))
    ));
    assert!(matches!(
        fake.close_guarded(&p, &id, None, &g, &a, d),
        Err(BeadsCacheError::Failed { code: 1, .. })
    ));
    assert!(
        fake.list_by_ids(&p, &ids("b1"), d).is_ok(),
        "one id does not match the injection"
    );
    let two = [sc_beads::testing::id("b1"), sc_beads::testing::id("b2")];
    assert!(fake.list_by_ids(&p, &two, d).unwrap_err().is_unavailable());
    assert_eq!(fake.pending_injections(), 0);
    fake.close_guarded(&p, &id, Some("r"), &g, &a, d).unwrap();
    assert_eq!(fake.row("b1").unwrap().status.as_deref(), Some("closed"));
    assert_eq!(fake.writes().len(), 3);
    assert_eq!(fake.reads().len(), 2);
    assert_eq!(
        fake.writes().last(),
        Some(&FakeCall::Close {
            id,
            reason: Some("r".into()),
            guards: g,
            actor: a
        })
    );
}

#[test]
fn a_delay_injection_under_the_deadline_still_answers() {
    let (clock, fake) = setup();
    let p = ProjectRef::local(None);
    fake.inject(
        FakeOp::Fingerprint,
        Injection::Delay(Duration::from_millis(100)),
    );
    let d = Deadline::after(&*clock, Duration::from_millis(500));
    assert!(fake.fingerprint(&p, d).is_ok());
    assert_eq!(clock.elapsed(), Duration::from_millis(100));
    assert!(!d.expired(&*clock));
}

#[test]
fn a_schema_change_changes_the_fingerprint() {
    let (clock, fake) = setup();
    let p = ProjectRef::local(None);
    let d = Deadline::after(&*clock, Duration::from_secs(1));
    let before = fake.fingerprint(&p, d).unwrap();
    fake.set_schema("{\"schema_version\": 2}");
    let after = fake.fingerprint(&p, d).unwrap();
    assert_ne!(before.schema_sha256, after.schema_sha256);
    assert_eq!(after.schema_version, "2");
    assert_eq!(clock.now_utc().as_str(), "2026-10-23T10:00:00Z");
}

#[test]
fn a_cursor_lists_a_row_whose_updated_at_does_not_parse() {
    let (clock, fake) = setup();
    let mut odd = bead("o1");
    odd.labels = vec!["phase-x".into()];
    odd.updated_at = "not-a-time".into();
    let mut old = bead("old");
    old.labels = vec!["phase-x".into()];
    fake.insert_all([odd, old]);
    let scope = sc_beads::ScopeFilter::Label("phase-x".into());
    let cursor = sc_beads::SyncCursor::new("2026-10-23T09:30:00Z");
    let d = Deadline::after(&*clock, Duration::from_secs(1));
    let rows = fake
        .list_scope(&ProjectRef::local(None), &scope, Some(&cursor), d)
        .unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        ["o1"],
        "unparseable is listed; the 09:00 row is before the cursor"
    );
}
