//! Shared fixtures for bd-cache tests (sprint a-1; used by a-5 and a-6).
//!
//! [`World`] mirrors the Python suite's `world` fixture: a `FakeBackend`
//! holding `b1` (in progress, assigned to `dev`), `b2` and `s1` (which
//! `blocks` on `b1`), all labelled `phase-x`, plus a `FakeStore`, a
//! `FakeDrainLock` and a `ManualClock` starting at `2026-10-23T10:00:00Z`.
//! Every part is a shared handle: keep it to drive or inspect the cache.
//!
//! Include it from a test file with `mod common;`.

#![allow(
    dead_code,
    unreachable_pub,
    reason = "each test file uses a different subset of the shared fixtures"
)]

use std::sync::Arc;
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::testing::{FakeDrainLock, FakeStore};
use bd_cache_model::{CacheConfig, IdemKey, NewEntry, Op, ScopeId};
use sc_beads::testing::{FakeBackend, ManualClock, bead};
use sc_beads::{Actor, Bead, BeadId, Clock, Deadline, DepType, Guards, ProjectRef};

/// The cache under test.
pub type TestCache = Cache<FakeStore, FakeDrainLock, FakeBackend>;

/// The pinned `bd schema` text (the fake's default schema).
pub const SCHEMA_TEXT: &str =
    include_str!("../../../sc-beads/tests/fixtures/bd-1.3.1/bd-schema.json");

/// Where the [`World`] clock starts.
pub const START: &str = "2026-10-23T10:00:00Z";

/// The `phase-x` scope.
#[must_use]
pub fn x() -> ScopeId {
    ScopeId::phase("x")
}

/// A bead id.
#[must_use]
pub fn id(s: &str) -> BeadId {
    BeadId::from(s)
}

/// A world: bd, store, lock and clock, as shared handles.
pub struct World {
    /// The clock every part reads.
    pub clock: Arc<ManualClock>,
    /// bd.
    pub bd: FakeBackend,
    /// The store.
    pub store: FakeStore,
    /// The drain lock.
    pub lock: FakeDrainLock,
    /// The configuration [`World::cache`] uses (scope `x`, defaults otherwise).
    pub config: CacheConfig,
}

impl World {
    /// An empty world.
    #[must_use]
    pub fn empty() -> Self {
        let clock = Arc::new(ManualClock::new(START));
        Self {
            bd: FakeBackend::new(Arc::clone(&clock)),
            clock,
            store: FakeStore::new(),
            lock: FakeDrainLock::new(),
            config: CacheConfig::new(ProjectRef::local(None), vec![x()]),
        }
    }

    /// The Python suite's world: `b1` (in progress, `dev`), `b2`, and `s1` blocked by `b1`.
    #[must_use]
    pub fn new() -> Self {
        let w = Self::empty();
        w.add("b1", |b| {
            b.status = Some("in_progress".into());
            b.assignee = Some("dev".into());
        });
        w.add("b2", |_| {});
        w.add("s1", |_| {});
        w.bd.add_edge("s1", "b1", DepType::Blocks);
        w
    }

    /// Adds a `phase-x` bead to bd, stamped with the clock's time, then moves the clock 1 s.
    pub fn add(&self, name: &str, f: impl FnOnce(&mut Bead)) -> Bead {
        let mut b = bead(name);
        b.labels = vec!["phase-x".into()];
        b.updated_at = self.clock.now_utc();
        f(&mut b);
        self.bd.insert(b.clone());
        self.clock.advance(Duration::from_secs(1));
        b
    }

    /// A cache over this world's parts.
    ///
    /// # Panics
    ///
    /// When the store refuses to register the scopes (a fixture bug).
    #[must_use]
    pub fn cache(&self) -> TestCache {
        self.cache_with(self.config.clone())
    }

    /// A cache over this world's parts with another configuration.
    ///
    /// # Panics
    ///
    /// When the store refuses to register the scopes (a fixture bug).
    #[must_use]
    pub fn cache_with(&self, config: CacheConfig) -> TestCache {
        let clock: Arc<dyn Clock> = Arc::clone(&self.clock) as Arc<dyn Clock>;
        Cache::new(
            config,
            self.store.clone(),
            self.lock.clone(),
            self.bd.clone(),
            clock,
        )
        .unwrap_or_else(|e| panic!("Cache::new: {e}"))
    }

    /// A deadline `d` from now.
    #[must_use]
    pub fn after(&self, d: Duration) -> Deadline {
        Deadline::after(&*self.clock, d)
    }

    /// The current bd row of a bead.
    ///
    /// # Panics
    ///
    /// When bd does not hold the bead.
    #[must_use]
    pub fn bd_row(&self, name: &str) -> Bead {
        self.bd
            .row(name)
            .unwrap_or_else(|| panic!("bd has no {name}"))
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

/// A journal entry for `bead` in scope `x`.
///
/// # Panics
///
/// When `op` / `args` do not parse (a fixture bug).
#[must_use]
pub fn new_entry(key: &str, bead: &str, op: &str, args: &str) -> NewEntry {
    NewEntry {
        idem_key: IdemKey::new(key),
        scope: Some(x()),
        bead: id(bead),
        op: Op::parse(op, args).unwrap_or_else(|e| panic!("{op} {args}: {e}")),
        precondition: Guards::default(),
        actor: Actor::from("san"),
    }
}
