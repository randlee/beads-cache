//! `ingest_scope` through the core (REQ-BDC-0726, REQ-BDC-0007, 0008, 0502; AC2, AC5).
//!
//! One test per Ingest rule: the strict decoder, declared membership, the built-in
//! `blocked` rule, empty-snapshot refusal, the 50 000-row / 64 MiB limits,
//! `INGEST_INVALID` with the stored scope untouched, other memberships kept, the
//! newer-row guard and the opaque `revision`; and the regression test that runs a real
//! built-in snapshot (rows and blocked, as `bd list` and `bd blocked` answered the
//! built-in sync) through ingest and finds the same scope stored. No ingest starts a bd
//! process: the fake records no call.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::store;
use bd_cache_model::testing::FakeStore;
use bd_cache_model::{
    CacheConfig, CacheError, CacheStore, FilterParts, IngestReason, IngestSnapshot,
    LEGACY_SCOPE_PREFIX, MAX_INGEST_BYTES, MAX_INGEST_ROWS, Scope, ScopeFilter, ScopeId,
};
use common::{World, id, sx, x};
use sc_beads::{Bead, CacheBackend, DepType, ProjectRef};

const T30: Duration = Duration::from_secs(30);

fn lane() -> Scope {
    Scope::new(
        ScopeId::new("lane").unwrap(),
        ScopeFilter::from_parts(FilterParts {
            labels_all: vec!["lane".into()],
            status: vec!["open".into(), "in_progress".into()],
            ..FilterParts::default()
        })
        .unwrap(),
    )
}

fn lane_bead(w: &World, name: &str) -> Bead {
    w.add(name, |b| {
        b.labels = vec!["lane".into(), format!("{LEGACY_SCOPE_PREFIX}x")];
    })
}

/// The rows and blocked rows bd answers for `scope`, as the built-in sync reads them.
fn snapshot_of(w: &World, scope: &Scope) -> (Vec<Bead>, Vec<sc_beads::BlockedRow>) {
    let p = ProjectRef::local(None);
    let filter = scope.backend_filter();
    let rows = w.bd.list_scope(&p, &filter, None, w.after(T30)).unwrap();
    let blocked = w.bd.list_blocked(&p, &filter, w.after(T30)).unwrap();
    (rows, blocked)
}

fn member_ids<S: CacheStore>(s: &S, scope: &ScopeId) -> Vec<String> {
    s.members(scope)
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// A real built-in snapshot through ingest stores what the built-in sync stores:
/// the same members and the same ready base (AC2's regression test).
#[test]
fn a_built_in_snapshot_through_ingest_stores_what_the_built_in_sync_stores() {
    let w = World::new();
    lane_bead(&w, "l1");
    lane_bead(&w, "l2");
    let closed = w.add("l3", |b| {
        b.labels = vec!["lane".into()];
        b.status = Some("closed".into());
    });
    w.add("out", |b| b.labels = vec!["other".into()]);
    w.bd.add_edge("l2", "out", DepType::Blocks);
    w.bd.add_edge("l1", "b1", DepType::Blocks);
    let config = CacheConfig::new(ProjectRef::local(None), vec![lane(), sx()]);
    let mut built_in = w.cache_with(config.clone());
    built_in.sync(&lane().id, w.after(T30)).unwrap();

    let (rows, blocked) = snapshot_of(&w, &lane());
    assert!(
        rows.iter().any(|r| r.id == closed.id),
        "bd lists every status"
    );
    let other = FakeStore::new();
    let mut ingesting = bd_cache::Cache::new(
        config,
        other.clone(),
        w.lock.clone(),
        w.bd.clone(),
        std::sync::Arc::clone(&w.clock) as _,
    )
    .unwrap();
    let calls = w.bd.calls().len();
    let snap = IngestSnapshot::new(&lane().id, rows, blocked).unwrap();
    ingesting
        .ingest_scope(&lane().id, &snap, Some("r1"), false)
        .unwrap();
    assert_eq!(w.bd.calls().len(), calls, "an ingest starts no bd process");

    assert_eq!(
        member_ids(&other, &lane().id),
        member_ids(store(&built_in), &lane().id),
        "the same members (the status criterion applied)"
    );
    assert_eq!(member_ids(&other, &lane().id), ["l1", "l2"]);
    let a = store(&built_in).ready_base(&lane().id).unwrap().unwrap();
    let b = other.ready_base(&lane().id).unwrap().unwrap();
    assert_eq!((a.blocked, a.ready), (b.blocked, b.ready), "the same base");
    assert_eq!(
        other.scope_revision(&lane().id).unwrap(),
        Some("r1".to_owned())
    );
}

/// The strict decoder: one invalid row among valid ones is `INGEST_INVALID`
/// (`row_invalid`, its index), and no row of the call is stored (REQ-BDC-0008).
#[test]
fn one_invalid_row_among_valid_ones_stores_none_of_them() {
    let w = World::new();
    let mut c = w.cache_with(CacheConfig::new(ProjectRef::local(None), vec![lane()]));
    let good = serde_json::to_value(lane_bead(&w, "l1")).unwrap();
    let bad = serde_json::json!({"id": "l2", "surprise": 1});
    let rows = serde_json::Value::Array(vec![good.clone(), bad, good]).to_string();
    let e = IngestSnapshot::from_json(&lane().id, &rows, "[]").unwrap_err();
    assert!(
        matches!(
            e,
            CacheError::IngestInvalid {
                reason: IngestReason::RowInvalid,
                index: Some(1),
                ..
            }
        ),
        "{e:?}"
    );
    assert!(
        store(&c).get_row(&id("l1")).unwrap().is_none(),
        "nothing stored"
    );
    let calls = w.bd.calls().len();
    let one = serde_json::Value::Array(vec![serde_json::to_value(w.bd_row("l1")).unwrap()]);
    let snap = IngestSnapshot::from_json(&lane().id, &one.to_string(), "null").unwrap();
    c.ingest_scope(&lane().id, &snap, None, false).unwrap();
    assert_eq!(member_ids(store(&c), &lane().id), ["l1"]);
    assert_eq!(w.bd.calls().len(), calls, "no bd process");
}

/// An empty snapshot is refused unless allowed; the stored scope is untouched.
#[test]
fn an_empty_snapshot_is_refused_unless_allowed() {
    let w = World::new();
    lane_bead(&w, "l1");
    let mut c = w.cache_with(CacheConfig::new(ProjectRef::local(None), vec![lane()]));
    c.sync(&lane().id, w.after(T30)).unwrap();
    let empty = IngestSnapshot::new(&lane().id, vec![], vec![]).unwrap();
    let e = c.ingest_scope(&lane().id, &empty, None, false).unwrap_err();
    assert!(
        matches!(
            e,
            CacheError::IngestInvalid {
                reason: IngestReason::Empty,
                ..
            }
        ),
        "{e:?}"
    );
    assert_eq!(member_ids(store(&c), &lane().id), ["l1"], "untouched");
    c.ingest_scope(&lane().id, &empty, None, true).unwrap();
    assert!(
        member_ids(store(&c), &lane().id).is_empty(),
        "allowed: the scope is empty"
    );
}

/// The limits: more than 50 000 rows or 64 MiB of input is `INGEST_INVALID`.
#[test]
fn the_row_and_size_limits_are_refused() {
    let w = World::new();
    let row = lane_bead(&w, "l1");
    let many: Vec<Bead> = (0..=MAX_INGEST_ROWS)
        .map(|i| {
            let mut b = row.clone();
            b.id = id(&format!("l{i}"));
            b
        })
        .collect();
    let e = IngestSnapshot::new(&lane().id, many, vec![]).unwrap_err();
    assert!(
        matches!(
            e,
            CacheError::IngestInvalid {
                reason: IngestReason::TooManyRows,
                ..
            }
        ),
        "{e:?}"
    );
    let huge = " ".repeat(MAX_INGEST_BYTES + 1);
    let e = IngestSnapshot::from_json(&lane().id, &huge, "[]").unwrap_err();
    assert!(
        matches!(
            e,
            CacheError::IngestInvalid {
                reason: IngestReason::TooLarge,
                ..
            }
        ),
        "{e:?}"
    );
    let twice = IngestSnapshot::new(&lane().id, vec![row.clone(), row], vec![]).unwrap_err();
    assert!(
        matches!(
            twice,
            CacheError::IngestInvalid {
                reason: IngestReason::Membership,
                index: Some(1),
                ..
            }
        ),
        "{twice:?}"
    );
}

/// Declared membership replaces the scope's members; other scopes keep theirs; the
/// newer-row guard keeps a newer stored row; the sync state is an ok result.
#[test]
fn declared_membership_replaces_the_scope_and_keeps_other_scopes() {
    let w = World::new();
    let l1 = lane_bead(&w, "l1");
    let l2 = lane_bead(&w, "l2");
    let mut c = w.cache_with(CacheConfig::new(
        ProjectRef::local(None),
        vec![lane(), sx()],
    ));
    c.sync(&x(), w.after(T30)).unwrap();
    c.sync(&lane().id, w.after(T30)).unwrap();
    assert_eq!(member_ids(store(&c), &lane().id), ["l1", "l2"]);
    // A snapshot listing only an older l1: l2 leaves the lane, keeps x.
    let mut older = l1.clone();
    older.title = "older".into();
    older.updated_at = "2026-10-22T00:00:00Z".into();
    let snap = IngestSnapshot::new(&lane().id, vec![older], vec![]).unwrap();
    let report = c
        .ingest_scope(&lane().id, &snap, Some("r2"), false)
        .unwrap();
    assert_eq!(
        report.skipped_newer,
        vec![l1.id.clone()],
        "the newer-row guard"
    );
    assert_eq!(
        store(&c).get_row(&l1.id).unwrap().unwrap().0.title,
        l1.title
    );
    assert_eq!(member_ids(store(&c), &lane().id), ["l1"]);
    assert!(
        member_ids(store(&c), &x()).contains(&l2.id.to_string()),
        "x keeps l2"
    );
    let st = c.sync_state(&lane().id).unwrap();
    assert!(st.last_ok_at.is_some() && st.failing_since.is_none());
    assert_eq!(c.scope_revision(&lane().id).unwrap(), Some("r2".to_owned()));
}

/// An ingest of an unregistered scope is `SCOPE_UNKNOWN` and registers nothing:
/// only a sync registers on demand.
#[test]
fn an_unregistered_scope_is_scope_unknown() {
    let w = World::new();
    let mut c = w.cache();
    let lane = lane();
    let snap = IngestSnapshot::new(&lane.id, vec![w.bd_row("b1")], vec![]).unwrap();
    let e = c.ingest_scope(&lane.id, &snap, None, false).unwrap_err();
    assert!(matches!(e, CacheError::ScopeUnknown { .. }), "{e:?}");
    assert_eq!(c.scope(&lane.id).unwrap(), None);
}

/// Ingest declares membership (REQ-BDC-0726): a row the scope's filter does
/// not select, ingested into a label-filter scope, is a member everywhere:
/// `list`, `ready`, `get` and its blocked state agree (B7Q1-008).
#[test]
fn an_ingested_row_the_filter_does_not_select_is_a_member_on_every_read() {
    let w = World::new();
    let area = Scope::new(
        ScopeId::new("area").unwrap(),
        ScopeFilter::label("area").unwrap(),
    );
    let mut c = w.cache_with(CacheConfig::new(
        ProjectRef::local(None),
        vec![area.clone()],
    ));
    // No `area` label: the filter does not select it, the snapshot declares it.
    let stray = w.add("stray", |b| b.labels = vec!["other".into()]);
    assert!(!area.selects(&stray));
    let snap = IngestSnapshot::new(&area.id, vec![stray.clone()], vec![]).unwrap();
    c.ingest_scope(&area.id, &snap, None, false).unwrap();
    let listed = c.list(&area.id, None, None).unwrap();
    assert_eq!(listed.rows.len(), 1);
    assert_eq!(listed.rows[0].blocked, Some(vec![]), "list: in the base");
    let ready = c.ready(&area.id, None).unwrap();
    assert_eq!(
        ready
            .ready
            .iter()
            .map(|r| r.bead.id.clone())
            .collect::<Vec<_>>(),
        vec![stray.id.clone()],
        "ready: an open, unblocked member"
    );
    assert_eq!(ready.ready[0].blocked, Some(vec![]));
    let got = c.get(&stray.id, None).unwrap();
    assert_eq!(
        got.scope,
        Some(area.id.clone()),
        "get: the scope it is a member of"
    );
    assert_eq!(got.row.blocked, Some(vec![]), "get: the same blocked state");
    // Planted negative: a bead that is no member is in no scope.
    let outside = w.add("outside", |b| b.labels = vec!["other".into()]);
    let got = c.get(&outside.id, None).unwrap();
    assert_eq!((got.scope, got.row.blocked), (None, None));
}

/// `prune` of an unregistered scope is `SCOPE_UNKNOWN` and sweeps nothing:
/// an out-of-scope row stays (ADR-BDC-0013, B7Q1-009).
#[test]
fn a_prune_of_an_unknown_scope_is_refused_and_sweeps_nothing() {
    let w = World::new();
    let mut c = w.cache();
    w.add("loose", |b| b.labels.clear());
    c.get(&id("loose"), None).unwrap();
    let ghost = ScopeId::new("ghost").unwrap();
    let e = c.prune(&ghost).unwrap_err();
    assert!(matches!(e, CacheError::ScopeUnknown { .. }), "{e:?}");
    assert!(
        store(&c).get_row(&id("loose")).unwrap().is_some(),
        "no out-of-scope sweep ran"
    );
    assert_eq!(c.scope(&ghost).unwrap(), None, "nothing registered");
    // A registered scope still prunes, sweeping the loose row.
    let r = c.prune(&x()).unwrap();
    assert!(r.out_of_scope_swept >= 1, "{r:?}");
}

/// The alert verbs are the one exception to ADR-BDC-0013's refusal of an
/// unknown scope (Approved, lead ruling, sprint b-7; until b-8 makes them
/// refuse unknown names): on an unregistered valid name they succeed and
/// register no scope (B7Q1-009).
#[test]
fn the_alert_verbs_accept_an_unregistered_name_and_register_no_scope() {
    let w = World::new();
    let mut c = w.cache();
    let ghost = ScopeId::new("ghost").unwrap();
    let known = c.scopes().unwrap();
    assert_eq!(c.sync_alert(&ghost).unwrap(), None);
    c.set_sync_alert(&ghost, Some("backend_unavailable"))
        .unwrap();
    assert_eq!(
        c.sync_alert(&ghost).unwrap().as_deref(),
        Some("backend_unavailable")
    );
    c.set_sync_alert_within(&ghost, None, Duration::from_millis(50))
        .unwrap();
    assert_eq!(c.sync_alert(&ghost).unwrap(), None);
    assert_eq!(c.scope(&ghost).unwrap(), None, "no scope registered");
    assert_eq!(c.scopes().unwrap(), known, "the registry is unchanged");
}

/// Rows over the limit are refused as they decode, before the store is
/// asked (REQ-BDC-0726, B7Q1-017).
#[test]
fn rows_over_the_limit_are_refused_while_they_decode() {
    let w = World::new();
    let row = serde_json::to_value(lane_bead(&w, "l1")).unwrap();
    let rows: Vec<serde_json::Value> = (0..=MAX_INGEST_ROWS)
        .map(|i| {
            let mut r = row.clone();
            r["id"] = serde_json::json!(format!("l{i}"));
            r
        })
        .collect();
    let json = serde_json::Value::Array(rows).to_string();
    let e = IngestSnapshot::from_json(&lane().id, &json, "[]").unwrap_err();
    assert!(
        matches!(
            e,
            CacheError::IngestInvalid {
                reason: IngestReason::TooManyRows,
                ..
            }
        ),
        "{e:?}"
    );
}
