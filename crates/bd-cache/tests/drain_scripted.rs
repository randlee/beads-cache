//! Drain paths `FakeBackend` cannot script on its own: a pre-read failing
//! with `BD_CONTRACT`, and a read-back that answers without the bead's row
//! (ADR-CORE-0101; REQ-BDC-0403, 0405).
//!
//! [`Scripted`] wraps a `FakeBackend` and answers chosen `list_by_ids` calls
//! itself, outside the fake's lock; every other call goes to the fake.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bd_cache::Cache;
use bd_cache::testing::store;
use bd_cache_model::testing::{FakeDrainLock, FakeStore};
use bd_cache_model::{CacheStore, DrainBudget, IdemKey, JournalState, Seq};
use common::{World, id};
use sc_beads::testing::{FakeBackend, actor};
use sc_beads::{
    Actor, BdRawIssue, Bead, BeadId, BeadsBackend, BeadsCacheError, BeadsError, BlockedRow,
    CacheBackend, Clock, CreatePayload, Deadline, DepType, Fingerprint, Guards, ListQuery, Patch,
    ProjectRef, RelationType, ScopeFilter, SyncCursor, UpdatePayload,
};

/// Answers a `list_by_ids` call instead of the fake (`Some`), or passes it on (`None`).
type ListHook = dyn Fn(&[BeadId]) -> Option<Result<Vec<Bead>, BeadsCacheError>> + Send + Sync;

/// A `FakeBackend` with scripted `list_by_ids` answers.
#[derive(Clone)]
struct Scripted {
    inner: FakeBackend,
    list_hook: Arc<ListHook>,
}

impl BeadsBackend for Scripted {
    fn project_uses_dolt(&self, project: &ProjectRef) -> bool {
        self.inner.project_uses_dolt(project)
    }
    fn list(&self, project: &ProjectRef, query: &ListQuery) -> Result<Vec<BdRawIssue>, BeadsError> {
        self.inner.list(project, query)
    }
    fn ready(&self, project: &ProjectRef) -> Result<Vec<BdRawIssue>, BeadsError> {
        self.inner.ready(project)
    }
    fn status(&self, project: &ProjectRef) -> Result<serde_json::Value, BeadsError> {
        self.inner.status(project)
    }
    fn show(&self, project: &ProjectRef, id: &str) -> Result<Option<BdRawIssue>, BeadsError> {
        self.inner.show(project, id)
    }
    fn create(
        &self,
        project: &ProjectRef,
        payload: &CreatePayload,
    ) -> Result<BdRawIssue, BeadsError> {
        self.inner.create(project, payload)
    }
    fn update(
        &self,
        project: &ProjectRef,
        id: &str,
        updates: &UpdatePayload,
    ) -> Result<Option<BdRawIssue>, BeadsError> {
        self.inner.update(project, id, updates)
    }
    fn close(&self, project: &ProjectRef, id: &str) -> Result<serde_json::Value, BeadsError> {
        self.inner.close(project, id)
    }
    fn search(&self, project: &ProjectRef, query: &str) -> Result<Vec<BdRawIssue>, BeadsError> {
        self.inner.search(project, query)
    }
    fn label_add(&self, project: &ProjectRef, id: &str, label: &str) -> Result<(), BeadsError> {
        self.inner.label_add(project, id, label)
    }
    fn label_remove(&self, project: &ProjectRef, id: &str, label: &str) -> Result<(), BeadsError> {
        self.inner.label_remove(project, id, label)
    }
    fn delete(&self, project: &ProjectRef, id: &str) -> Result<(), BeadsError> {
        self.inner.delete(project, id)
    }
    fn comment_add(&self, project: &ProjectRef, id: &str, content: &str) -> Result<(), BeadsError> {
        self.inner.comment_add(project, id, content)
    }
    fn dep_add(
        &self,
        project: &ProjectRef,
        issue_id: &str,
        depends_on_id: &str,
        relation_type: Option<&str>,
    ) -> Result<(), BeadsError> {
        self.inner
            .dep_add(project, issue_id, depends_on_id, relation_type)
    }
    fn dep_remove(
        &self,
        project: &ProjectRef,
        issue_id: &str,
        depends_on_id: &str,
    ) -> Result<(), BeadsError> {
        self.inner.dep_remove(project, issue_id, depends_on_id)
    }
    fn relation_types(&self) -> Vec<RelationType> {
        self.inner.relation_types()
    }
    fn sync(&self, project: &ProjectRef) -> Result<(), BeadsError> {
        BeadsBackend::sync(&self.inner, project)
    }
}

impl CacheBackend for Scripted {
    fn fingerprint(&self, p: &ProjectRef, d: Deadline) -> Result<Fingerprint, BeadsCacheError> {
        self.inner.fingerprint(p, d)
    }
    fn list_by_ids(
        &self,
        p: &ProjectRef,
        ids: &[BeadId],
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        match (self.list_hook)(ids) {
            Some(answer) => answer,
            None => self.inner.list_by_ids(p, ids, d),
        }
    }
    fn list_scope(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        since: Option<&SyncCursor>,
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        self.inner.list_scope(p, f, since, d)
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

type ScriptedCache = Cache<FakeStore, FakeDrainLock, Scripted>;

/// A cache over the world's parts with `hook` in front of bd's `list_by_ids`.
fn scripted_cache(
    w: &World,
    hook: impl Fn(&[BeadId]) -> Option<Result<Vec<Bead>, BeadsCacheError>> + Send + Sync + 'static,
) -> ScriptedCache {
    let clock: Arc<dyn Clock> = Arc::clone(&w.clock) as Arc<dyn Clock>;
    let backend = Scripted {
        inner: w.bd.clone(),
        list_hook: Arc::new(hook),
    };
    Cache::new(
        w.config.clone(),
        w.store.clone(),
        w.lock.clone(),
        backend,
        clock,
    )
    .unwrap()
}

fn journal(c: &mut ScriptedCache, bead: &str, key: &str) -> Seq {
    c.write(
        &id(bead),
        "close",
        "{}",
        Guards::default(),
        IdemKey::new(key).unwrap(),
        actor("san"),
    )
    .unwrap_or_else(|e| panic!("write {bead}: {e}"))
    .seq
}

fn roomy(w: &World) -> DrainBudget {
    DrainBudget {
        start_gate: w.after(Duration::from_secs(20)),
        hard: w.after(Duration::from_secs(23)),
    }
}

#[test]
fn test_failed_pre_read_retries_every_entry() {
    let w = World::new();
    let armed = Arc::new(AtomicBool::new(false));
    let on = Arc::clone(&armed);
    // Once armed, the batched pre-read (two ids) returns a row bd's model rejects.
    let mut c = scripted_cache(&w, move |ids| {
        (on.load(Ordering::SeqCst) && ids.len() == 2).then(|| {
            Err(BeadsCacheError::Contract {
                bead: Some(id("b2")),
                field: "status".into(),
                detail: "expected a string".into(),
            })
        })
    });
    let e1 = journal(&mut c, "b1", "k1");
    let e2 = journal(&mut c, "b2", "k2");
    armed.store(true, Ordering::SeqCst);
    w.bd.clear_calls();

    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.retrying, vec![e1, e2], "every entry retries");
    assert!(r.applied.is_empty() && r.deferred.is_empty());
    assert_eq!(r.outage, None, "BD_CONTRACT is no outage");
    assert_eq!(store(&c).outage().unwrap(), None);
    assert!(w.bd.writes().is_empty());
    for seq in [e1, e2] {
        let e = store(&c).entry_by_seq(seq).unwrap().unwrap();
        assert_eq!(
            (e.state, e.attempts),
            (JournalState::Pending, 0),
            "no attempt"
        );
    }

    armed.store(false, Ordering::SeqCst);
    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![e1, e2]);
}

#[test]
fn a_read_back_that_answers_without_the_row_is_applied_and_stale() {
    let w = World::new();
    let armed = Arc::new(AtomicBool::new(false));
    let on = Arc::clone(&armed);
    // Once armed, b1's read-back answers with no row; the pre-read lists b1 and b2.
    let mut c = scripted_cache(&w, move |ids| {
        (on.load(Ordering::SeqCst) && *ids == [id("b1")]).then(|| Ok(Vec::new()))
    });
    let e1 = journal(&mut c, "b1", "k1");
    let e2 = journal(&mut c, "b2", "k2");
    armed.store(true, Ordering::SeqCst);

    let r = c.drain(&roomy(&w)).unwrap();
    assert_eq!(r.applied, vec![e1, e2], "the write landed: applied");
    let e = store(&c).entry_by_seq(e1).unwrap().unwrap();
    assert_eq!(e.state, JournalState::Applied);
    assert!(
        e.last_error
            .as_deref()
            .is_some_and(|m| m.starts_with("BD_CACHE.NOT_FOUND")),
        "{:?}",
        e.last_error
    );
    assert_eq!(store(&c).stale_rows().unwrap(), vec![id("b1")]);
    assert_eq!(w.bd_row("b1").status.as_deref(), Some("closed"));
    assert_eq!(
        store(&c).entry_by_seq(e2).unwrap().unwrap().last_error,
        None
    );
}
