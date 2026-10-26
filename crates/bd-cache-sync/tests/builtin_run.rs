//! The built-in run (REQ-BDC-0703 as amended, 0720, 0722, 0727, 0729; ADR-SYNC-0008):
//! lease, drain, every scope by its filter, the reply. The hook step is in
//! `tests/hook_runner.rs`. Each behaviour has its positive test and a
//! planted negative (AC-N).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::collections::BTreeSet;
use std::time::Duration;

use bd_cache_model::{CacheStore, LeaseHolder, RunId, TickBudget};
use bd_cache_sync::{BuiltinPayload, RunStatus, ScopeOutcome, run_builtin};
use common::{World, all_scope, id, label_scope, scope};
use sc_beads::{Clock, Timestamp};

fn payload() -> BuiltinPayload {
    BuiltinPayload {
        budget: TickBudget::new(Some(20_000), Some(5_000), false).unwrap(),
        hook: None,
    }
}

fn members(world: &World, name: &str) -> BTreeSet<String> {
    world
        .store
        .members(&scope(name))
        .unwrap()
        .into_iter()
        .map(|b| b.as_str().to_owned())
        .collect()
}

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| (*s).to_owned()).collect()
}

/// A label scope and an `All` scope are each synced by their filter, and the
/// lease is free after the run (acceptance 11's sync half, REQ-BDC-0721).
#[tokio::test(flavor = "multi_thread")]
async fn a_run_syncs_a_label_scope_and_an_all_scope_by_their_filters_and_frees_the_lease() {
    let world = World::new();
    world.add("b1", &["area"], |_| {});
    world.add("b2", &["other"], |_| {});
    let cache = world.shared(vec![label_scope("lbl", "area"), all_scope("every")]);
    let reply = run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    assert_eq!(reply.outcome, RunStatus::Ok);
    let results: Vec<_> = reply
        .scopes
        .iter()
        .map(|s| (s.scope.to_string(), s.result))
        .collect();
    assert_eq!(
        results,
        [
            ("every".to_owned(), ScopeOutcome::Ok),
            ("lbl".to_owned(), ScopeOutcome::Ok)
        ]
    );
    assert_eq!(members(&world, "lbl"), set(&["b1"]));
    assert_eq!(members(&world, "every"), set(&["b1", "b2"]));
    assert!(reply.drain.is_some(), "the run drains first");
    assert_eq!(world.store.lease().unwrap(), None, "released");
}

/// A run with no scopes still drains (REQ-BDC-0721).
#[tokio::test(flavor = "multi_thread")]
async fn a_run_with_no_scopes_still_drains() {
    let world = World::new();
    let cache = world.shared(vec![]);
    let reply = run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    assert!(reply.scopes.is_empty());
    assert!(reply.drain.is_some());
}

fn other_holder(world: &World, secs: i64) -> LeaseHolder {
    LeaseHolder {
        run_id: RunId::new("1-999-1").unwrap(),
        pid: 999,
        host: "elsewhere".into(),
        expires_at: Timestamp::from_unix(world.clock.now_utc().unix() + secs).unwrap(),
    }
}

/// A live lease held by another run: the run does nothing and replies
/// `skipped: "in_flight"` (REQ-BDC-0722). Planted negative: once that lease
/// has expired (a holder killed with `kill -9`), it is taken over.
#[tokio::test(flavor = "multi_thread")]
async fn a_live_lease_skips_the_run_and_an_expired_one_is_taken_over() {
    let world = World::new();
    world.add("b1", &["area"], |_| {});
    let mut store = world.store.clone();
    let now = world.clock.now_utc();
    store.lease_take(&other_holder(&world, 30), &now).unwrap();
    let cache = world.shared(vec![label_scope("lbl", "area")]);
    let reply = run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    assert_eq!(
        (reply.outcome, reply.skipped),
        (RunStatus::Skipped, Some("in_flight"))
    );
    assert_eq!(reply.holder.as_ref().unwrap()["pid"], 999);
    assert!(
        reply.drain.is_none() && reply.scopes.is_empty(),
        "nothing ran"
    );
    assert!(members(&world, "lbl").is_empty());

    world.clock.advance(Duration::from_secs(31));
    let reply = run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    assert_eq!(reply.outcome, RunStatus::Ok);
    assert_eq!(reply.took_over.as_ref().unwrap()["pid"], 999);
    assert_eq!(members(&world, "lbl"), set(&["b1"]));
}

/// An unreachable bd gives the scope an `outage` result with
/// `error.outage: true`, its rows stay, and cached reads still answer
/// (ADR-BDC-0005, REQ-BDC-0729). Planted negative: with bd back, `ok`.
#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_bd_is_an_outage_result_and_the_rows_stay() {
    let world = World::new();
    world.add("b1", &["area"], |_| {});
    let cache = world.shared(vec![label_scope("lbl", "area")]);
    run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    world.switch.set_lists_down(true);
    world.clock.advance(Duration::from_secs(60));
    let reply = run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    let r = &reply.scopes[0];
    assert_eq!(r.result, ScopeOutcome::Outage, "{r:?}");
    assert!(r.error.as_ref().unwrap().outage);
    assert!(r.failing_since.is_some(), "an outage is a non-ok result");
    assert_eq!(
        members(&world, "lbl"),
        set(&["b1"]),
        "rows stay as they were"
    );
    assert!(
        cache.lock().unwrap().cached(&id("b1")).unwrap().is_some(),
        "reads answer"
    );

    world.switch.set_lists_down(false);
    world.clock.advance(Duration::from_secs(60));
    let reply = run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    assert_eq!(reply.scopes[0].result, ScopeOutcome::Ok);
    assert!(reply.scopes[0].failing_since.is_none());
}

/// One row the scope's filter does not select, among valid ones, stores no
/// row of that answer and fails the scope (REQ-BDC-0008). Planted negative:
/// without it, the valid rows are stored.
#[tokio::test(flavor = "multi_thread")]
async fn one_invalid_row_fails_the_scope_and_stores_no_row_of_its_answer() {
    let world = World::new();
    world.add("b1", &["area"], |_| {});
    world.add("b2", &["area"], |_| {});
    let mut stray = sc_beads::testing::bead("stray");
    stray.labels = vec!["unrelated".into()];
    world.switch.set_extra_scope_rows(vec![stray]);
    let cache = world.shared(vec![label_scope("lbl", "area")]);
    let reply = run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    let r = &reply.scopes[0];
    assert_eq!(r.result, ScopeOutcome::Failed, "{r:?}");
    assert_eq!(r.error.as_ref().unwrap().code, "BD_CACHE.SYNC_FAILED");
    assert!(
        !r.error.as_ref().unwrap().outage,
        "bd answered: outage false"
    );
    assert!(
        members(&world, "lbl").is_empty(),
        "no row of the answer stored"
    );
    assert!(world.store.get_row(&id("b1")).unwrap().is_none());

    world.switch.set_extra_scope_rows(vec![]);
    let reply = run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    assert_eq!(reply.scopes[0].result, ScopeOutcome::Ok);
    assert_eq!(members(&world, "lbl"), set(&["b1", "b2"]));
}

/// A stored row whose `updated_at` is newer is never replaced by an older
/// incoming one (REQ-BDC-0502). Planted negative: a newer incoming row is stored.
#[tokio::test(flavor = "multi_thread")]
async fn an_older_incoming_row_never_replaces_a_newer_stored_one() {
    let world = World::new();
    let old = world.add("b1", &["area"], |b| b.title = "old".into());
    let cache = world.shared(vec![label_scope("lbl", "area")]);
    let mut newer = old.clone();
    newer.title = "newer".into();
    newer.updated_at =
        Timestamp::from_unix(Timestamp::parse(&old.updated_at).unwrap().unix() + 3600)
            .unwrap()
            .as_str()
            .to_owned();
    world
        .store
        .clone()
        .upsert_rows(None, &[newer], true)
        .unwrap();
    run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    let (row, _) = world.store.get_row(&id("b1")).unwrap().unwrap();
    assert_eq!(row.title, "newer", "the newer stored row stays");

    world.clock.advance(Duration::from_secs(7200));
    let now = world.clock.now_utc();
    world.bd.edit("b1", |b| {
        b.title = "newest".into();
        b.updated_at = now.as_str().to_owned();
    });
    run_builtin(&cache, &payload(), &world.ctx()).await.unwrap();
    let (row, _) = world.store.get_row(&id("b1")).unwrap().unwrap();
    assert_eq!(row.title, "newest");
}

/// A run whose stop flag is set syncs no scope; planted negative above.
#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_run_syncs_no_scope_and_frees_the_lease() {
    let world = World::new();
    world.add("b1", &["area"], |_| {});
    let cache = world.shared(vec![label_scope("lbl", "area")]);
    let ctx = world.ctx();
    ctx.stop.stop();
    let reply = run_builtin(&cache, &payload(), &ctx).await.unwrap();
    assert_eq!(reply.outcome, RunStatus::Stopped);
    assert_eq!(reply.scopes[0].result, ScopeOutcome::Skipped);
    assert!(members(&world, "lbl").is_empty());
    assert_eq!(world.store.lease().unwrap(), None);
}
