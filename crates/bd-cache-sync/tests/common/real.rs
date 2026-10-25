//! The real composition for the end-to-end tests: `SqliteStore` +
//! `FlockDrainLock` (bd-cache-store) behind `Cache`, a `BdCli` (sc-bd-cli)
//! over `FakeBd` (an invoker that parses the argv the real `BdCli` built and
//! applies it to a fake bd; it records every argv), and `SyncJob` run by an
//! `sc-cron` `Scheduler` on a manual clock. Only the `bd` process and the
//! `atm` sender are replaced.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::ffi::OsStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use bd_cache_model::EscalationSender;
use bd_cache_store::{FlockDrainLock, SqliteStore};
use bd_cache_sync::{SyncJob, compose_with, schedule_spec};
use chrono::{DateTime, Utc};
use sc_bd_cli::bounded::BoundedError;
use sc_bd_cli::testing::FakeBd;
use sc_bd_cli::{BdCli, CliInvoker, ProjectLocks};
use sc_beads::error::BeadsError;
use sc_beads::testing::{ManualClock, bead};
use sc_beads::{Bead, CliOutput, CliProbe, Clock, ProjectRef};
use sc_cron::{
    Job, RunOutcome, Schedule, ScheduleStore, Scheduler, SeqJitter, SqliteScheduleStore,
    StopDecision,
};
use tempfile::TempDir;
use tokio::sync::watch;

use super::{FALLBACK, RecordingSender, START, TEAM};

/// What the real composition is made of.
pub type RealJob = SyncJob<SqliteStore, FlockDrainLock, BdCli>;

/// An invoker in front of `FakeBd` that can be switched to "the Dolt server
/// is unreachable" for every cache call.
///
/// It forwards every method it does not change, `set_child_var` included
/// (the forwarding rule of `CliInvoker`, ADR-BDCLI-0004; RSH-036).
#[derive(Clone)]
pub struct Switched {
    inner: FakeBd,
    down: Arc<AtomicBool>,
}

impl CliInvoker for Switched {
    fn binary(&self) -> String {
        self.inner.binary()
    }
    fn client_info(&self) -> Option<CliProbe> {
        self.inner.client_info()
    }
    fn probe(&self) -> Option<CliProbe> {
        self.inner.probe()
    }
    fn run_json(
        &self,
        project: &ProjectRef,
        command: &str,
        args: &[String],
    ) -> Result<String, BeadsError> {
        self.inner.run_json(project, command, args)
    }
    fn run_raw(&self, project: &ProjectRef, args: &[&str]) -> Result<CliOutput, BeadsError> {
        self.inner.run_raw(project, args)
    }
    fn run_bounded(
        &self,
        project: &ProjectRef,
        argv: &[String],
        timeout: Duration,
    ) -> Result<CliOutput, BoundedError> {
        if self.down.load(Ordering::SeqCst) {
            return Ok(CliOutput {
                status: Some(1),
                success: false,
                stdout: String::new(),
                stderr: "Error: failed to open database: Dolt server unreachable at 127.0.0.1:1: \
                         dial tcp 127.0.0.1:1: connect: connection refused"
                    .into(),
            });
        }
        self.inner.run_bounded(project, argv, timeout)
    }
    fn set_child_var(&mut self, key: &OsStr, value: Option<&OsStr>) {
        self.inner.set_child_var(key, value);
    }
}

/// `sc-cron`'s clock on the cache's timeline: wall time and parks come from a
/// manual clock, but monotonic time is the cache clock's. A run's deadline,
/// which the scheduler computes from `monotonic_now`, is therefore on the same
/// timeline as every deadline the cache and the fake bd measure, with one
/// origin (a-7 round 1, FTQ-003, ARCH-011).
#[derive(Debug)]
pub struct Bridge {
    beads: Arc<ManualClock>,
    wall: sc_cron::ManualClock,
}

impl Bridge {
    /// A bridge over `beads`, with wall time starting at [`START`].
    pub fn new(beads: Arc<ManualClock>) -> Self {
        let start: DateTime<Utc> = START.parse().unwrap();
        Self {
            beads,
            wall: sc_cron::ManualClock::new(start),
        }
    }

    pub fn parks(&self) -> u64 {
        self.wall.parks()
    }

    pub async fn wait_parks_above(&self, count: u64) {
        self.wall.wait_parks_above(count).await;
    }

    /// Moves both timelines by `step`, then waits for the worker to settle.
    pub async fn advance_settled(&self, step: chrono::Duration) {
        self.beads.advance(step.to_std().unwrap());
        self.wall.advance_settled(step).await;
    }
}

#[async_trait]
impl sc_cron::Clock for Bridge {
    fn now(&self) -> DateTime<Utc> {
        sc_cron::Clock::now(&self.wall)
    }

    fn monotonic_now(&self) -> std::time::Instant {
        sc_beads::Clock::now_mono(&*self.beads)
    }

    async fn sleep_until(&self, target: DateTime<Utc>) {
        sc_cron::Clock::sleep_until(&self.wall, target).await;
    }
}

/// A job that tells a test when each run ended.
pub struct Spy {
    inner: RealJob,
    outcomes: watch::Sender<Vec<RunOutcome>>,
}

#[async_trait]
impl Job for Spy {
    async fn run(&self, payload: &serde_json::Value, deadline: std::time::Instant) -> RunOutcome {
        let outcome = self.inner.run(payload, deadline).await;
        self.outcomes.send_modify(|o| o.push(outcome.clone()));
        outcome
    }

    async fn should_stop(&self, payload: &serde_json::Value) -> StopDecision {
        self.inner.should_stop(payload).await
    }
}

/// The fixture.
pub struct Real {
    /// The repository root (`.beads/`, `.atm-bd/`, `.bd-cache/`).
    pub root: TempDir,
    /// The clock the cache, the store and the fake bd share.
    pub clock: Arc<ManualClock>,
    /// The fake bd; `fake.calls()` is the recorded argv of every call.
    pub fake: FakeBd,
    /// Switch: bd unreachable.
    pub down: Arc<AtomicBool>,
    /// The sender.
    pub sender: Arc<RecordingSender>,
    /// The job the scheduler runs; `job.with_cache` reaches its cache.
    pub job: RealJob,
    cron_clock: Arc<Bridge>,
    scheduler: Scheduler<SqliteScheduleStore, Bridge, SeqJitter>,
    outcomes: watch::Sender<Vec<RunOutcome>>,
    _cron_dir: TempDir,
}

impl Real {
    /// A repository with phase `x` (lead `lead-x`, root `rx`) and the files
    /// `.beads/metadata.json` and `.beads/bd-cache.toml`.
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let beads = root.path().join(".beads");
        std::fs::create_dir_all(&beads).unwrap();
        std::fs::write(
            beads.join("metadata.json"),
            r#"{"dolt_database": "atm-dev", "project_id": "p-1"}"#,
        )
        .unwrap();
        let phases = root.path().join(".atm-bd");
        std::fs::create_dir_all(&phases).unwrap();
        std::fs::write(
            phases.join("phase-x.toml"),
            "root = \"rx\"\nlead = \"lead-x\"\n",
        )
        .unwrap();
        // Literal strings: a Windows path has backslashes.
        std::fs::write(
            beads.join("bd-cache.toml"),
            format!(
                "scopes = [\"x\"]\n\n[tick]\nteam = \"{TEAM}\"\nfallback_lead = \"{FALLBACK}\"\nphases_dir = '{}'\n",
                phases.display()
            ),
        )
        .unwrap();

        let clock = Arc::new(ManualClock::new(START));
        let fake = FakeBd::new(Arc::clone(&clock));
        let down = Arc::new(AtomicBool::new(false));
        let invoker = Switched {
            inner: fake.clone(),
            down: Arc::clone(&down),
        };
        let beads_clock: Arc<dyn Clock> = Arc::clone(&clock) as Arc<dyn Clock>;
        // The fake runs no `bd`, so it takes no lock set.
        let backend = |_locks| BdCli::with_invoker(Box::new(invoker));
        // `None`: no `$BEADS_DIR`, whatever the environment holds (S36).
        let composed = compose_with(
            root.path(),
            backend,
            Arc::new(ProjectLocks::new()),
            Arc::clone(&beads_clock),
            None,
        )
        .unwrap();
        let sender = Arc::new(RecordingSender::default());
        let spec = schedule_spec(&composed.tick, &composed.name, None).unwrap();
        let job = SyncJob::new(
            composed.cache,
            &composed.tick,
            Arc::clone(&sender) as Arc<dyn EscalationSender>,
        )
        .unwrap();

        let cron_dir = tempfile::tempdir().unwrap();
        let cron_clock = Arc::new(Bridge::new(Arc::clone(&clock)));
        let scheduler = Scheduler::new(
            Arc::new(SqliteScheduleStore::open(cron_dir.path().join("cron.db")).unwrap()),
            Arc::clone(&cron_clock),
            Arc::new(SeqJitter::default()),
        );
        scheduler.start(spec).unwrap();
        let this = Self {
            root,
            clock,
            fake,
            down,
            sender,
            job,
            cron_clock,
            scheduler,
            outcomes: watch::channel(Vec::new()).0,
            _cron_dir: cron_dir,
        };
        this.add("rx", Some("x"), |_| {});
        this
    }

    /// Adds a bead to the fake bd, stamped with the clock's time.
    pub fn add(&self, name: &str, phase: Option<&str>, f: impl FnOnce(&mut Bead)) {
        let mut b = bead(name);
        if let Some(phase) = phase {
            b.labels = vec![format!("phase-{phase}")];
        }
        b.updated_at = sc_beads::Clock::now_utc(&*self.clock).into();
        f(&mut b);
        self.fake.backend().insert(b);
    }

    /// Starts the scheduler's worker; returns its handle.
    pub async fn start(&self) -> sc_cron::WorkerHandle {
        let parks = self.cron_clock.parks();
        let spy = Spy {
            inner: self.job.clone(),
            outcomes: self.outcomes.clone(),
        };
        let handle = self.scheduler.spawn_worker(Arc::new(spy));
        self.cron_clock.wait_parks_above(parks).await;
        handle
    }

    /// Moves both clocks 30 s, lets the scheduler run the tick that falls due,
    /// and returns its outcome.
    pub async fn step(&self) -> RunOutcome {
        let seen = self.outcomes.borrow().len();
        self.cron_clock
            .advance_settled(chrono::Duration::seconds(30))
            .await;
        let mut rx = self.outcomes.subscribe();
        tokio::time::timeout(Duration::from_secs(10), rx.wait_for(|o| o.len() > seen))
            .await
            .expect("the scheduled tick did not finish")
            .unwrap();
        let outcomes = self.outcomes.borrow();
        outcomes[seen].clone()
    }

    /// The scheduler's stored row.
    pub fn schedule(&self) -> Schedule {
        let id = self
            .scheduler
            .store()
            .list_active()
            .unwrap()
            .first()
            .map(|s| s.id)
            .unwrap();
        self.scheduler.store().get(id).unwrap().unwrap()
    }

    /// Stops the worker and waits for its in-flight run to be recorded.
    pub async fn shutdown(&self, handle: sc_cron::WorkerHandle) {
        assert!(
            handle
                .shutdown(tokio::time::Instant::now() + Duration::from_secs(10))
                .await
        );
    }
}
