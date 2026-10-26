//! Scopes are named bd filters (ADR-BDC-0013; REQ-BDC-0721, 0501, 0602, 0603, 0901,
//! 0909 as amended), end to end through the core against `FakeBackend`.
//!
//! `sync_label_filter_scope` and `sync_all_scope` pin the two new variants: the
//! backend is sent the scope's filter, the rows it selects become members, and the
//! ready base is bd's answer intersected with the members. A status criterion is the
//! store's: bd is never sent one, and a bead whose status leaves the set leaves the
//! scope on the next sync that lists it. The registry follows `.beads/bd-cache.toml`
//! (the file wins), and the 0.1 names keep their meaning (REQ-BDC-0812 as amended).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::store;
use bd_cache_model::{
    CacheConfig, CacheError, CacheStore, FilterParts, LEGACY_SCOPE_PREFIX, Scope, ScopeFilter,
    ScopeId,
};
use common::{World, id, sx, x};
use sc_beads::testing::FakeCall;
use sc_beads::{Bead, DepType, ProjectRef};

const T30: Duration = Duration::from_secs(30);

fn scope(name: &str, parts: FilterParts) -> Scope {
    Scope::new(
        ScopeId::new(name).unwrap(),
        ScopeFilter::from_parts(parts).unwrap(),
    )
}

/// A world whose beads carry neutral labels and types, with `scopes` configured.
fn world_with(scopes: Vec<Scope>) -> (World, CacheConfig) {
    let w = World::empty();
    w.add("a1", |b| {
        b.labels = vec!["lane".into()];
        b.issue_type = Some("task".into());
    });
    w.add("a2", |b| {
        b.labels = vec!["lane".into(), "red".into()];
        b.issue_type = Some("bug".into());
    });
    w.add("o1", |b| {
        b.labels = vec!["other".into()];
        b.issue_type = Some("bug".into());
    });
    // a2 is blocked by the bead outside the lane; o1 by a1.
    w.bd.add_edge("a2", "o1", DepType::Blocks);
    w.bd.add_edge("o1", "a1", DepType::Blocks);
    let config = CacheConfig::new(ProjectRef::local(None), scopes);
    (w, config)
}

fn ids(rows: &[bd_cache_model::RowView]) -> Vec<String> {
    rows.iter().map(|r| r.bead.id.to_string()).collect()
}

fn listed_filters(w: &World) -> Vec<sc_beads::ScopeFilter> {
    w.bd.calls()
        .into_iter()
        .filter_map(|c| match c {
            FakeCall::ListScope { filter, .. } => Some(filter),
            _ => None,
        })
        .collect()
}

/// A label-criteria scope syncs the beads its label selects, sends bd the
/// filter, and stores bd's ready and blocked answers for its members only.
#[test]
fn sync_label_filter_scope() {
    let lane = scope(
        "lane",
        FilterParts {
            labels_all: vec!["lane".into()],
            ..FilterParts::default()
        },
    );
    let (w, config) = world_with(vec![lane.clone()]);
    let mut c = w.cache_with(config);
    let r = c.sync(&lane.id, w.after(T30)).unwrap();
    assert_eq!(r.stored, 2);
    assert_eq!(listed_filters(&w), vec![lane.backend_filter()]);
    let list = c.list(&lane.id, None, None).unwrap();
    assert_eq!(ids(&list.rows), ["a1", "a2"]);
    let base = store(&c).ready_base(&lane.id).unwrap().unwrap();
    assert_eq!(base.ready, vec![id("a1")]);
    assert_eq!(
        base.blocked
            .iter()
            .map(|b| b.id.to_string())
            .collect::<Vec<_>>(),
        ["a2"],
        "bd's blocked set restricted to the members"
    );
}

/// An `All` scope syncs every bead with `bd list --all` and holds them all.
#[test]
fn sync_all_scope() {
    let everything = scope(
        "everything",
        FilterParts {
            all: true,
            ..FilterParts::default()
        },
    );
    let (w, config) = world_with(vec![everything.clone()]);
    let mut c = w.cache_with(config);
    c.sync(&everything.id, w.after(T30)).unwrap();
    assert_eq!(listed_filters(&w), vec![sc_beads::ScopeFilter::All]);
    let list = c.list(&everything.id, None, None).unwrap();
    assert_eq!(ids(&list.rows), ["a1", "a2", "o1"]);
    let ready = c.ready(&everything.id, None).unwrap();
    assert_eq!(ids(&ready.ready), ["a1"]);
    let base = store(&c).ready_base(&everything.id).unwrap().unwrap();
    assert_eq!(base.blocked.len(), 2, "a2 and o1 are blocked");
}

/// A type criterion is sent to bd's list and ready, not to `bd blocked`, and the
/// base keeps only members: the blocked bug outside the type is dropped.
#[test]
fn a_type_scope_base_is_intersected_with_its_members() {
    let bugs = scope(
        "bugs",
        FilterParts {
            labels_any: vec!["lane".into(), "other".into()],
            issue_type: Some("task".into()),
            ..FilterParts::default()
        },
    );
    let (w, config) = world_with(vec![bugs.clone()]);
    let mut c = w.cache_with(config);
    c.sync(&bugs.id, w.after(T30)).unwrap();
    assert_eq!(ids(&c.list(&bugs.id, None, None).unwrap().rows), ["a1"]);
    let base = store(&c).ready_base(&bugs.id).unwrap().unwrap();
    assert!(
        base.blocked.is_empty(),
        "bd blocked named a2 and o1, neither a member: {:?}",
        base.blocked
    );
    assert_eq!(base.ready, vec![id("a1")]);
}

/// A status criterion is never sent to bd: the sync lists all seven statuses, and
/// a bead whose status left the set is dropped from the scope on the next sync.
#[test]
fn a_status_criterion_drops_a_bead_whose_status_left_the_set() {
    let open_lane = scope(
        "open-lane",
        FilterParts {
            labels_all: vec!["lane".into()],
            status: vec!["open".into(), "in_progress".into()],
            ..FilterParts::default()
        },
    );
    let (w, config) = world_with(vec![open_lane.clone()]);
    let mut c = w.cache_with(config);
    c.sync(&open_lane.id, w.after(T30)).unwrap();
    assert_eq!(
        ids(&c.list(&open_lane.id, None, None).unwrap().rows),
        ["a1", "a2"]
    );
    let sent = listed_filters(&w);
    assert_eq!(
        sent,
        vec![sc_beads::ScopeFilter::Criteria {
            labels_all: vec!["lane".into()],
            labels_any: vec![],
            issue_type: None,
            assignee: None,
        }],
        "the backend filter carries no status"
    );
    w.clock.advance(Duration::from_secs(60));
    assert!(w.bd.edit("a2", |b: &mut Bead| b.status = Some("closed".into())));
    w.clock.advance(Duration::from_secs(1));
    c.sync(&open_lane.id, w.after(T30)).unwrap();
    assert_eq!(
        ids(&c.list(&open_lane.id, None, None).unwrap().rows),
        ["a1"]
    );
    assert!(
        store(&c).get_row(&id("a2")).unwrap().is_some(),
        "the row stays"
    );
}

/// A sync of an unregistered name registers it on demand as the label scope with
/// that one label (REQ-BDC-0909 as amended); a read of an unknown name is
/// `SCOPE_UNKNOWN` and registers nothing.
#[test]
fn an_unregistered_name_is_registered_on_demand_by_a_sync_only() {
    let (w, config) = world_with(vec![]);
    let mut c = w.cache_with(config);
    let lane = ScopeId::new("lane").unwrap();
    let e = c.list(&lane, None, None).unwrap_err();
    assert!(matches!(e, CacheError::ScopeUnknown { .. }), "{e:?}");
    assert!(
        store(&c).scopes().unwrap().is_empty(),
        "a read registers nothing"
    );
    c.sync(&lane, w.after(T30)).unwrap();
    assert_eq!(
        c.scope(&lane).unwrap(),
        Some(Scope::on_demand(lane.clone()).unwrap())
    );
    assert_eq!(ids(&c.list(&lane, None, None).unwrap().rows), ["a1", "a2"]);
}

/// The configuration file wins: its scopes are written on open (a changed filter
/// updated), and `scopes set` or `scopes remove` of one is `CONFIG_INVALID`.
#[test]
fn a_file_scope_is_written_on_open_and_cannot_be_changed_through_the_registry() {
    let lane = scope(
        "lane",
        FilterParts {
            labels_all: vec!["lane".into()],
            ..FilterParts::default()
        },
    );
    let (w, config) = world_with(vec![lane.clone()]);
    let mut c = w.cache_with(config);
    assert_eq!(c.scope(&lane.id).unwrap(), Some(lane.clone()));
    let wider = scope(
        "lane",
        FilterParts {
            all: true,
            ..FilterParts::default()
        },
    );
    for e in [
        c.scopes_set(&wider).unwrap_err(),
        c.scopes_remove(&lane.id).unwrap_err(),
        c.scopes_add(&wider).unwrap_err(),
    ] {
        assert!(
            matches!(
                &e,
                CacheError::ConfigInvalid {
                    origin: "config_scope",
                    ..
                }
            ),
            "{e:?}"
        );
    }
    // A new open with the file changed updates the registry.
    let config = CacheConfig::new(ProjectRef::local(None), vec![wider.clone()]);
    let c2 = w.cache_with(config);
    assert_eq!(c2.scope(&lane.id).unwrap(), Some(wider));
}

/// Registry scopes are added, set and removed; removing deletes the scope's
/// memberships, not the rows another scope holds (REQ-BDC-0721).
#[test]
fn removing_a_registry_scope_keeps_the_rows_another_scope_holds() {
    let lane = scope(
        "lane",
        FilterParts {
            labels_all: vec!["lane".into()],
            ..FilterParts::default()
        },
    );
    let red = scope(
        "red",
        FilterParts {
            labels_any: vec!["red".into()],
            ..FilterParts::default()
        },
    );
    let (w, config) = world_with(vec![lane.clone()]);
    let mut c = w.cache_with(config);
    c.scopes_add(&red).unwrap();
    c.sync(&lane.id, w.after(T30)).unwrap();
    c.sync(&red.id, w.after(T30)).unwrap();
    assert_eq!(ids(&c.list(&red.id, None, None).unwrap().rows), ["a2"]);
    let report = c.scopes_remove(&red.id).unwrap();
    assert_eq!(report.beads_deleted, 0, "a2 is still held by lane");
    assert_eq!(c.scope(&red.id).unwrap(), None);
    assert_eq!(
        ids(&c.list(&lane.id, None, None).unwrap().rows),
        ["a1", "a2"]
    );
    assert!(matches!(
        c.list(&red.id, None, None).unwrap_err(),
        CacheError::ScopeUnknown { .. }
    ));
}

/// The 0.1 rule (REQ-BDC-0812 as amended): `x` and the prefixed `x` both name the
/// phase-a scope `x`; a 0.1 name never means a filter scope of the same name.
#[test]
fn a_bare_name_on_the_legacy_path_means_the_phase_a_scope_only() {
    let w = World::new();
    let mut c = w.cache();
    let prefixed = format!("{LEGACY_SCOPE_PREFIX}x");
    assert_eq!(c.legacy_scope("x").unwrap(), x());
    assert_eq!(c.legacy_scope(&prefixed).unwrap(), x());
    c.sync_legacy(&x(), w.after(T30)).unwrap();
    assert_eq!(c.list(&x(), None, None).unwrap().rows.len(), 3);
    assert_eq!(c.scope(&x()).unwrap(), Some(sx()));
    // A filter scope named `lane` is not what the 0.1 name `lane` means.
    let lane = scope(
        "lane",
        FilterParts {
            labels_all: vec!["lane".into()],
            ..FilterParts::default()
        },
    );
    c.scopes_add(&lane).unwrap();
    for e in [
        c.legacy_scope("lane").unwrap_err(),
        c.sync_legacy(&lane.id, w.after(T30)).unwrap_err(),
    ] {
        assert!(matches!(e, CacheError::ScopeUnknown { .. }), "{e:?}");
    }
    assert_eq!(
        c.scope(&lane.id).unwrap(),
        Some(lane),
        "nothing re-registered"
    );
}
