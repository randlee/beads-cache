//! `sync`: incremental rows, the newer-row guard, stale re-reads, sync state,
//! outages and cuts, on-demand scopes and the ready base (REQ-BDC-0501..0504,
//! 0904, 0909; ADR-BDC-0002, 0005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "test code: a failed unwrap or index is a test failure; w, c, r name the world, cache and reply"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::{store, store_mut};
use bd_cache_model::{CacheError, CacheStore, LEGACY_SCOPE_PREFIX, ReadyBasis, ScopeId, SyncCause};
use common::{TestCache, World, id, new_entry, ts, x};
use sc_beads::testing::{FakeCall, FakeOp, Injection};
use sc_beads::{CheckStep, Clock, DepType};

const T30: Duration = Duration::from_secs(30);

fn synced(w: &World) -> TestCache {
    let mut c = w.cache();
    c.sync(&x(), w.after(T30)).unwrap();
    c
}

fn ready_ids(c: &mut TestCache) -> Vec<String> {
    c.ready(&x(), None)
        .unwrap()
        .ready
        .iter()
        .map(|r| r.bead.id.to_string())
        .collect()
}

fn scope_lists(w: &World) -> Vec<Option<String>> {
    w.bd.calls()
        .into_iter()
        .filter_map(|c| match c {
            FakeCall::ListScope { since, .. } => Some(since),
            _ => None,
        })
        .collect()
}

#[test]
fn test_sync_is_incremental_and_never_replaces_a_newer_row() {
    let w = World::new();
    let mut c = w.cache();
    let start = w.clock.now_utc();
    let r = c.sync(&x(), w.after(T30)).unwrap();
    assert_eq!((r.stored, r.deferred, r.outage.clone()), (3, false, None));
    assert_eq!(
        store(&c).sync_state(&x()).unwrap().last_ok_at,
        Some(start.clone()),
        "last_ok_at is the sync's start time"
    );

    // The cache holds a newer b1 (a drain read-back) than the row bd lists.
    let mut newer = w.bd_row("b1");
    newer.title = "read back".into();
    newer.updated_at = "2026-10-23T11:00:00Z".into();
    store_mut(&mut c)
        .upsert_rows(None, std::slice::from_ref(&newer), true)
        .unwrap();
    w.clock.advance(Duration::from_secs(600));
    assert!(w.bd.edit("b2", |b| b.title = "edited in bd".into()));

    c.sync(&x(), w.after(T30)).unwrap();
    assert_eq!(
        scope_lists(&w),
        vec![None, Some("2026-10-23T09:58:03Z".to_owned())],
        "a full list first, then from last_ok_at - 2 min"
    );
    let rows = c.list(&x(), None, None).unwrap().rows;
    let title = |name: &str| {
        rows.iter()
            .find(|r| r.bead.id == id(name))
            .map(|r| r.bead.title.clone())
            .unwrap()
    };
    assert_eq!(title("b1"), "read back", "the newer stored row is kept");
    assert_eq!(title("b2"), "edited in bd");
}

#[test]
fn test_sync_failure_is_typed_and_recorded() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    let e = c.sync(&x(), w.after(T30)).unwrap_err();
    let CacheError::SyncFailed { scope, cause, .. } = &e else {
        panic!("expected SYNC_FAILED, got {e:?}");
    };
    assert_eq!(
        (scope.clone(), *cause),
        (Some(x()), Some(SyncCause::BdListFailed))
    );
    assert!(
        std::error::Error::source(&e).is_some(),
        "keeps the backend error"
    );
    let st = store(&c).sync_state(&x()).unwrap();
    assert_eq!(st.last_ok_at, None);
    assert!(st.last_started_at.is_some());
    assert_eq!(st.last_error.as_deref(), Some(e.to_string().as_str()));
    assert_eq!(
        store(&c).outage().unwrap(),
        None,
        "a reachable backend is no outage"
    );

    w.bd.inject(
        FakeOp::Fingerprint,
        Injection::CheckFailed {
            step: CheckStep::Schema,
            code: 2,
        },
    );
    let e = c.sync(&x(), w.after(T30)).unwrap_err();
    assert!(
        matches!(
            e,
            CacheError::SyncFailed {
                cause: Some(SyncCause::BdSchemaFailed),
                ..
            }
        ),
        "{e:?}"
    );

    c.sync(&x(), w.after(T30)).unwrap();
    let st = store(&c).sync_state(&x()).unwrap();
    assert_eq!(st.last_error, None, "a passing sync clears the error");
    assert!(st.last_ok_at.is_some());
}

#[test]
fn test_sync_records_an_outage_and_a_passing_sync_clears_it() {
    let w = World::new();
    let mut c = w.cache();
    c.set_sync_alert(&x(), Some("BD_CACHE.BD_LIST_FAILED"))
        .unwrap();
    w.bd.inject(
        FakeOp::ListScope,
        Injection::Unavailable { timed_out: false },
    );
    let since = w.clock.now_utc();
    let r = c.sync(&x(), w.after(T30)).unwrap();
    let outage = r.outage.clone().expect("the outage is reported");
    assert_eq!(outage.since, since.clone());
    assert_eq!(store(&c).outage().unwrap(), Some(outage.clone()));
    // The outage is the scope's result (REQ-BDC-0729): its error, not a
    // SYNC_FAILED, and the failure state (REQ-BDC-0728).
    let st = store(&c).sync_state(&x()).unwrap();
    let recorded = st.last_error.clone().unwrap_or_default();
    assert!(
        recorded.starts_with("BD_CACHE.BD_UNAVAILABLE"),
        "an outage records the outage's error, not SYNC_FAILED: {recorded}"
    );
    assert_eq!(st.last_ok_at, None);
    let failing = st.failing_since.clone().expect("failing_since is set");
    assert!(!r.deferred);

    // Only the check answers: the outage holds and keeps its start.
    w.clock.advance(Duration::from_secs(30));
    w.bd.inject(
        FakeOp::ListScope,
        Injection::Unavailable { timed_out: true },
    );
    let r = c.sync(&x(), w.after(T30)).unwrap();
    assert_eq!(r.outage.map(|o| o.since), Some(since.clone()));
    assert_eq!(
        store(&c).sync_state(&x()).unwrap().failing_since,
        Some(failing),
        "kept across consecutive non-ok results"
    );

    let r = c.sync(&x(), w.after(T30)).unwrap();
    assert_eq!(r.outage, None);
    let st = store(&c).sync_state(&x()).unwrap();
    assert_eq!(
        (st.last_error, st.failing_since),
        (None, None),
        "an ok clears both"
    );
    assert_eq!(
        store(&c).outage().unwrap(),
        None,
        "a list that answers clears it"
    );
    assert_eq!(
        c.sync_alert(&x()).unwrap().as_deref(),
        Some("BD_CACHE.BD_LIST_FAILED"),
        "sync never touches the alert"
    );
}

#[test]
fn an_unreachable_backend_check_is_an_outage_too() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.inject(
        FakeOp::Fingerprint,
        Injection::Unavailable { timed_out: false },
    );
    let r = c.sync(&x(), w.after(T30)).unwrap();
    assert!(r.outage.is_some());
    let st = store(&c).sync_state(&x()).unwrap();
    assert!(
        st.last_error
            .as_deref()
            .is_some_and(|e| e.starts_with("BD_CACHE.BD_UNAVAILABLE")),
        "{st:?}"
    );
    assert!(st.failing_since.is_some());
    assert_eq!(scope_lists(&w), Vec::<Option<String>>::new());
}

#[test]
fn a_cut_sync_reports_deferred_and_changes_no_alert() {
    let w = World::new();
    let mut c = w.cache();
    c.set_sync_alert(&x(), Some("backend_unavailable")).unwrap();
    w.bd.inject(FakeOp::ListScope, Injection::Delay(Duration::from_secs(5)));
    let before = w.clock.elapsed();
    let r = c.sync(&x(), w.after(Duration::from_secs(1))).unwrap();
    assert!(r.deferred);
    assert_eq!(r.outage, None, "a cut is not an outage");
    assert_eq!(
        w.clock.elapsed().saturating_sub(before),
        Duration::from_secs(1)
    );
    let st = store(&c).sync_state(&x()).unwrap();
    assert_eq!(
        (st.last_ok_at, st.last_error),
        (None, None),
        "no SYNC_FAILED"
    );
    assert_eq!(store(&c).outage().unwrap(), None);
    assert_eq!(
        c.sync_alert(&x()).unwrap().as_deref(),
        Some("backend_unavailable")
    );
    assert!(w.store.row_ids().is_empty());

    // A deadline already gone: nothing starts.
    w.bd.clear_calls();
    let r = c.sync(&x(), w.after(Duration::ZERO)).unwrap();
    assert!(r.deferred);
    assert!(w.bd.calls().is_empty());
}

#[test]
fn a_stale_row_is_reread_by_the_next_sync_and_its_held_entries_become_drainable() {
    let w = World::new();
    let mut c = synced(&w);
    w.clock.advance(Duration::from_secs(600));
    c.sync(&x(), w.after(T30)).unwrap();
    // A read-back failed: the entry is applied with the error and b2 is stale;
    // a later entry of b2 is held by the drain until a sync re-reads the row.
    let at = w.clock.now_utc();
    let s = store_mut(&mut c);
    let applied = s
        .insert_entry(new_entry("k1", "b2", "close", "{}"), &at)
        .unwrap();
    let held = s
        .insert_entry(
            new_entry("k2", "b2", "update", r#"{"append_notes": "n"}"#),
            &at,
        )
        .unwrap();
    s.apply_with_readback(applied, None, Some("BD_CACHE.BD_UNAVAILABLE: timeout"), &at)
        .unwrap();
    assert_eq!(s.stale_rows().unwrap(), vec![id("b2")]);
    assert!(c.get(&id("b2"), None).unwrap().row.stale);
    // bd closed it without bumping updated_at past the cursor, so only the
    // stale re-read can see it.
    let mut row = w.bd_row("b2");
    row.status = Some("closed".into());
    w.bd.insert(row);

    w.bd.clear_calls();
    w.clock.advance(Duration::from_secs(30));
    let r = c.sync(&x(), w.after(T30)).unwrap();
    assert_eq!(r.stored, 1, "only the stale row was stored");
    assert!(w.bd.calls().contains(&FakeCall::ListByIds(vec![id("b2")])));
    assert!(
        store(&c).stale_rows().unwrap().is_empty(),
        "the stale mark is cleared"
    );
    let got = c.get(&id("b2"), None).unwrap();
    assert!(!got.row.stale);
    assert_eq!(got.row.bead.status.as_deref(), Some("closed"));
    assert_eq!(
        got.pending, 1,
        "the held entry is still pending, now drainable"
    );
    assert_eq!(c.entry(held).unwrap().state.as_str(), "pending");
}

#[test]
fn sync_then_list_of_an_unconfigured_phase_works() {
    let w = World::new();
    let y = ScopeId::new("y").unwrap();
    let mut b = sc_beads::testing::bead("y1");
    b.labels = vec![format!("{LEGACY_SCOPE_PREFIX}y")];
    w.bd.insert(b);
    let mut c = w.cache();
    let e = c.list(&y, None, None).unwrap_err();
    assert!(matches!(&e, CacheError::ScopeUnknown { known, .. } if *known == vec![x()]));
    // The 0.1 path: a bare `y` is the phase-a scope `y` (REQ-BDC-0812).
    c.sync_legacy(&y, w.after(T30)).unwrap();
    let l = c.list(&y, None, None).unwrap();
    assert_eq!(l.rows.len(), 1);
    assert!(l.last_ok_at.is_some());
    assert!(
        store(&c)
            .scopes()
            .unwrap()
            .contains(&bd_cache_model::Scope::legacy(y.clone())),
        "registered on demand"
    );
    // It survives a new Cache over the same store (on-demand scopes persist).
    assert!(w.cache().list(&y, None, None).is_ok());
}

#[test]
fn a_rebuild_drops_the_cursor_so_the_next_list_is_full() {
    let w = World::new();
    let mut c = synced(&w);
    assert!(c.sync(&x(), w.after(T30)).unwrap().model_current);
    w.bd.set_schema("{\"schema_version\": 2}");
    let r = c.sync(&x(), w.after(T30)).unwrap();
    assert!(r.rebuilt && !r.model_current);
    assert_eq!(r.stored, 3);
    assert_eq!(scope_lists(&w).last(), Some(&None));
}

#[test]
fn each_sync_stores_the_ready_base_fetched_after_the_rows() {
    let w = World::new();
    let mut c = w.cache();
    w.bd.set_cost(Duration::from_secs(1));
    let r = c.sync(&x(), w.after(T30)).unwrap();
    assert_eq!(r.blocked, 1);
    let ops: Vec<FakeOp> = w.bd.calls().iter().map(FakeCall::op).collect();
    assert_eq!(
        ops,
        [
            FakeOp::Fingerprint,
            FakeOp::ListScope,
            FakeOp::ListBlocked,
            FakeOp::ListReady
        ]
    );
    let base = store(&c).ready_base(&x()).unwrap().unwrap();
    // Start 10:00:03; the check and the list took 1 s each.
    assert_eq!(base.fetched_at, ts("2026-10-23T10:00:05Z"));
    assert!(!base.stale);
    assert_eq!(base.blocked.len(), 1);
    assert_eq!(base.blocked[0].id, id("s1"));
}

#[test]
fn a_failed_list_blocked_or_list_ready_keeps_the_previous_base_marked_stale() {
    for op in [FakeOp::ListBlocked, FakeOp::ListReady] {
        let w = World::new();
        let mut c = synced(&w);
        let before = store(&c).ready_base(&x()).unwrap().unwrap();
        w.clock.advance(Duration::from_secs(60));
        assert!(w.bd.edit("b1", |b| b.status = Some("closed".into())));
        w.bd.inject(op, Injection::Failed(1));
        let r = c.sync(&x(), w.after(T30)).unwrap();
        assert_eq!(r.blocked, 0, "{op:?}");
        let after = store(&c).ready_base(&x()).unwrap().unwrap();
        assert!(after.stale, "{op:?}");
        assert_eq!(
            (after.blocked, after.ready, after.fetched_at),
            (before.blocked, before.ready, before.fetched_at.clone()),
            "{op:?}: the previous base is kept"
        );
        let reply = c.ready(&x(), None).unwrap();
        assert_eq!(
            reply.basis,
            ReadyBasis::Cached {
                fetched_at: before.fetched_at,
                stale: true
            }
        );
        // The rows still synced.
        assert!(store(&c).sync_state(&x()).unwrap().last_error.is_none());
    }
}

#[test]
fn after_a_cached_blocker_closes_in_bd_one_sync_makes_its_depender_ready_and_reopen_blocks_it() {
    let w = World::new();
    let mut c = synced(&w);
    assert!(!ready_ids(&mut c).contains(&"s1".to_owned()));
    w.clock.advance(Duration::from_secs(60));
    assert!(w.bd.edit("b1", |b| b.status = Some("closed".into())));
    c.sync(&x(), w.after(T30)).unwrap();
    assert!(ready_ids(&mut c).contains(&"s1".to_owned()));
    w.clock.advance(Duration::from_secs(60));
    assert!(w.bd.edit("b1", |b| b.status = Some("open".into())));
    c.sync(&x(), w.after(T30)).unwrap();
    assert!(!ready_ids(&mut c).contains(&"s1".to_owned()));
    let s1 = c.get(&id("s1"), None).unwrap();
    assert_eq!(s1.row.blocked, Some(vec![id("b1")]));
}

#[test]
fn after_a_blocker_outside_every_scope_closes_in_bd_one_sync_makes_its_depender_ready() {
    let w = World::new();
    w.bd.insert(sc_beads::testing::bead("o1"));
    w.add("s2", |_| {});
    assert!(w.bd.add_edge("s2", "o1", DepType::Blocks));
    let mut c = synced(&w);
    assert!(!ready_ids(&mut c).contains(&"s2".to_owned()));
    assert_eq!(
        c.get(&id("s2"), None).unwrap().row.blocked,
        Some(vec![id("o1")])
    );
    assert!(
        w.store.row_ids().iter().all(|r| *r != id("o1")),
        "o1 is not cached"
    );
    w.clock.advance(Duration::from_secs(60));
    assert!(w.bd.edit("o1", |b| b.status = Some("closed".into())));
    c.sync(&x(), w.after(T30)).unwrap();
    assert!(ready_ids(&mut c).contains(&"s2".to_owned()));
    w.clock.advance(Duration::from_secs(60));
    assert!(w.bd.edit("o1", |b| b.status = Some("open".into())));
    c.sync(&x(), w.after(T30)).unwrap();
    assert!(!ready_ids(&mut c).contains(&"s2".to_owned()));
}

#[test]
fn a_child_of_a_blocked_parent_is_not_ready() {
    let w = World::new();
    w.add("c1", |_| {});
    assert!(w.bd.add_edge("c1", "s1", DepType::ParentChild));
    let mut c = synced(&w);
    let ready = ready_ids(&mut c);
    assert!(!ready.contains(&"c1".to_owned()), "{ready:?}");
    assert!(ready.contains(&"b2".to_owned()), "{ready:?}");
    assert_eq!(
        c.get(&id("c1"), None).unwrap().row.blocked,
        Some(vec![id("s1")])
    );
}
