//! Shared fixtures for the bd-cache-sync tests.
//!
//! [`World`] is the BD-CACHE cron test world: a `FakeBackend`, a `FakeStore`
//! and `FakeDrainLock`, a `ManualClock` at `2026-10-23T10:00:00Z`, a directory
//! of phase files and a [`RecordingSender`] that stands in for `atm send`.
//! Every part is a shared handle: keep it to drive or inspect the cache.
//!
//! Include it from a test file with `mod common;`.

#![allow(
    dead_code,
    unreachable_pub,
    reason = "each test file uses a different subset of the shared fixtures"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unused_self,
    reason = "test code: a failed unwrap or index is a test failure; fixture methods keep a uniform receiver"
)]

pub mod blocking;
pub mod counting;
pub mod real;

use blocking::Controlled;

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::testing::{FakeDrainLock, FakeStore};
use bd_cache_model::{
    CacheConfig, EscalationSender, IdemKey, JournalEntry, ScopeId, SendError, Seq, TickConfig,
};
use bd_cache_sync::{ComposeError, TICK_BUDGET, TickReply, tick};
use sc_beads::testing::{FakeBackend, ManualClock, actor, bead};
use sc_beads::{Bead, BeadId, Clock, Deadline, Guards, ProjectRef};

pub use tempfile::TempDir;

/// The cache under test.
pub type TestCache = Cache<FakeStore, FakeDrainLock, Controlled>;

/// Where the [`World`] clock starts.
pub const START: &str = "2026-10-23T10:00:00Z";
/// The team of the phase leads.
pub const TEAM: &str = "atm-dev";
/// The lead of escalations that belong to no phase.
pub const FALLBACK: &str = "team-lead";

/// A bead id.
#[must_use]
pub fn id(s: &str) -> BeadId {
    sc_beads::testing::id(s)
}

/// The `phase-<x>` scope.
#[must_use]
pub fn scope(x: &str) -> ScopeId {
    ScopeId::phase(x).unwrap()
}

/// One message the sender was asked to deliver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    /// Who it went to.
    pub lead: String,
    /// In which team.
    pub team: String,
    /// What it said.
    pub body: String,
}

/// An [`EscalationSender`] that records what it delivers and can be told to fail.
#[derive(Debug, Default)]
pub struct RecordingSender {
    sent: Mutex<Vec<Sent>>,
    attempts: Mutex<Vec<Sent>>,
    failing: AtomicBool,
}

impl RecordingSender {
    /// Makes every send fail (`true`) or succeed (`false`) from now on.
    pub fn fail(&self, failing: bool) {
        self.failing.store(failing, Ordering::SeqCst);
    }

    /// The messages delivered so far.
    #[must_use]
    pub fn sent(&self) -> Vec<Sent> {
        self.sent.lock().unwrap().clone()
    }

    /// The messages delivered to `lead`.
    #[must_use]
    pub fn sent_to(&self, lead: &str) -> Vec<Sent> {
        self.sent().into_iter().filter(|s| s.lead == lead).collect()
    }

    /// Every send that was attempted, delivered or not.
    #[must_use]
    pub fn attempts(&self) -> Vec<Sent> {
        self.attempts.lock().unwrap().clone()
    }

    /// Forgets the messages recorded so far.
    pub fn clear(&self) {
        self.sent.lock().unwrap().clear();
        self.attempts.lock().unwrap().clear();
    }
}

impl EscalationSender for RecordingSender {
    fn send(&self, lead: &str, team: &str, body: &str) -> Result<(), SendError> {
        let message = Sent {
            lead: lead.to_owned(),
            team: team.to_owned(),
            body: body.to_owned(),
        };
        self.attempts.lock().unwrap().push(message.clone());
        if self.failing.load(Ordering::SeqCst) {
            return Err(SendError::new("atm is down"));
        }
        self.sent.lock().unwrap().push(message);
        Ok(())
    }
}

/// A world: bd, store, lock, clock, phase files and a sender.
pub struct World {
    /// The clock every part reads.
    pub clock: Arc<ManualClock>,
    /// bd (the fake underneath is `bd.inner`).
    pub bd: FakeBackend,
    /// The switches in front of bd.
    pub switch: Controlled,
    /// The store.
    pub store: FakeStore,
    /// The drain lock.
    pub lock: FakeDrainLock,
    /// The phases directory.
    pub dir: TempDir,
    /// The sender.
    pub sender: Arc<RecordingSender>,
}

impl World {
    /// An empty world.
    #[must_use]
    pub fn new() -> Self {
        let clock = Arc::new(ManualClock::new(START));
        let bd = FakeBackend::new(Arc::clone(&clock));
        Self {
            switch: Controlled::new(bd.clone()),
            bd,
            clock,
            store: FakeStore::new(),
            lock: FakeDrainLock::new(),
            dir: tempfile::tempdir().unwrap(),
            sender: Arc::new(RecordingSender::default()),
        }
    }

    /// The `[tick]` table: the team, the fallback lead and this world's phases directory.
    #[must_use]
    pub fn config(&self) -> TickConfig {
        TickConfig {
            team: Some(TEAM.into()),
            fallback_lead: Some(FALLBACK.into()),
            phases_dir: Some(self.dir.path().to_path_buf()),
            phases: None,
        }
    }

    /// Writes `phase-<name>.toml` with the root and lead.
    pub fn phase_file(&self, name: &str, root: &str, lead: Option<&str>) -> PathBuf {
        let path = self.dir.path().join(format!("phase-{name}.toml"));
        let mut text = format!("root = \"{root}\"\n");
        if let Some(lead) = lead {
            writeln!(text, "lead = \"{lead}\"").unwrap();
        }
        std::fs::write(&path, text).unwrap();
        path
    }

    /// Adds a bead labelled `phase-<phase>` to bd, stamped with the clock's time.
    pub fn add(&self, name: &str, phase: Option<&str>, f: impl FnOnce(&mut Bead)) -> Bead {
        let mut b = bead(name);
        if let Some(phase) = phase {
            b.labels = vec![format!("phase-{phase}")];
        }
        b.updated_at = self.clock.now_utc().into();
        f(&mut b);
        self.bd.insert(b.clone());
        b
    }

    /// A phase: its file, and its open root bead `r<name>` labelled with the phase.
    pub fn phase(&self, name: &str, lead: Option<&str>) {
        self.phase_file(name, &format!("r{name}"), lead);
        self.add(&format!("r{name}"), Some(name), |_| {});
    }

    /// Edits a bead in bd as another writer would, bumping its `updated_at`.
    pub fn edit(&self, name: &str, f: impl FnOnce(&mut Bead)) {
        let now = self.clock.now_utc();
        assert!(self.bd.edit(name, |b| {
            f(b);
            b.updated_at = now.into();
        }));
    }

    /// Closes a bead in bd.
    pub fn close_in_bd(&self, name: &str) {
        self.edit(name, |b| b.status = Some("closed".into()));
    }

    /// A cache over this world's parts with no configured scope.
    #[must_use]
    pub fn cache(&self) -> TestCache {
        cache_over(&self.clock, &self.store, &self.lock, &self.switch)
    }

    /// The recompose function of a `SyncJob` over this world: each call builds
    /// a new cache over the same parts (the same composition inputs) and
    /// counts one in `built`.
    pub fn recompose(
        &self,
        built: &Arc<AtomicUsize>,
    ) -> impl Fn() -> Result<TestCache, ComposeError> + Send + Sync + 'static {
        let (clock, store, lock, switch) = (
            Arc::clone(&self.clock),
            self.store.clone(),
            self.lock.clone(),
            self.switch.clone(),
        );
        let built = Arc::clone(built);
        move || {
            built.fetch_add(1, Ordering::SeqCst);
            Ok(cache_over(&clock, &store, &lock, &switch))
        }
    }

    /// A deadline `d` from now.
    #[must_use]
    pub fn after(&self, d: Duration) -> Deadline {
        Deadline::after(&*self.clock, d)
    }

    /// Runs one tick with the world's config and sender, under the 25 s budget.
    pub fn tick(&self, cache: &mut TestCache) -> TickReply {
        self.tick_with(cache, &self.config())
    }

    /// Runs one tick with `config`.
    pub fn tick_with(&self, cache: &mut TestCache, config: &TickConfig) -> TickReply {
        tick(cache, config, &*self.sender, self.after(TICK_BUDGET)).unwrap()
    }

    /// Moves the clock to the next 30 s tick.
    pub fn next_tick(&self) {
        self.clock.advance(Duration::from_secs(30));
    }

    /// Journals a close of `bead` through the core.
    pub fn journal_close(&self, cache: &mut TestCache, bead: &str, key: &str) -> Seq {
        cache
            .write(
                &id(bead),
                "close",
                "{}",
                Guards::default(),
                IdemKey::new(key).unwrap(),
                actor("san"),
            )
            .unwrap()
            .seq
    }

    /// The journal entry with `seq`.
    #[must_use]
    pub fn entry(&self, seq: Seq) -> JournalEntry {
        use bd_cache_model::CacheStore;
        self.store.entry_by_seq(seq).unwrap().unwrap()
    }
}

/// A cache over the given parts with no configured scope.
fn cache_over(
    clock: &Arc<ManualClock>,
    store: &FakeStore,
    lock: &FakeDrainLock,
    switch: &Controlled,
) -> TestCache {
    let clock: Arc<dyn Clock> = Arc::clone(clock) as Arc<dyn Clock>;
    Cache::new(
        CacheConfig::new(ProjectRef::local(None), vec![]),
        store.clone(),
        lock.clone(),
        switch.clone(),
        clock,
    )
    .unwrap()
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

/// A sender that spends `cost` of the world's clock on every message, as a slow
/// `atm` would, and records what it was asked.
pub struct SlowSender {
    clock: Arc<ManualClock>,
    cost: Duration,
    sent: Mutex<Vec<(String, Duration)>>,
}

impl SlowSender {
    /// A sender whose every message takes `cost` on `clock` (or the time it is given, if less).
    pub fn new(clock: &Arc<ManualClock>, cost: Duration) -> Self {
        Self {
            clock: Arc::clone(clock),
            cost,
            sent: Mutex::new(Vec::new()),
        }
    }

    /// Who was sent to, with the time each send was given.
    pub fn sent(&self) -> Vec<(String, Duration)> {
        self.sent.lock().unwrap().clone()
    }
}

impl EscalationSender for SlowSender {
    fn send(&self, lead: &str, team: &str, body: &str) -> Result<(), SendError> {
        self.send_within(lead, team, body, Duration::from_secs(3600))
    }

    fn send_within(
        &self,
        lead: &str,
        _team: &str,
        _body: &str,
        timeout: Duration,
    ) -> Result<(), SendError> {
        self.sent.lock().unwrap().push((lead.to_owned(), timeout));
        if self.cost > timeout {
            self.clock.advance(timeout);
            return Err(SendError::with_kind(
                bd_cache_model::SendErrorKind::TimedOut,
                "the scripted atm was too slow",
            ));
        }
        self.clock.advance(self.cost);
        Ok(())
    }
}
