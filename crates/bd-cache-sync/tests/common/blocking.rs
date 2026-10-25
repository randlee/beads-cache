//! [`Controlled`]: a `FakeBackend` with switches.
//!
//! - `down`: every call is unreachable (an outage), until switched off;
//! - `lists_down`: the backend check answers but every list is unreachable
//!   (`bd version` passes while lists time out);
//! - `panicking`: every call panics, as a backend with a bug would, so a
//!   tick can be made to die inside the cache;
//! - `gate`: `list_scope` blocks on a real condition variable until the test
//!   releases it, so a tick can be caught in the middle of a blocking sync.
//!
//! Clones share the switches and the fake.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use sc_beads::testing::FakeBackend;
use sc_beads::{
    Actor, BdRawIssue, Bead, BeadId, BeadsBackend, BeadsCacheError, BeadsError, BlockedRow,
    CacheBackend, CreatePayload, Deadline, DepType, Fingerprint, Guards, ListQuery, Patch,
    ProjectRef, RelationType, ScopeFilter, SyncCursor, UpdatePayload,
};

/// A blocking point: callers wait in [`Gate::pass`] until [`Gate::open`].
#[derive(Debug, Default)]
pub struct Gate {
    state: Mutex<GateState>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct GateState {
    armed: bool,
    entered: usize,
    open: bool,
}

impl Gate {
    /// Makes the next callers wait.
    pub fn arm(&self) {
        let mut s = self.state.lock().unwrap();
        s.armed = true;
        s.open = false;
    }

    /// Lets every waiting caller go and stops waiting new ones.
    pub fn open(&self) {
        let mut s = self.state.lock().unwrap();
        s.open = true;
        s.armed = false;
        self.changed.notify_all();
    }

    /// Waits (up to ten seconds) until `n` callers have entered the gate.
    pub fn wait_entered(&self, n: usize) {
        let s = self.state.lock().unwrap();
        let (s, timeout) = self
            .changed
            .wait_timeout_while(s, Duration::from_secs(10), |s| s.entered < n)
            .unwrap();
        assert!(!timeout.timed_out(), "no caller reached the gate");
        drop(s);
    }

    fn pass(&self) {
        let mut s = self.state.lock().unwrap();
        if !s.armed {
            return;
        }
        s.entered += 1;
        self.changed.notify_all();
        let (s, timeout) = self
            .changed
            .wait_timeout_while(s, Duration::from_secs(30), |s| !s.open)
            .unwrap();
        assert!(!timeout.timed_out(), "the gate was never opened");
        drop(s);
    }
}

/// A callback run at every backend check.
type Hook = Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>;

/// A `FakeBackend` with an outage switch and a blocking gate.
#[derive(Clone)]
pub struct Controlled {
    /// The fake underneath.
    pub inner: FakeBackend,
    down: Arc<AtomicBool>,
    lists_down: Arc<AtomicBool>,
    panicking: Arc<AtomicBool>,
    /// Blocks `list_scope` while armed.
    pub gate: Arc<Gate>,
    on_fingerprint: Hook,
}

impl Controlled {
    /// Wraps `inner`, up and ungated.
    #[must_use]
    pub fn new(inner: FakeBackend) -> Self {
        Self {
            inner,
            down: Arc::new(AtomicBool::new(false)),
            lists_down: Arc::new(AtomicBool::new(false)),
            panicking: Arc::new(AtomicBool::new(false)),
            gate: Arc::new(Gate::default()),
            on_fingerprint: Arc::new(Mutex::new(None)),
        }
    }

    /// Runs `f` at every backend check (the first call of a drain or a sync).
    pub fn on_fingerprint(&self, f: impl Fn() + Send + Sync + 'static) {
        *self.on_fingerprint.lock().unwrap() = Some(Box::new(f));
    }

    /// Every call is unreachable while `down`.
    pub fn set_down(&self, down: bool) {
        self.down.store(down, Ordering::SeqCst);
    }

    /// Every list call is unreachable while `down`; the backend check answers.
    pub fn set_lists_down(&self, down: bool) {
        self.lists_down.store(down, Ordering::SeqCst);
    }

    /// Every call panics while `panicking`.
    pub fn set_panicking(&self, panicking: bool) {
        self.panicking.store(panicking, Ordering::SeqCst);
    }

    fn check(&self, list: bool) -> Result<(), BeadsCacheError> {
        assert!(
            !self.panicking.load(Ordering::SeqCst),
            "the scripted backend panics"
        );
        if self.down.load(Ordering::SeqCst) || (list && self.lists_down.load(Ordering::SeqCst)) {
            return Err(BeadsCacheError::Unavailable {
                reason: "connection refused (scripted)".into(),
                timed_out: false,
            });
        }
        Ok(())
    }
}

impl std::fmt::Debug for Controlled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Controlled").finish_non_exhaustive()
    }
}

impl BeadsBackend for Controlled {
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

impl CacheBackend for Controlled {
    fn fingerprint(&self, p: &ProjectRef, d: Deadline) -> Result<Fingerprint, BeadsCacheError> {
        if let Some(f) = self.on_fingerprint.lock().unwrap().as_ref() {
            f();
        }
        self.check(false)?;
        self.inner.fingerprint(p, d)
    }
    fn list_by_ids(
        &self,
        p: &ProjectRef,
        ids: &[BeadId],
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        self.check(true)?;
        self.inner.list_by_ids(p, ids, d)
    }
    fn list_scope(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        since: Option<&SyncCursor>,
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        self.check(true)?;
        self.gate.pass();
        self.inner.list_scope(p, f, since, d)
    }
    fn list_blocked(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BlockedRow>, BeadsCacheError> {
        self.check(true)?;
        self.inner.list_blocked(p, f, d)
    }
    fn list_ready(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BeadId>, BeadsCacheError> {
        self.check(true)?;
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
        self.check(false)?;
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
        self.check(false)?;
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
        self.check(false)?;
        self.inner.dep_add_typed(p, id, on, kind, a, d)
    }
}
