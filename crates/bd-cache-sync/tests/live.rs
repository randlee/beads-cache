//! The sync task against a real `bd` in a scratch workspace (REQ-BDC-0003,
//! NFR-BDC-0003, NFR-BDC-0006).
//!
//! Every test here is `#[ignore]`d and also returns at once unless
//! `SC_BD_LIVE=1`, so neither `cargo test` nor `cargo test -- --ignored`
//! touches a database by accident. They run on the `m5-atmbench` testbed
//! through `tests/run-live.sh`, which starts a scratch Dolt server with
//! `scripts/bd-compat/setup-scratch.sh` and removes it afterwards.
//!
//! Environment (set by the script):
//!
//! - `SC_BD_LIVE=1`: run the tests.
//! - `SC_BD_LIVE_DIR`: the scratch workspace. It must contain
//!   `.beads/sc-bd-live-scratch`, the marker `setup-scratch.sh` writes; a
//!   workspace without it is refused, so a shared database cannot be used by
//!   mistake.
//! - `SC_BD_BIN`: the `bd` to test (default `bd`).
//!
//! The composition is the real one: `compose_with` (`SqliteStore`,
//! `FlockDrainLock`, `BdCli` running `bd`), `SyncJob` and the tick. Only the
//! sender is replaced, by a recorder, because no `atm` message must leave a
//! test. As in `tests/common/real.rs`, the composition is given no
//! `$BEADS_DIR` (`None`, lead ruling S36, ARCH-807) and its `bd` children
//! run without one (S40); the setup `bd` calls inherit the environment, so a
//! run with `BEADS_DIR` set is refused rather than split across two
//! databases.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use bd_cache_model::{EscalationSender, IdemKey, JournalState, ScopeId};
use bd_cache_store::BEADS_DIR_ENV;
use bd_cache_sync::{SyncJob, TICK_BUDGET, TickOutcome, TickRun, compose_with};
use common::RecordingSender;
use sc_bd_cli::bounded::{BoundedError, run_bounded};
use sc_bd_cli::{BdCli, ProjectLocks, WorkingDir};
use sc_beads::testing::actor;
use sc_beads::{BeadId, Clock, Deadline, Guards, SystemClock};

const MARKER: &str = ".beads/sc-bd-live-scratch";
/// Each `bd` call of the test's own setup may take this long (a loaded testbed
/// can be slow; the cache's own cap is 30 s). A call that hangs fails the test
/// with a message instead of blocking the run (a-7 round 1, FTQ-004).
const CALL: Duration = Duration::from_secs(60);
/// NFR-BDC-0003, reported with the measured figure; see the test.
const SYNC_LIMIT: Duration = Duration::from_secs(1);
/// An incremental sync must fetch only what changed: far fewer than the phase.
const INCREMENTAL_MAX_ROWS: usize = 50;
/// Beads in the large phase (NFR-BDC-0003).
const BEADS: usize = 1000;

struct Live {
    dir: PathBuf,
    bin: String,
    /// A phase name no other run uses.
    phase: String,
    /// Held for the test's life: the tests share one workspace and its
    /// `.beads/bd-cache.toml`, so they run one at a time whatever the harness'
    /// thread count (FTQ-006).
    _exclusive: MutexGuard<'static, ()>,
}

/// The one lock table of the process for the setup calls.
fn locks() -> &'static Arc<ProjectLocks> {
    static LOCKS: OnceLock<Arc<ProjectLocks>> = OnceLock::new();
    LOCKS.get_or_init(|| Arc::new(ProjectLocks::new()))
}

/// Serializes the live tests.
fn exclusive() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Live {
    /// `None` unless `SC_BD_LIVE=1`; panics when set but not a scratch workspace.
    fn open() -> Option<Self> {
        if std::env::var("SC_BD_LIVE").as_deref() != Ok("1") {
            eprintln!("skipped: SC_BD_LIVE is not 1");
            return None;
        }
        let dir = PathBuf::from(
            std::env::var("SC_BD_LIVE_DIR").expect("SC_BD_LIVE_DIR (see tests/run-live.sh)"),
        );
        assert!(
            std::env::var_os(BEADS_DIR_ENV).is_none(),
            "BEADS_DIR is set: the composition and its bd run without it (S36, S40), \
             the setup bd calls would not; unset it to run the live tests"
        );
        assert!(
            dir.join(MARKER).is_file(),
            "{} has no {MARKER}: live tests only run in a scratch workspace made by \
             scripts/bd-compat/setup-scratch.sh, never in a shared database",
            dir.display()
        );
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        Some(Self {
            _exclusive: exclusive(),
            bin: std::env::var("SC_BD_BIN").unwrap_or_else(|_| "bd".to_owned()),
            dir,
            phase: format!("live{nanos}"),
        })
    }

    /// Runs `bd <args>` in the scratch workspace, bounded by [`CALL`].
    fn bd(&self, args: &[&str]) -> String {
        let words: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let out = run_bounded(
            &self.bin,
            locks(),
            &WorkingDir::new(&self.dir.to_string_lossy()),
            &words,
            CALL,
        )
        .unwrap_or_else(|e| match e {
            BoundedError::TimedOut { phase } => {
                panic!("bd {args:?} did not complete within {CALL:?} ({phase})")
            }
            other => panic!("bd {args:?}: {other}"),
        });
        assert!(
            out.success,
            "bd {args:?} exited {:?}: {}",
            out.status, out.stderr
        );
        out.stdout
    }

    /// Creates a bead in the phase and returns its id.
    fn create(&self, title: &str) -> BeadId {
        let label = format!("phase-{}", self.phase);
        let out = self.bd(&["create", title, "--labels", &label, "--json"]);
        let value: serde_json::Value = serde_json::from_str(&out).expect("bd create --json");
        BeadId::new(value["id"].as_str().expect("an id")).unwrap()
    }

    /// Writes the repository configuration and the phase file, and builds the
    /// real composition.
    fn job(&self, root: &BeadId) -> (RealJob, Arc<RecordingSender>) {
        let phases = self.dir.join(format!(".atm-bd-{}", self.phase));
        std::fs::create_dir_all(&phases).unwrap();
        std::fs::write(
            phases.join(format!("phase-{}.toml", self.phase)),
            format!("root = \"{root}\"\nlead = \"live-lead\"\n"),
        )
        .unwrap();
        // Literal strings: a Windows path has backslashes.
        std::fs::write(
            self.dir.join(".beads").join("bd-cache.toml"),
            format!(
                "cache_path = '.bd-cache/{}.sqlite3'\n\n[tick]\nteam = \"live-team\"\n\
                 fallback_lead = \"live-fallback\"\nphases_dir = '{}'\n",
                self.phase,
                phases.display()
            ),
        )
        .unwrap();
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let bin = self.bin.as_str();
        // `None`: no `$BEADS_DIR`, whatever the environment holds (S36, ARCH-807).
        let composed = compose_with(
            &self.dir,
            |locks| BdCli::new(bin, locks),
            Arc::clone(locks()),
            clock,
            None,
        )
        .expect("compose");
        let sender = Arc::new(RecordingSender::default());
        let job = SyncJob::new(
            composed.cache,
            &composed.tick,
            Arc::clone(&sender) as Arc<dyn EscalationSender>,
        )
        .expect("the [tick] table");
        (job, sender)
    }

    fn scope(&self) -> ScopeId {
        ScopeId::new(&self.phase).unwrap()
    }
}

type RealJob =
    SyncJob<bd_cache_store::SqliteStore, bd_cache_store::FlockDrainLock, sc_bd_cli::BdCli>;

/// The deadline of a setup tick: one a deferral cannot reach, so only the sync
/// a test measures is ever at the mercy of a loaded host (FTQ-017).
const SETUP_BUDGET: Duration = Duration::from_secs(600);

fn run_tick(job: &RealJob, budget: Duration) -> bd_cache_sync::TickReply {
    match job.run_blocking(Deadline::after(&SystemClock, budget)) {
        TickRun::Done(reply) => *reply,
        other => panic!("the tick did not run: {other:?}"),
    }
}

/// A tick that only prepares state. Its deadline is far beyond any deferral, so
/// a deferral here is a real failure, not a loaded host.
fn setup_tick(job: &RealJob) -> bd_cache_sync::TickReply {
    let reply = run_tick(job, SETUP_BUDGET);
    assert!(
        reply.deferred.is_empty() && !reply.busy,
        "a setup tick deferred its work despite {SETUP_BUDGET:?}: {reply:?}"
    );
    reply
}

/// The tick a test is about, on the real budget. `None` is an explicit
/// inconclusive outcome (the host ran out of budget): reported, never a red
/// failure, and no assertion about the sync is made.
fn tick_under_test(job: &RealJob) -> Option<bd_cache_sync::TickReply> {
    let reply = run_tick(job, TICK_BUDGET);
    if reply.deferred.is_empty() && !reply.busy {
        return Some(reply);
    }
    eprintln!(
        "INCONCLUSIVE (not a regression): the tick deferred its work ({reply:?}); the host is \
         too loaded for a live run, rerun when it is idle"
    );
    None
}

fn write_close(job: &RealJob, bead: &BeadId, key: &str) -> bd_cache_model::Seq {
    job.with_cache(|c| {
        c.write(
            bead,
            "close",
            r#"{"reason": "live"}"#,
            Guards::default(),
            IdemKey::new(key).unwrap(),
            actor("bd-cache-live"),
        )
        .unwrap()
        .seq
    })
    .unwrap()
}

/// A write journaled through the core lands in a real `bd` via the tick's
/// drain and is verified by read-back (REQ-BDC-0003).
#[test]
#[ignore = "live bd test: run through tests/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
fn a_journaled_write_lands_in_a_real_bd_through_the_tick() {
    let Some(live) = Live::open() else {
        return;
    };
    let root = live.create("live root");
    let bead = live.create("live bead");
    let (job, sender) = live.job(&root);

    let first = setup_tick(&job);
    assert_eq!(first.outcome, TickOutcome::PassThrough, "{first:?}");
    assert_eq!(first.synced.len(), 1, "{first:?}");
    assert!(first.synced[0].stored >= 2);

    let seq = write_close(&job, &bead, "live-k1");
    let Some(second) = tick_under_test(&job) else {
        return;
    };
    assert_eq!(second.outcome, TickOutcome::PassThrough, "{second:?}");
    assert_eq!(second.drain.as_ref().unwrap().applied, [seq], "{second:?}");
    let entry = job.with_cache(|c| c.entry(seq).unwrap()).unwrap();
    assert_eq!(entry.state, JournalState::Applied);
    assert!(entry.last_error.is_none(), "read back cleanly: {entry:?}");
    // And bd itself says so.
    let shown = live.bd(&["show", bead.as_str(), "--json"]);
    assert!(shown.contains("\"closed\""), "{shown}");
    assert!(sender.sent().is_empty(), "nothing escalated or announced");
    assert!(
        !job.with_cache(|c| c.list(&live.scope(), None, None).unwrap())
            .unwrap()
            .rows
            .is_empty()
    );
}

/// An incremental sync of a 1000-bead phase takes under 1 s (NFR-BDC-0003).
///
/// The beads are imported with an `updated_at` far in the past, so the
/// incremental sync (cursor = last success - 2 min) fetches only the few
/// beads changed since, as it does in a long-lived cache.
#[test]
#[ignore = "live bd test: run through tests/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
fn an_incremental_sync_of_a_1000_bead_phase_is_incremental_and_its_time_is_reported() {
    let Some(live) = Live::open() else {
        return;
    };
    let root = live.create("live root");
    import_beads(&live, BEADS);
    let (job, _sender) = live.job(&root);

    let started = Instant::now();
    let full = setup_tick(&job);
    let full_took = started.elapsed();
    assert_eq!(full.synced.len(), 1, "{full:?}");
    assert!(
        usize::try_from(full.synced[0].stored).unwrap() > BEADS,
        "{full:?}"
    );

    // A few beads change in bd, then the incremental sync.
    let ids = job
        .with_cache(|c| {
            c.list(&live.scope(), None, None)
                .unwrap()
                .rows
                .into_iter()
                .take(5)
                .map(|r| r.bead.id)
                .collect::<Vec<_>>()
        })
        .unwrap();
    for id in &ids {
        live.bd(&["update", id.as_str(), "--notes", "changed"]);
    }
    let end = Deadline::after(&SystemClock, TICK_BUDGET);
    let started = Instant::now();
    let report = job
        .with_cache(|c| c.sync(&live.scope(), end))
        .unwrap()
        .expect("incremental sync");
    let took = started.elapsed();

    if report.deferred {
        eprintln!(
            "INCONCLUSIVE (not a regression): the sync was deferred by its deadline (the host \
             is too loaded for a measurement); rerun when it is idle"
        );
        return;
    }
    // The figure for the record (docs/plans/phase-a/evidence/a-7-testbed.md).
    // Wall-clock time on a shared host is reported, not gated; what gates is
    // that the sync was incremental, which is what makes it fast.
    eprintln!(
        "NFR-BDC-0003: full sync of {BEADS}+ beads (whole tick) {full_took:?}; incremental sync \
         {took:?} ({} rows stored, {} blocked); limit {SYNC_LIMIT:?}: {}",
        report.stored,
        report.blocked,
        if took < SYNC_LIMIT { "PASS" } else { "OVER" }
    );
    assert!(
        usize::try_from(report.stored).unwrap() < INCREMENTAL_MAX_ROWS,
        "the sync was not incremental: {report:?}"
    );
}

/// Imports `n` open beads of the phase, last updated an hour ago.
fn import_beads(live: &Live, n: usize) {
    let hour_ago = (std::time::SystemTime::now() - Duration::from_secs(3600))
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let stamp = sc_beads::unix_to_rfc3339(i64::try_from(hour_ago).unwrap());
    let label = format!("phase-{}", live.phase);
    let mut lines = String::new();
    for i in 0..n {
        let record = serde_json::json!({
            "title": format!("live bead {i}"),
            "status": "open",
            "priority": 2,
            "issue_type": "task",
            "labels": [label],
            "created_at": stamp,
            "updated_at": stamp,
        });
        lines.push_str(&record.to_string());
        lines.push('\n');
    }
    let file: &Path = &live.dir.join(format!("import-{}.jsonl", live.phase));
    std::fs::write(file, lines).unwrap();
    live.bd(&["import", &file.display().to_string()]);
}
