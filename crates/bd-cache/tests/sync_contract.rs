//! A bad row stores nothing (REQ-BDC-0008).
//!
//! `StrictBackend` wraps `FakeBackend` and passes every row it returns
//! through `sc_beads::parse_bd_rows` as JSON text, exactly as `sc-bd-cli`
//! validates bd's stdout, so a row planted in the fake with a bad field
//! fails the whole call with `BeadsCacheError::Contract`. It can also add a
//! row bd was not asked for, to drive the cache's own checks.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    reason = "test code: a failed unwrap or index is a test failure; w, c, r name the world, cache and reply"
)]

mod common;

use std::error::Error as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bd_cache::Cache;
use bd_cache::testing::store;
use bd_cache_model::testing::{FakeDrainLock, FakeStore};
use bd_cache_model::{CacheError, CacheStore, SyncCause};
use common::{World, id, x};
use sc_beads::testing::FakeBackend;
use sc_beads::{
    Actor, BdRawIssue, Bead, BeadId, BeadsBackend, BeadsCacheError, BeadsError, BlockedRow,
    CacheBackend, Clock, CreatePayload, Deadline, DepType, Fingerprint, Guards, ListQuery, Patch,
    ProjectRef, RelationType, ScopeFilter, SyncCursor, UpdatePayload,
};

/// `FakeBackend` behind bd's strict row validation.
#[derive(Debug, Clone)]
struct StrictBackend {
    inner: FakeBackend,
    /// A row appended to the next list answer (bd misbehaving). `Arc<Mutex>`
    /// because `CacheBackend` is `Send + Sync` with `&self` methods, and the test
    /// keeps a clone to set the row after the cache has taken its own.
    extra: Arc<Mutex<Option<Bead>>>,
}

impl StrictBackend {
    fn new(inner: FakeBackend) -> Self {
        Self {
            inner,
            extra: Arc::new(Mutex::new(None)),
        }
    }

    /// The rows as bd prints them, validated the way `sc-bd-cli` does.
    fn strict(&self, mut rows: Vec<Bead>) -> Result<Vec<Bead>, BeadsCacheError> {
        if let Some(extra) = self.extra.lock().unwrap().take() {
            rows.push(extra);
        }
        // bd prints each dependency with its `created_at`; the cache's `Edge` drops it.
        let printed: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                let mut v = serde_json::to_value(row).unwrap();
                if let Some(deps) = v
                    .get_mut("dependencies")
                    .and_then(serde_json::Value::as_array_mut)
                {
                    for dep in deps.iter_mut().filter_map(serde_json::Value::as_object_mut) {
                        dep.insert("created_at".into(), row.created_at.clone().into());
                    }
                }
                v
            })
            .collect();
        sc_beads::parse_bd_rows(&serde_json::to_string(&printed).unwrap())
    }
}

impl BeadsBackend for StrictBackend {
    fn project_uses_dolt(&self, p: &ProjectRef) -> bool {
        self.inner.project_uses_dolt(p)
    }
    fn list(&self, p: &ProjectRef, q: &ListQuery) -> Result<Vec<BdRawIssue>, BeadsError> {
        self.inner.list(p, q)
    }
    fn ready(&self, p: &ProjectRef) -> Result<Vec<BdRawIssue>, BeadsError> {
        BeadsBackend::ready(&self.inner, p)
    }
    fn status(&self, p: &ProjectRef) -> Result<serde_json::Value, BeadsError> {
        self.inner.status(p)
    }
    fn show(&self, p: &ProjectRef, id: &str) -> Result<Option<BdRawIssue>, BeadsError> {
        self.inner.show(p, id)
    }
    fn create(&self, p: &ProjectRef, c: &CreatePayload) -> Result<BdRawIssue, BeadsError> {
        self.inner.create(p, c)
    }
    fn update(
        &self,
        p: &ProjectRef,
        id: &str,
        u: &UpdatePayload,
    ) -> Result<Option<BdRawIssue>, BeadsError> {
        self.inner.update(p, id, u)
    }
    fn close(&self, p: &ProjectRef, id: &str) -> Result<serde_json::Value, BeadsError> {
        self.inner.close(p, id)
    }
    fn search(&self, p: &ProjectRef, q: &str) -> Result<Vec<BdRawIssue>, BeadsError> {
        self.inner.search(p, q)
    }
    fn label_add(&self, p: &ProjectRef, id: &str, l: &str) -> Result<(), BeadsError> {
        self.inner.label_add(p, id, l)
    }
    fn label_remove(&self, p: &ProjectRef, id: &str, l: &str) -> Result<(), BeadsError> {
        self.inner.label_remove(p, id, l)
    }
    fn delete(&self, p: &ProjectRef, id: &str) -> Result<(), BeadsError> {
        self.inner.delete(p, id)
    }
    fn comment_add(&self, p: &ProjectRef, id: &str, c: &str) -> Result<(), BeadsError> {
        self.inner.comment_add(p, id, c)
    }
    fn dep_add(
        &self,
        p: &ProjectRef,
        id: &str,
        on: &str,
        kind: Option<&str>,
    ) -> Result<(), BeadsError> {
        BeadsBackend::dep_add(&self.inner, p, id, on, kind)
    }
    fn dep_remove(&self, p: &ProjectRef, id: &str, on: &str) -> Result<(), BeadsError> {
        self.inner.dep_remove(p, id, on)
    }
    fn relation_types(&self) -> Vec<RelationType> {
        self.inner.relation_types()
    }
    fn sync(&self, p: &ProjectRef) -> Result<(), BeadsError> {
        self.inner.sync(p)
    }
}

impl CacheBackend for StrictBackend {
    fn fingerprint(&self, p: &ProjectRef, d: Deadline) -> Result<Fingerprint, BeadsCacheError> {
        self.inner.fingerprint(p, d)
    }
    fn list_by_ids(
        &self,
        p: &ProjectRef,
        ids: &[BeadId],
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        self.strict(self.inner.list_by_ids(p, ids, d)?)
    }
    fn list_scope(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        since: Option<&SyncCursor>,
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        self.strict(self.inner.list_scope(p, f, since, d)?)
    }
    fn list_blocked(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BlockedRow>, BeadsCacheError> {
        self.inner.list_blocked(p, f, d)
    }
    fn list_ready(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BeadId>, BeadsCacheError> {
        self.inner.list_ready(p, f, d)
    }
    fn close_guarded(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        reason: Option<&str>,
        g: &Guards,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        self.inner.close_guarded(p, id, reason, g, a, d)
    }
    fn update_guarded(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        patch: &Patch,
        g: &Guards,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        self.inner.update_guarded(p, id, patch, g, a, d)
    }
    fn dep_add_typed(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        on: &BeadId,
        kind: DepType,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        self.inner.dep_add_typed(p, id, on, kind, a, d)
    }
}

type StrictCache = Cache<FakeStore, FakeDrainLock, StrictBackend>;

fn strict_cache(w: &World) -> (StrictCache, StrictBackend) {
    let bd = StrictBackend::new(w.bd.clone());
    let clock: Arc<dyn Clock> = Arc::clone(&w.clock) as Arc<dyn Clock>;
    let c = Cache::new(
        w.config.clone(),
        w.store.clone(),
        w.lock.clone(),
        bd.clone(),
        clock,
    )
    .unwrap();
    (c, bd)
}

/// The backend error at the end of a cache error's source chain.
fn backend_source(e: &CacheError) -> &BeadsCacheError {
    e.source()
        .and_then(|s| s.downcast_ref::<BeadsCacheError>())
        .unwrap_or_else(|| panic!("{e} has no backend source"))
}

#[test]
fn test_contract_violation_is_typed_and_sync_stores_nothing() {
    let w = World::new();
    w.add("b3", |b| b.updated_at = "yesterday".into());
    let (mut c, _) = strict_cache(&w);
    let e = c.sync(&x(), w.after(Duration::from_secs(30))).unwrap_err();
    let CacheError::SyncFailed { scope, cause, .. } = &e else {
        panic!("expected SYNC_FAILED, got {e}");
    };
    assert_eq!(scope.as_ref(), Some(&x()));
    assert_eq!(*cause, Some(SyncCause::BdContract));
    let BeadsCacheError::Contract { bead, field, .. } = backend_source(&e) else {
        panic!("expected a contract source, got {:?}", backend_source(&e));
    };
    assert_eq!(bead.as_ref(), Some(&id("b3")));
    assert_eq!(field, "updated_at");
    assert!(e.to_string().contains("b3"), "{e}");
    assert!(
        store(&c).rows_in_scope(&x()).unwrap().is_empty() && w.store.row_ids().is_empty(),
        "one bad row stores no row of the result"
    );
    let state = store(&c).sync_state(&x()).unwrap();
    assert_eq!(state.last_ok_at, None);
    assert!(
        state
            .last_error
            .as_deref()
            .is_some_and(|t| t.starts_with("BD_CACHE.SYNC_FAILED")),
        "{state:?}"
    );
}

#[test]
fn a_scope_list_row_without_the_scope_label_stores_nothing() {
    let w = World::new();
    let (mut c, bd) = strict_cache(&w);
    *bd.extra.lock().unwrap() = Some(sc_beads::testing::bead("stray"));
    let e = c.sync(&x(), w.after(Duration::from_secs(30))).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.SYNC_FAILED");
    assert!(
        matches!(
            &e,
            CacheError::SyncFailed {
                cause: Some(SyncCause::BdContract),
                ..
            }
        ),
        "{e:?}"
    );
    assert!(
        e.to_string().contains("stray"),
        "{e} / {:?}",
        backend_source(&e)
    );
    assert!(w.store.row_ids().is_empty());
}

#[test]
fn a_bad_row_in_a_refresh_or_miss_fill_stores_nothing() {
    let w = World::new();
    w.add("b3", |b| b.updated_at = "yesterday".into());
    let (mut c, bd) = strict_cache(&w);
    let e = c
        .refresh(&[id("b1"), id("b3")], Duration::from_secs(2))
        .unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BD_CONTRACT");
    assert!(matches!(
        backend_source(&e),
        BeadsCacheError::Contract { field, .. } if field == "updated_at"
    ));
    let e = c.get(&id("b3"), None).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BD_CONTRACT");
    assert!(w.store.row_ids().is_empty(), "nothing stored");

    *bd.extra.lock().unwrap() = Some(w.bd_row("b2"));
    let e = c.refresh(&[id("b1")], Duration::from_secs(2)).unwrap_err();
    assert!(
        matches!(&e, CacheError::BdContract { bead: Some(b), field, .. } if *b == id("b2") && field == "id"),
        "an unrequested bead is a contract violation: {e:?}"
    );
    assert!(w.store.row_ids().is_empty(), "nothing stored");
}

/// `StrictBackend` is a faithful `CacheBackend`: it passes a-1's backend
/// contract suite, so the tests above exercise only the strict validation.
mod strict_backend_contract {
    use std::sync::Arc;

    use sc_beads::testing::contract::{BackendFixture, World};
    use sc_beads::testing::{FakeBackend, ManualClock};

    use super::StrictBackend;

    struct StrictFixture;

    impl BackendFixture for StrictFixture {
        type Backend = StrictBackend;

        fn backend(&mut self, world: &World, clock: Arc<ManualClock>) -> StrictBackend {
            let fake = FakeBackend::new(clock);
            fake.insert_all(world.rows.iter().cloned());
            StrictBackend::new(fake)
        }
    }

    sc_beads::backend_contract_tests!(StrictFixture);
}
