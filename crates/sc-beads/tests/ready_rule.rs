//! `is_bd_ready`, the one statement of bd's ready rule above the port, equals
//! recorded bd 1.3.1 `bd ready` answers (B7Q1-036).
//!
//! The bd 1.3.1 scope-filter recording (`tests/fixtures/bd-1.3.1/scope-filters/`)
//! holds every row of a workspace (`list-all.json`), bd's blocked set over all
//! of them (`blocked-all.json`) and `bd ready`'s answer (`ready-all.json`). The
//! rule applied to the rows and the blocked set must name exactly bd's ready
//! ids.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a failed unwrap is a test failure"
)]

use std::collections::BTreeSet;

use sc_beads::cache::is_bd_ready;
use sc_beads::{parse_bd_rows, parse_blocked_rows, BeadId};

const LIST_ALL: &str = include_str!("fixtures/bd-1.3.1/scope-filters/list-all.json");
const BLOCKED_ALL: &str = include_str!("fixtures/bd-1.3.1/scope-filters/blocked-all.json");
const READY_ALL: &str = include_str!("fixtures/bd-1.3.1/scope-filters/ready-all.json");

/// The ready ids the rule gives for the recorded workspace.
fn ruled_ready(now_unix: i64) -> BTreeSet<BeadId> {
    let rows = parse_bd_rows(LIST_ALL).unwrap();
    let blocked: BTreeSet<BeadId> = parse_blocked_rows(BLOCKED_ALL)
        .unwrap()
        .into_iter()
        .filter(|b| !b.blocked_by.is_empty())
        .map(|b| b.id)
        .collect();
    rows.iter()
        .filter(|r| is_bd_ready(r, blocked.contains(&r.id), now_unix))
        .map(|r| r.id.clone())
        .collect()
}

#[test]
fn the_ready_rule_equals_the_recorded_bd_ready_answer() {
    let recorded: BTreeSet<BeadId> = parse_bd_rows(READY_ALL)
        .unwrap()
        .into_iter()
        .map(|b| b.id)
        .collect();
    assert!(!recorded.is_empty(), "the recording lists ready beads");
    // The recording has no deferred bead, so any instant after it agrees.
    assert_eq!(ruled_ready(i64::MAX), recorded);
}

#[test]
fn a_bead_deferred_past_now_is_not_ready() {
    let mut rows = parse_bd_rows(LIST_ALL).unwrap();
    let open = rows
        .iter_mut()
        .find(|r| r.status.as_deref() == Some("open"))
        .expect("the recording has an open bead");
    assert!(is_bd_ready(open, false, 0));
    open.defer_until = Some("2030-01-01T00:00:00Z".into());
    assert!(!is_bd_ready(open, false, 0));
    assert!(is_bd_ready(open, false, i64::MAX));
}
