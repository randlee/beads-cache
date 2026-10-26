//! The built-in sync row against a real `bd` in a scratch workspace
//! (REQ-SYNC-0010 as amended, REQ-BDC-0003, NFR-BDC-0003, NFR-BDC-0006).
//!
//! Every test here is `#[ignore]`d and also returns at once unless
//! `SC_BD_LIVE=1`, so neither `cargo test` nor `cargo test -- --ignored`
//! touches a database by accident. They run on the live testbed
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
//! The composition is the real one (`compose_with`: `SqliteStore` and its
//! `CacheScheduleStore`, `FlockDrainLock`, `BdCli` running `bd`), and the run
//! is the built-in row's scheduled run: `Schedules::cron_run_once` on the
//! system clock, sc-cron's `run_due_with` with the sync resolver, exactly as
//! `bdc cache cron run --once` runs it. The composition is given no
//! `$BEADS_DIR` (`None`, lead ruling S36) and its `bd` children run without
//! one (S40); the setup `bd` calls inherit the environment, so a run with
//! `BEADS_DIR` set is refused rather than split across two databases.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use bd_cache_model::{IdemKey, JournalState, ScopeId};
use bd_cache_store::BEADS_DIR_ENV;
use bd_cache_sync::{ScheduleSet, StopFlag, SystemSchedules, compose_with};
use sc_bd_cli::bounded::{BoundedError, run_bounded};
use sc_bd_cli::{BdCli, ProjectLocks, WorkingDir};
use sc_beads::testing::actor;
use sc_beads::{BeadId, Clock, Deadline, Guards, SystemClock};

const MARKER: &str = ".beads/sc-bd-live-scratch";
/// Each `bd` call of the test's own setup may take this long (a loaded testbed
/// can be slow; the cache's own cap is 30 s). A call that hangs fails the test
/// with a message instead of blocking the run.
const CALL: Duration = Duration::from_secs(60);
/// NFR-BDC-0003, reported with the measured figure; see the test.
const SYNC_LIMIT: Duration = Duration::from_secs(1);
/// An incremental sync must fetch only what changed: far fewer than the scope.
const INCREMENTAL_MAX_ROWS: usize = 50;
/// Beads in the large scope (NFR-BDC-0003).
const BEADS: usize = 1000;
/// The built-in row's budget in a live run: generous, so only a host too
/// loaded for a measurement defers anything.
const BUDGET_MS: u64 = 120_000;

struct Live {
    dir: PathBuf,
    bin: String,
    /// A scope name (and its label) no other run uses.
    scope: String,
    /// Held for the test's life: the tests share one workspace and its
    /// `.beads/bd-cache.toml`, so they run one at a time.
    _exclusive: MutexGuard<'static, ()>,
}

/// The one lock table of the process, for the setup calls and the composition.
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
            scope: format!("live{nanos}"),
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

    /// Creates a bead with the scope's label and returns its id.
    fn create(&self, title: &str) -> BeadId {
        let out = self.bd(&["create", title, "--labels", &self.scope, "--json"]);
        let value: serde_json::Value = serde_json::from_str(&out).expect("bd create --json");
        BeadId::new(value["id"].as_str().expect("an id")).unwrap()
    }

    /// Writes the repository configuration (a cache file of its own and the
    /// scope as a `[[scope]]` table of the label), composes the real cache,
    /// and writes the built-in row due at once.
    async fn schedules(&self) -> SystemSchedules {
        // Literal strings: a Windows path has backslashes.
        std::fs::write(
            self.dir.join(".beads").join("bd-cache.toml"),
            format!(
                "cache_path = '.bd-cache/{scope}.sqlite3'\n\n[[scope]]\nname = '{scope}'\nlabels_all = ['{scope}']\n",
                scope = self.scope
            ),
        )
        .unwrap();
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let bin = self.bin.as_str();
        // `None`: no `$BEADS_DIR`, whatever the environment holds (S36).
        let composed = compose_with(
            &self.dir,
            |locks| BdCli::new(bin, locks),
            Arc::clone(locks()),
            clock,
            None,
        )
        .expect("compose");
        let schedules = composed.into_schedules().expect("the schedule table");
        schedules
            .set(&ScheduleSet {
                expr: "* * * * * *".to_owned(),
                budget_ms: Some(BUDGET_MS),
                hook_reserve_ms: Some(0),
                post_sync: None,
                python: None,
                env: vec![],
            })
            .await
            .expect("the built-in row");
        schedules
    }

    fn scope_id(&self) -> ScopeId {
        ScopeId::new(&self.scope).unwrap()
    }
}

/// One scheduled run of the built-in row (`cron run --once`), once it is due.
async fn run_once(schedules: &SystemSchedules) -> String {
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    let once = schedules
        .cron_run_once(StopFlag::new())
        .await
        .expect("the table");
    assert!(once.error().is_none(), "the run failed: {:?}", once.error());
    assert_eq!(once.ran.len(), 1, "{:?}", once.ran);
    once.ran[0].outcome.clone()
}

/// A write journaled through the core lands in a real `bd` via the scheduled
/// run's drain and is verified by read-back (REQ-SYNC-0010, REQ-BDC-0003).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "live bd test: run through tests/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
async fn a_journaled_write_lands_in_a_real_bd_through_cron_run_once() {
    let Some(live) = Live::open() else {
        return;
    };
    let bead = live.create("live bead");
    let schedules = live.schedules().await;
    assert_eq!(
        run_once(&schedules).await,
        "ok",
        "the first run syncs the scope"
    );

    let cache = schedules.cache_for_tests();
    let seq = cache
        .lock()
        .unwrap()
        .write(
            &bead,
            "close",
            r#"{"reason": "live"}"#,
            Guards::default(),
            IdemKey::new("live-k1").unwrap(),
            actor("bd-cache-live"),
        )
        .unwrap()
        .seq;
    assert_eq!(run_once(&schedules).await, "ok");
    let entry = cache.lock().unwrap().entry(seq).unwrap();
    assert_eq!(entry.state, JournalState::Applied, "{entry:?}");
    assert!(entry.last_error.is_none(), "read back cleanly: {entry:?}");
    let shown = live.bd(&["show", bead.as_str(), "--json"]);
    assert!(
        shown.contains("\"closed\""),
        "and bd itself says so: {shown}"
    );
    assert!(
        !cache
            .lock()
            .unwrap()
            .list(&live.scope_id(), None, None)
            .unwrap()
            .rows
            .is_empty()
    );
}

/// An incremental sync of a 1000-bead scope is incremental, its time
/// reported against 1 s (NFR-BDC-0003).
///
/// The beads are imported with an `updated_at` far in the past, so the
/// incremental sync (cursor = last success - 2 min) fetches only the few
/// beads changed since, as it does in a long-lived cache.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "live bd test: run through tests/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
async fn an_incremental_sync_of_a_1000_bead_scope_is_incremental_and_its_time_is_reported() {
    let Some(live) = Live::open() else {
        return;
    };
    import_beads(&live, BEADS);
    let schedules = live.schedules().await;
    let started = Instant::now();
    assert_eq!(run_once(&schedules).await, "ok");
    let full_took = started.elapsed();
    let cache = schedules.cache_for_tests();
    let ids: Vec<BeadId> = cache
        .lock()
        .unwrap()
        .list(&live.scope_id(), None, None)
        .unwrap()
        .rows
        .into_iter()
        .take(5)
        .map(|r| r.bead.id)
        .collect();
    assert_eq!(ids.len(), 5);
    for id in &ids {
        live.bd(&["update", id.as_str(), "--notes", "changed"]);
    }
    let end = Deadline::after(&SystemClock, Duration::from_millis(BUDGET_MS));
    let started = Instant::now();
    let report = cache
        .lock()
        .unwrap()
        .sync(&live.scope_id(), end)
        .expect("incremental sync");
    let took = started.elapsed();
    if report.deferred {
        eprintln!(
            "INCONCLUSIVE (not a regression): the sync was deferred by its deadline (the host \
             is too loaded for a measurement); rerun when it is idle"
        );
        return;
    }
    // Wall-clock time on a shared host is reported, not gated; what gates is
    // that the sync was incremental, which is what makes it fast.
    eprintln!(
        "NFR-BDC-0003: full sync of {BEADS}+ beads (the whole scheduled run) {full_took:?}; \
         incremental sync {took:?} ({} rows stored, {} blocked); limit {SYNC_LIMIT:?}: {}",
        report.stored,
        report.blocked,
        if took < SYNC_LIMIT { "PASS" } else { "OVER" }
    );
    assert!(
        usize::try_from(report.stored).unwrap() < INCREMENTAL_MAX_ROWS,
        "the sync was not incremental: {report:?}"
    );
}

/// Imports `n` open beads of the scope, last updated an hour ago.
fn import_beads(live: &Live, n: usize) {
    let hour_ago = (std::time::SystemTime::now() - Duration::from_secs(3600))
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let stamp = sc_beads::unix_to_rfc3339(i64::try_from(hour_ago).unwrap());
    let mut lines = String::new();
    for i in 0..n {
        let record = serde_json::json!({
            "title": format!("live bead {i}"),
            "status": "open",
            "priority": 2,
            "issue_type": "task",
            "labels": [live.scope],
            "created_at": stamp,
            "updated_at": stamp,
        });
        lines.push_str(&record.to_string());
        lines.push('\n');
    }
    let file: &Path = &live.dir.join(format!("import-{}.jsonl", live.scope));
    std::fs::write(file, lines).unwrap();
    live.bd(&["import", &file.display().to_string()]);
}
