//! The contract suite every [`CacheBackend`] implementation must pass.
//!
//! An implementation provides a [`BackendFixture`] that builds a backend whose
//! bd holds a given [`World`], sharing a [`ManualClock`]. Instantiate every
//! rule as a `#[test]` with [`backend_contract_tests!`](crate::backend_contract_tests):
//!
//! ```ignore
//! sc_beads::backend_contract_tests!(MyFixture::new());
//! ```
//!
//! `FakeBackend` passes it in this crate (`tests/backend_contract.rs`); a-2
//! runs it against `BdCli` through a recording invoker.
//!
//! The P-6 rules use the recording in `tests/fixtures/p6-blocked/` (bd 1.3.0,
//! see its `PROVENANCE.md`): the world is the recorded `bd list` rows, and the
//! expected answers are the recorded `bd blocked` and `bd ready` outputs before,
//! after closing, and after reopening the outside blocker.
//!
//! # Panics
//!
//! Every rule panics (through `assert!`) when the backend breaks it; that is
//! how a test reports the violation.
#![expect(
    clippy::missing_panics_doc,
    reason = "every rule panics on a violation by design; documented once above"
)]

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use super::{bead, edge, id as bead_id, FakeBackend, ManualClock};
use crate::cache::{
    Assignee, BeadId, BeadsCacheError, CacheBackend, Guards, MetaValue, Patch, Refusal,
    ScopeFilter, SyncCursor,
};
use crate::clock::{Clock, Deadline};
use crate::row::{
    parse_bd_rows, parse_blocked_rows, Bead, DepType, BD_SCHEMA_SHA256, BD_SCHEMA_VERSION,
    BD_VERSION,
};
use crate::types::backend::ProjectRef;

/// The recorded P-6 probe: `bd list` rows of the initial state.
pub const P6_INITIAL_LIST: &str =
    include_str!("../../tests/fixtures/p6-blocked/1-initial-list.json");
/// The recorded P-6 probe: `bd blocked -l phase-p6` of the initial state.
pub const P6_INITIAL_BLOCKED: &str =
    include_str!("../../tests/fixtures/p6-blocked/1-initial-blocked.json");
/// The recorded P-6 probe: `bd ready -l phase-p6` of the initial state.
pub const P6_INITIAL_READY: &str =
    include_str!("../../tests/fixtures/p6-blocked/1-initial-ready.json");
/// `bd blocked` after closing the outside blocker `pr-tqm`.
pub const P6_CLOSED_BLOCKED: &str =
    include_str!("../../tests/fixtures/p6-blocked/2-x-closed-blocked.json");
/// `bd ready` after closing the outside blocker `pr-tqm`.
pub const P6_CLOSED_READY: &str =
    include_str!("../../tests/fixtures/p6-blocked/2-x-closed-ready.json");
/// `bd blocked` after reopening `pr-tqm`.
pub const P6_REOPENED_BLOCKED: &str =
    include_str!("../../tests/fixtures/p6-blocked/3-x-reopened-blocked.json");
/// `bd ready` after reopening `pr-tqm`.
pub const P6_REOPENED_READY: &str =
    include_str!("../../tests/fixtures/p6-blocked/3-x-reopened-ready.json");
/// The P-6 scope label.
pub const P6_LABEL: &str = "phase-p6";

/// The bd 1.3.1 scope-filter recording (`tests/fixtures/bd-1.3.1/scope-filters/`,
/// see its `PROVENANCE.md`): every row of the recorded workspace.
pub const FILTERS_LIST_ALL: &str =
    include_str!("../../tests/fixtures/bd-1.3.1/scope-filters/list-all.json");

/// One recorded answer of the scope-filter recording: the fixture's name and stdout.
const FILTER_RECORDINGS: &[(&str, &str)] = &[
    ("list-all", FILTERS_LIST_ALL),
    (
        "list-criteria",
        include_str!("../../tests/fixtures/bd-1.3.1/scope-filters/list-criteria.json"),
    ),
    (
        "list-label-any",
        include_str!("../../tests/fixtures/bd-1.3.1/scope-filters/list-label-any.json"),
    ),
    (
        "list-label",
        include_str!("../../tests/fixtures/bd-1.3.1/scope-filters/list-label.json"),
    ),
    (
        "blocked-all",
        include_str!("../../tests/fixtures/bd-1.3.1/scope-filters/blocked-all.json"),
    ),
    (
        "blocked-criteria",
        include_str!("../../tests/fixtures/bd-1.3.1/scope-filters/blocked-criteria.json"),
    ),
    (
        "ready-all",
        include_str!("../../tests/fixtures/bd-1.3.1/scope-filters/ready-all.json"),
    ),
    (
        "ready-criteria",
        include_str!("../../tests/fixtures/bd-1.3.1/scope-filters/ready-criteria.json"),
    ),
    (
        "ready-label-any",
        include_str!("../../tests/fixtures/bd-1.3.1/scope-filters/ready-label-any.json"),
    ),
];

/// The filter each recording of the scope-filter fixture was made with, by
/// name: the variant whose argv `sc-bd-cli` builds is the recorded argv
/// (`argv.json`). `blocked-criteria` and `ready-criteria` share one filter,
/// whose type and assignee `bd blocked` does not take.
#[must_use]
pub fn recorded_filter(name: &str) -> Option<ScopeFilter> {
    let strings = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let criteria =
        |all: &[&str], any: &[&str], t: Option<&str>, a: Option<&str>| ScopeFilter::Criteria {
            labels_all: strings(all),
            labels_any: strings(any),
            issue_type: t.map(str::to_owned),
            assignee: a.map(str::to_owned),
        };
    Some(match name {
        "list-all" | "blocked-all" | "ready-all" => ScopeFilter::All,
        "list-criteria" => criteria(
            &["area-x", "team-y"],
            &["team-y", "area-z"],
            Some("task"),
            Some("dev"),
        ),
        "list-label-any" => criteria(&[], &["team-y", "area-z"], None, None),
        "list-label" => ScopeFilter::Label("area-x".to_owned()),
        "blocked-criteria" | "ready-criteria" => criteria(
            &["area-x"],
            &["area-x", "team-y"],
            Some("task"),
            Some("dev"),
        ),
        "ready-label-any" => criteria(&[], &["area-x", "team-y"], None, None),
        _ => return None,
    })
}

/// The names of the scope-filter recordings, in the order of `argv.json`.
#[must_use]
pub fn recorded_filter_names() -> Vec<&'static str> {
    FILTER_RECORDINGS.iter().map(|(name, _)| *name).collect()
}
/// The P-6 outside blocker that is closed and reopened.
pub const P6_OUTSIDE_BLOCKER: &str = "pr-tqm";

/// The bd state a fixture must present.
#[derive(Debug, Clone, Default)]
pub struct World {
    /// Every row bd holds, as `bd list --json` would print them.
    pub rows: Vec<Bead>,
}

impl World {
    /// A world holding these rows.
    #[must_use]
    pub fn new(rows: Vec<Bead>) -> Self {
        Self { rows }
    }

    /// The recorded P-6 probe world.
    ///
    /// # Panics
    ///
    /// When the vendored fixture fails strict validation (a fixture bug).
    #[must_use]
    pub fn p6() -> Self {
        let rows = parse_bd_rows(P6_INITIAL_LIST);
        assert!(rows.is_ok(), "P-6 fixture rows must validate: {rows:?}");
        Self::new(rows.unwrap_or_default())
    }
}

/// Builds the backend under test.
pub trait BackendFixture {
    /// The implementation under test.
    type Backend: CacheBackend;

    /// A backend whose bd holds `world`, reading time from `clock`.
    fn backend(&mut self, world: &World, clock: Arc<ManualClock>) -> Self::Backend;
}

/// The fixture for [`FakeBackend`].
#[derive(Debug, Clone, Copy, Default)]
pub struct FakeBackendFixture;

impl BackendFixture for FakeBackendFixture {
    type Backend = FakeBackend;

    fn backend(&mut self, world: &World, clock: Arc<ManualClock>) -> FakeBackend {
        let fake = FakeBackend::new(clock);
        fake.insert_all(world.rows.iter().cloned());
        fake
    }
}

fn project() -> ProjectRef {
    ProjectRef::local(None)
}

fn generous(clock: &ManualClock) -> Deadline {
    Deadline::after(clock, Duration::from_secs(30))
}

fn labelled(id: &str, label: &str) -> Bead {
    let mut b = bead(id);
    b.labels = vec![label.to_owned()];
    b
}

fn ids(rows: &[Bead]) -> BTreeSet<String> {
    rows.iter().map(|r| r.id.as_str().to_owned()).collect()
}

fn blocked_set(rows: Vec<crate::cache::BlockedRow>) -> BTreeSet<(String, BTreeSet<String>)> {
    rows.into_iter()
        .map(|r| {
            (
                r.id.into_string(),
                r.blocked_by.into_iter().map(BeadId::into_string).collect(),
            )
        })
        .collect()
}

fn recorded_blocked(text: &str) -> BTreeSet<(String, BTreeSet<String>)> {
    let rows = parse_blocked_rows(text);
    assert!(rows.is_ok(), "recorded bd blocked must validate: {rows:?}");
    blocked_set(rows.unwrap_or_default())
}

fn recorded_ready(text: &str) -> BTreeSet<String> {
    let rows = parse_bd_rows(text);
    assert!(rows.is_ok(), "recorded bd ready must validate: {rows:?}");
    ids(&rows.unwrap_or_default())
}

/// `updated_at` as an instant; a value that does not parse fails the contract.
#[expect(
    clippy::panic,
    reason = "a contract rule reports a failure by panicking, as assert! does"
)]
fn instant(updated_at: &str) -> i64 {
    match crate::clock::rfc3339_to_unix(updated_at) {
        Some(t) => t,
        None => panic!("updated_at {updated_at:?} is not an RFC 3339 date-time"),
    }
}

#[expect(
    clippy::panic,
    reason = "a contract rule reports a failure by panicking, as assert! does"
)]
fn first(rows: Vec<Bead>) -> Bead {
    match rows.into_iter().next() {
        Some(row) => row,
        None => panic!("expected the read-back row"),
    }
}

#[expect(
    clippy::panic,
    reason = "a contract rule reports a failure by panicking, as assert! does"
)]
fn ok<T>(r: Result<T, BeadsCacheError>, what: &str) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("{what}: unexpected error {e}"),
    }
}

/// `fingerprint` reports bd's version and the sha256 and version of its schema.
pub fn fingerprint_reports_version_and_schema(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::default());
    let b = fx.backend(&World::default(), Arc::clone(&clock));
    let f = ok(b.fingerprint(&project(), generous(&clock)), "fingerprint");
    assert_eq!(f.bd_version, BD_VERSION);
    assert_eq!(f.schema_sha256.as_str(), BD_SCHEMA_SHA256);
    assert_eq!(f.schema_version, BD_SCHEMA_VERSION);
}

/// `list_by_ids` returns the requested rows, closed ones included, omits unknown ids,
/// and no row carries `is_blocked`.
pub fn list_by_ids_returns_every_status(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::default());
    let mut closed = bead("c1");
    closed.status = Some("closed".into());
    closed.closed_at = Some("2026-10-23T09:30:00Z".into());
    let world = World::new(vec![bead("o1"), closed, bead("other")]);
    let b = fx.backend(&world, Arc::clone(&clock));
    let want = [bead_id("o1"), bead_id("c1"), bead_id("missing")];
    let rows = ok(
        b.list_by_ids(&project(), &want, generous(&clock)),
        "list_by_ids",
    );
    assert_eq!(
        ids(&rows),
        BTreeSet::from(["o1".to_owned(), "c1".to_owned()])
    );
    assert!(
        rows.iter().all(|r| r.is_blocked.is_none()),
        "rows carry no is_blocked"
    );
}

/// `list_scope` selects by label and, with a cursor, only rows updated after it.
pub fn list_scope_filters_by_label_and_cursor(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::default());
    let mut newer = labelled("n1", "phase-x");
    newer.updated_at = "2026-10-23T09:10:00Z".into();
    let mut closed = labelled("c1", "phase-x");
    closed.status = Some("closed".into());
    let world = World::new(vec![
        labelled("a1", "phase-x"),
        newer,
        closed,
        labelled("y1", "phase-y"),
    ]);
    let b = fx.backend(&world, Arc::clone(&clock));
    let scope = ScopeFilter::Label("phase-x".into());
    let all = ok(
        b.list_scope(&project(), &scope, None, generous(&clock)),
        "list_scope",
    );
    assert_eq!(
        ids(&all),
        BTreeSet::from(["a1".into(), "n1".into(), "c1".into()])
    );
    let cursor = SyncCursor::new("2026-10-23T09:05:00Z");
    let since = ok(
        b.list_scope(&project(), &scope, Some(&cursor), generous(&clock)),
        "list_scope since",
    );
    assert_eq!(ids(&since), BTreeSet::from(["n1".into()]));
}

/// Every [`ScopeFilter`] variant answers as bd 1.3.1 did in the scope-filter
/// recording (ADR-BDC-0013): `list_scope`, `list_blocked` and `list_ready` of
/// `All`, of label and type and assignee `Criteria` and of `Label`, over the
/// recorded workspace; and an empty `Criteria` is refused before any bd call.
pub fn scope_lists_answer_every_filter_variant(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::default());
    let rows = parse_bd_rows(FILTERS_LIST_ALL);
    assert!(rows.is_ok(), "recorded rows must validate: {rows:?}");
    let world = World::new(rows.unwrap_or_default());
    let b = fx.backend(&world, Arc::clone(&clock));
    for (name, stdout) in FILTER_RECORDINGS {
        let filter = recorded_filter(name);
        assert!(filter.is_some(), "no filter for recording {name}");
        let filter = filter.unwrap_or(ScopeFilter::All);
        let (got, want): (BTreeSet<String>, BTreeSet<String>) = if name.starts_with("blocked") {
            let got = ok(
                b.list_blocked(&project(), &filter, generous(&clock)),
                "list_blocked",
            );
            let want = parse_blocked_rows(stdout);
            assert!(want.is_ok(), "{name}: {want:?}");
            (
                got.iter().map(|r| r.id.as_str().to_owned()).collect(),
                want.unwrap_or_default()
                    .iter()
                    .map(|r| r.id.as_str().to_owned())
                    .collect(),
            )
        } else if name.starts_with("ready") {
            let got = ok(
                b.list_ready(&project(), &filter, generous(&clock)),
                "list_ready",
            );
            (
                got.iter().map(|id| id.as_str().to_owned()).collect(),
                recorded_ids(name, stdout),
            )
        } else {
            let got = ok(
                b.list_scope(&project(), &filter, None, generous(&clock)),
                "list_scope",
            );
            (
                got.iter().map(|r| r.id.as_str().to_owned()).collect(),
                recorded_ids(name, stdout),
            )
        };
        assert_eq!(got, want, "{name}: {filter:?}");
    }
    let empty = ScopeFilter::Criteria {
        labels_all: Vec::new(),
        labels_any: Vec::new(),
        issue_type: None,
        assignee: None,
    };
    let refused = b.list_scope(&project(), &empty, None, generous(&clock));
    assert!(
        matches!(refused, Err(BeadsCacheError::Invalid(_))),
        "an empty criteria filter is refused: {refused:?}"
    );
    assert!(matches!(
        b.list_blocked(&project(), &empty, generous(&clock)),
        Err(BeadsCacheError::Invalid(_))
    ));
    assert!(matches!(
        b.list_ready(&project(), &empty, generous(&clock)),
        Err(BeadsCacheError::Invalid(_))
    ));
}

/// The ids of a recorded `bd list` / `bd ready` answer.
fn recorded_ids(name: &str, stdout: &str) -> BTreeSet<String> {
    let rows = parse_bd_rows(stdout);
    assert!(rows.is_ok(), "{name}: {rows:?}");
    rows.unwrap_or_default()
        .iter()
        .map(|r| r.id.as_str().to_owned())
        .collect()
}

/// `list_blocked` and `list_ready` match the recorded P-6 probe: `blocks`,
/// parent-child inheritance (the child lists its parent), `related`, a closed
/// blocker and a blocker outside the label; then after closing and reopening
/// the outside blocker.
pub fn blocked_and_ready_match_the_p6_probe(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::new("2026-10-24T04:41:30Z"));
    let b = fx.backend(&World::p6(), Arc::clone(&clock));
    let scope = ScopeFilter::Label(P6_LABEL.into());
    let check = |blocked: &str, ready: &str, state: &str| {
        let got = ok(
            b.list_blocked(&project(), &scope, generous(&clock)),
            "list_blocked",
        );
        assert_eq!(
            blocked_set(got),
            recorded_blocked(blocked),
            "bd blocked, {state}"
        );
        let got: BTreeSet<String> = ok(
            b.list_ready(&project(), &scope, generous(&clock)),
            "list_ready",
        )
        .into_iter()
        .map(BeadId::into_string)
        .collect();
        assert_eq!(got, recorded_ready(ready), "bd ready, {state}");
    };
    check(P6_INITIAL_BLOCKED, P6_INITIAL_READY, "initial");
    let x = bead_id(P6_OUTSIDE_BLOCKER);
    let actor = super::actor("contract");
    ok(
        b.close_guarded(
            &project(),
            &x,
            Some("probe close"),
            &Guards::default(),
            &actor,
            generous(&clock),
        ),
        "close x",
    );
    check(
        P6_CLOSED_BLOCKED,
        P6_CLOSED_READY,
        "after closing the outside blocker",
    );
    let reopen = Patch {
        status: Some("open".into()),
        ..Patch::default()
    };
    ok(
        b.update_guarded(
            &project(),
            &x,
            &reopen,
            &Guards::default(),
            &actor,
            generous(&clock),
        ),
        "reopen x",
    );
    check(
        P6_REOPENED_BLOCKED,
        P6_REOPENED_READY,
        "after reopening the outside blocker",
    );
}

/// Closing and reopening a blocker bumps its own `updated_at` only, never the
/// depender's (bd's `BlockedStateInvariant`; P-6).
pub fn close_and_reopen_bump_only_the_beads_own_updated_at(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::new("2026-10-23T10:00:00Z"));
    let mut dep = labelled("d1", "phase-x");
    dep.dependencies = vec![edge("d1", "x1", DepType::Blocks)];
    let b = fx.backend(&World::new(vec![bead("x1"), dep]), Arc::clone(&clock));
    let read = |id: &str| {
        let rows = ok(
            b.list_by_ids(&project(), &[bead_id(id)], generous(&clock)),
            "read back",
        );
        first(rows)
    };
    let before = read("d1").updated_at;
    clock.advance(Duration::from_secs(5));
    let x = bead_id("x1");
    let actor = super::actor("contract");
    ok(
        b.close_guarded(
            &project(),
            &x,
            Some("done"),
            &Guards::default(),
            &actor,
            generous(&clock),
        ),
        "close",
    );
    let closed = read("x1");
    assert_eq!(closed.status.as_deref(), Some("closed"));
    assert_eq!(closed.close_reason.as_deref(), Some("done"));
    assert!(
        instant(&closed.updated_at) > instant(&bead("x1").updated_at),
        "close bumps the bead"
    );
    clock.advance(Duration::from_secs(5));
    let reopen = Patch {
        status: Some("open".into()),
        ..Patch::default()
    };
    ok(
        b.update_guarded(
            &project(),
            &x,
            &reopen,
            &Guards::default(),
            &actor,
            generous(&clock),
        ),
        "reopen",
    );
    let reopened = read("x1");
    assert_eq!(reopened.status.as_deref(), Some("open"));
    assert!(
        instant(&reopened.updated_at) > instant(&closed.updated_at),
        "reopen bumps the bead"
    );
    assert_eq!(
        read("d1").updated_at,
        before,
        "the depender's updated_at never moves"
    );
}

/// A guard that no longer holds is `Refused(Precondition)` and changes nothing;
/// `""` means unassigned.
pub fn unmet_guards_are_refused(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::default());
    let mut b1 = bead("b1");
    b1.status = Some("in_progress".into());
    b1.assignee = Some("dev".into());
    let b = fx.backend(
        &World::new(vec![b1.clone(), bead("b2")]),
        Arc::clone(&clock),
    );
    let actor = super::actor("contract");
    let id = bead_id("b1");
    let wrong_status = Guards {
        status: Some("open".into()),
        assignee: None,
    };
    let r = b.close_guarded(
        &project(),
        &id,
        None,
        &wrong_status,
        &actor,
        generous(&clock),
    );
    assert!(
        matches!(r, Err(BeadsCacheError::Refused(Refusal::Precondition))),
        "close: {r:?}"
    );
    let unassigned = Guards {
        status: None,
        assignee: Some(Assignee::Unassigned),
    };
    let patch = Patch {
        status: Some("open".into()),
        ..Patch::default()
    };
    let r = b.update_guarded(
        &project(),
        &id,
        &patch,
        &unassigned,
        &actor,
        generous(&clock),
    );
    assert!(
        matches!(r, Err(BeadsCacheError::Refused(Refusal::Precondition))),
        "update: {r:?}"
    );
    let rows = ok(
        b.list_by_ids(&project(), &[id], generous(&clock)),
        "read back",
    );
    assert_eq!(
        rows.first().and_then(|r| r.status.clone()).as_deref(),
        Some("in_progress")
    );
    // `""` matches an unassigned bead.
    let r = b.update_guarded(
        &project(),
        &bead_id("b2"),
        &patch,
        &unassigned,
        &actor,
        generous(&clock),
    );
    assert!(r.is_ok(), "unassigned guard on an unassigned bead: {r:?}");
}

/// `update_guarded` applies every patch field; `dep_add_typed` adds the edge.
pub fn writes_apply_their_effect(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::default());
    let mut b2 = labelled("b2", "old");
    b2.notes = Some("first".into());
    b2.metadata.insert("gone".into(), serde_json::json!(1));
    let b = fx.backend(&World::new(vec![bead("b1"), b2]), Arc::clone(&clock));
    let actor = super::actor("contract");
    let id = bead_id("b2");
    let patch = Patch {
        status: Some("in_progress".into()),
        assignee: Some(Assignee::Named("dev".into())),
        add_labels: vec!["new".into()],
        remove_labels: vec!["old".into()],
        set_metadata: [
            ("n".to_owned(), MetaValue::Int(5)),
            ("ok".to_owned(), MetaValue::Bool(true)),
            ("sha".to_owned(), MetaValue::Str("abc123".into())),
        ]
        .into_iter()
        .collect(),
        unset_metadata: vec!["gone".into()],
        append_notes: Some("second".into()),
    };
    ok(
        b.update_guarded(
            &project(),
            &id,
            &patch,
            &Guards::default(),
            &actor,
            generous(&clock),
        ),
        "update",
    );
    ok(
        b.dep_add_typed(
            &project(),
            &id,
            &bead_id("b1"),
            DepType::Blocks,
            &actor,
            generous(&clock),
        ),
        "dep add",
    );
    let rows = ok(
        b.list_by_ids(&project(), &[id], generous(&clock)),
        "read back",
    );
    let row = first(rows);
    assert_eq!(row.status.as_deref(), Some("in_progress"));
    assert_eq!(row.assignee.as_deref(), Some("dev"));
    assert_eq!(row.labels, ["new"]);
    assert_eq!(
        serde_json::Value::Object(row.metadata),
        serde_json::json!({"n": 5, "ok": true, "sha": "abc123"})
    );
    assert_eq!(row.notes.as_deref(), Some("first\nsecond"));
    assert_eq!(row.dependencies, [edge("b2", "b1", DepType::Blocks)]);
}

/// A write to a bead bd does not know fails and is not a refusal.
pub fn writes_to_unknown_beads_fail(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::default());
    let b = fx.backend(&World::default(), Arc::clone(&clock));
    let r = b.close_guarded(
        &project(),
        &bead_id("nope"),
        None,
        &Guards::default(),
        &super::actor("contract"),
        generous(&clock),
    );
    assert!(
        r.is_err() && !matches!(r, Err(BeadsCacheError::Refused(_))),
        "{r:?}"
    );
}

/// Every call stops at an expired deadline with `Unavailable { timed_out: true }`.
#[expect(
    clippy::many_single_char_names,
    reason = "the call arguments mirror the trait's own parameter names"
)]
pub fn every_call_stops_at_its_deadline(fx: &mut impl BackendFixture) {
    let clock = Arc::new(ManualClock::default());
    let b = fx.backend(
        &World::new(vec![labelled("b1", "phase-x")]),
        Arc::clone(&clock),
    );
    let d = Deadline::after(&*clock, Duration::ZERO);
    let p = project();
    let id = bead_id("b1");
    let a = super::actor("contract");
    let g = Guards::default();
    let scope = ScopeFilter::Label("phase-x".into());
    let timed_out = |r: Result<(), BeadsCacheError>, what: &str| {
        assert!(
            matches!(
                r,
                Err(BeadsCacheError::Unavailable {
                    timed_out: true,
                    ..
                })
            ),
            "{what}: expected a timeout, got {r:?}"
        );
    };
    timed_out(b.fingerprint(&p, d).map(drop), "fingerprint");
    timed_out(
        b.list_by_ids(&p, std::slice::from_ref(&id), d).map(drop),
        "list_by_ids",
    );
    timed_out(b.list_scope(&p, &scope, None, d).map(drop), "list_scope");
    timed_out(b.list_blocked(&p, &scope, d).map(drop), "list_blocked");
    timed_out(b.list_ready(&p, &scope, d).map(drop), "list_ready");
    timed_out(b.close_guarded(&p, &id, None, &g, &a, d), "close_guarded");
    timed_out(
        b.update_guarded(&p, &id, &Patch::default(), &g, &a, d),
        "update_guarded",
    );
    timed_out(
        b.dep_add_typed(&p, &id, &id, DepType::Related, &a, d),
        "dep_add_typed",
    );
    assert!(clock.now_mono() >= d.instant());
}

/// Instantiates the `CacheBackend` contract suite as `#[test]` functions.
///
/// The argument is an expression building a fresh
/// [`BackendFixture`](crate::testing::contract::BackendFixture); it is
/// evaluated once per test.
///
/// ```ignore
/// sc_beads::backend_contract_tests!(sc_beads::testing::contract::FakeBackendFixture);
/// ```
#[macro_export]
macro_rules! backend_contract_tests {
    ($fixture:expr) => {
        $crate::backend_contract_tests!(@rules $fixture;
            fingerprint_reports_version_and_schema,
            list_by_ids_returns_every_status,
            list_scope_filters_by_label_and_cursor,
            scope_lists_answer_every_filter_variant,
            blocked_and_ready_match_the_p6_probe,
            close_and_reopen_bump_only_the_beads_own_updated_at,
            unmet_guards_are_refused,
            writes_apply_their_effect,
            writes_to_unknown_beads_fail,
            every_call_stops_at_its_deadline,
        );
    };
    (@rules $fixture:expr; $($rule:ident),* $(,)?) => {
        $(
            #[test]
            fn $rule() {
                let mut fixture = $fixture;
                $crate::testing::contract::$rule(&mut fixture);
            }
        )*
    };
}
