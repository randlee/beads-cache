//! A repository on the real composition: `SqliteStore` (with its
//! `CacheScheduleStore`), `FlockDrainLock` and `BdCli` over `FakeBd`, with
//! sc-cron's `ManualClock` for the schedule rows, and a stub Python
//! interpreter that speaks sc-cron's hook protocol (Unix).

#![expect(clippy::unwrap_used, reason = "test code")]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use bd_cache_model::{RepoConfig, Scope};
use bd_cache_store::{CacheScheduleStore, FlockDrainLock, SqliteStore};
use bd_cache_sync::{
    Schedules, SharedCache, StopFlag, SyncContext, SyncFiles, compose_with_config_and_backend,
};
use sc_bd_cli::testing::FakeBd;
use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::Clock;
use sc_beads::testing::{FakeBackend, ManualClock};
use sc_cron::{ManualClock as CronClock, Scheduler, SeqJitter};

use super::TempDir;

/// The cache of the real composition.
pub type RealShared = SharedCache<SqliteStore, FlockDrainLock, BdCli>;

/// The scheduler over the cache file's schedule table.
pub type RealScheduler = Scheduler<CacheScheduleStore, CronClock, SeqJitter>;

/// The operations over the real composition on sc-cron's manual clock.
pub type RealSchedules =
    Schedules<CacheScheduleStore, SqliteStore, FlockDrainLock, BdCli, CronClock, SeqJitter>;

/// A repository and its open cache.
pub struct Repo {
    /// The repository root.
    pub dir: TempDir,
    /// bd's fake world.
    pub bd: FakeBackend,
    /// The cache's clock.
    pub clock: Arc<ManualClock>,
    /// sc-cron's clock.
    pub cron_clock: Arc<CronClock>,
    /// The cache.
    pub cache: RealShared,
    /// The schedule table.
    pub store: Arc<CacheScheduleStore>,
    /// The schedule table as the composition hands it to the front ends.
    pub table: bd_cache_sync::ScheduleTable,
    /// The scheduler over it.
    pub scheduler: RealScheduler,
    /// The cache file.
    pub cache_path: PathBuf,
    /// The beads directory.
    pub beads_dir: PathBuf,
    /// Set: every bounded `bd` call answers as an unreachable Dolt server.
    pub down: Arc<AtomicBool>,
}

impl Repo {
    /// A repository whose configuration defines `scopes` (`[[scope]]`-like).
    #[must_use]
    pub fn new(scopes: Vec<Scope>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".beads")).unwrap();
        std::fs::write(
            dir.path().join(".beads/metadata.json"),
            r#"{"dolt_database": "dev", "project_id": "p-1"}"#,
        )
        .unwrap();
        let clock = Arc::new(ManualClock::new(super::START));
        let fake = FakeBd::new(Arc::clone(&clock));
        let bd = fake.backend().clone();
        let (switched, down) = super::real::Switched::new(fake);
        let composed = compose_with_config_and_backend(
            dir.path(),
            RepoConfig::default(),
            |_| BdCli::with_invoker(Box::new(switched)),
            Arc::new(ProjectLocks::new()),
            Arc::clone(&clock) as Arc<dyn Clock>,
            None,
        )
        .unwrap();
        let store = composed.schedule_store().unwrap();
        let table = composed.schedules.clone();
        let cache_path = composed.cache_path.clone();
        let beads_dir = composed.beads_dir.clone();
        let mut cache = composed.cache;
        for scope in scopes {
            cache.scopes_add(&scope).unwrap();
        }
        let cron_clock = Arc::new(CronClock::new(super::START.parse().unwrap()));
        let scheduler = Scheduler::new(
            Arc::clone(&store),
            Arc::clone(&cron_clock),
            Arc::new(SeqJitter::default()),
        );
        Self {
            dir,
            bd,
            clock,
            cron_clock,
            cache: Arc::new(Mutex::new(cache)),
            store,
            table,
            scheduler,
            cache_path,
            beads_dir,
            down,
        }
    }

    /// The files the runs use.
    #[must_use]
    pub fn files(&self) -> SyncFiles {
        SyncFiles {
            cache_path: self.cache_path.clone(),
            beads_dir: Some(self.beads_dir.clone()),
            repo_root: self.dir.path().to_path_buf(),
        }
    }

    /// The sync context of this repository, with fresh flags.
    #[must_use]
    pub fn ctx(&self) -> SyncContext<SqliteStore, FlockDrainLock, BdCli, CronClock> {
        SyncContext::new(
            Arc::clone(&self.cache),
            Arc::clone(&self.cron_clock),
            self.files(),
            StopFlag::new(),
            StopFlag::new(),
        )
    }

    /// The operations over this repository (its own scheduler on the same
    /// table and clocks).
    #[must_use]
    pub fn schedules(&self) -> RealSchedules {
        Schedules::new(
            Arc::clone(&self.store),
            Arc::clone(&self.cron_clock),
            Arc::new(SeqJitter::default()),
            Arc::clone(&self.cache),
            self.files(),
        )
    }

    /// The run context of an on-demand tick.
    #[must_use]
    pub fn run_ctx(&self) -> bd_cache_sync::RunContext {
        bd_cache_sync::RunContext::on_demand(&self.files(), StopFlag::new())
    }

    /// The two rows now.
    #[must_use]
    pub fn rows(&self) -> bd_cache_sync::Rows {
        bd_cache_sync::Rows::read(self.store.as_ref()).unwrap()
    }

    /// The (enabled flag) of each active sync row, `(builtin, python)`.
    #[must_use]
    pub fn flags(&self) -> (Option<bool>, Option<bool>) {
        let rows = bd_cache_sync::Rows::read(self.store.as_ref()).unwrap();
        (
            rows.builtin.map(|r| r.enabled),
            rows.python.map(|r| r.enabled),
        )
    }
}

/// A stub interpreter for the Python row: `-I -c 'import bd_cache_rs'`
/// exits 0 (1 when the file `noimport` exists); `-I -m <module>` reads one
/// request, records it as `<event>.<n>.json` with its environment, and acts
/// on `stop` (the stop answer, default `{"action": "continue"}`) and `tick`
/// (`exit:<n>`, `sleep`, `stderr:<text>`, `grandchild`, `leak`: `$HOME`
/// over more than 8 KiB of stderr, then exit 1). With the file `stopgate`
/// (see [`PyStub::arm_stop_gate`]), `cron.stop` touches `stop.started`, then
/// polls until the test writes something into `stopgate`, for at most 30 s
/// (a plain file, so neither side can block forever); with the file `vanish`, `cron.stop` deletes the
/// interpreter itself, so the run's next launch cannot start.
#[cfg(unix)]
pub struct PyStub {
    /// Its directory.
    pub dir: TempDir,
    /// The interpreter.
    pub python: PathBuf,
}

#[cfg(unix)]
impl PyStub {
    /// A stub whose script succeeds.
    #[must_use]
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let python = dir.path().join("python");
        let body = r#"#!/bin/sh
d="@DIR@"
if [ "$2" = -c ]; then [ -f "$d/noimport" ] && exit 1; exit 0; fi
input=$(cat)
case "$input" in *'"cron.stop"'*) ev=stop ;; *) ev=tick ;; esac
n=$(( $(cat "$d/$ev.count" 2>/dev/null || echo 0) + 1 )); echo $n > "$d/$ev.count"
printf '%s' "$input" > "$d/$ev.$n.json"
env > "$d/$ev.env"
printf '%s\n' "$@" > "$d/$ev.argv"
if [ "$ev" = stop ]; then
  if [ -f "$d/stopgate" ]; then
    : > "$d/stop.started"; i=0
    while [ ! -s "$d/stopgate" ] && [ $i -lt 600 ]; do sleep 0.05; i=$((i+1)); done
  fi
  [ -f "$d/vanish" ] && rm -f "$0"
  if [ -f "$d/stop" ]; then cat "$d/stop"; else echo '{"action": "continue"}'; fi
  exit 0
fi
mode=$(cat "$d/tick" 2>/dev/null)
case "$mode" in
  exit:*) echo "script failed: ${mode#exit:}" >&2; exit "${mode#exit:}" ;;
  sleep) exec sleep 3600 ;;
  stderr:*) printf '%s' "${mode#stderr:}" >&2; exit 0 ;;
  leak) i=0; while [ $i -lt 400 ]; do printf 'leak %s ' "$HOME" >&2; i=$((i+1)); done; exit 1 ;;
  grandchild) exec 3> "$d/life"; ( trap '' TERM; echo up > "$d/ready"; exec sleep 3600 ) & wait; exit 0 ;;
esac
exit 0
"#
        .replace("@DIR@", &dir.path().display().to_string());
        super::script(&python, &body);
        Self { dir, python }
    }

    /// Writes `name` in its directory (`stop`, `tick`, `noimport`).
    pub fn set(&self, name: &str, text: &str) {
        std::fs::write(self.dir.path().join(name), text).unwrap();
    }

    /// Arms the `cron.stop` gate: an empty `stopgate` file. The next
    /// `cron.stop` touches `stop.started` and waits (30 s at most) until the
    /// returned path holds something; release it by writing "go" there.
    #[must_use]
    pub fn arm_stop_gate(&self) -> PathBuf {
        let gate = self.dir.path().join("stopgate");
        std::fs::write(&gate, "").unwrap();
        gate
    }

    /// Removes `name`.
    pub fn unset(&self, name: &str) {
        let _ = std::fs::remove_file(self.dir.path().join(name));
    }

    /// How many `event` calls it saw (`stop` or `tick`).
    #[must_use]
    pub fn count(&self, event: &str) -> usize {
        std::fs::read_to_string(self.dir.path().join(format!("{event}.count")))
            .map_or(0, |s| s.trim().parse().unwrap())
    }

    /// The `n`th request of `event`.
    #[must_use]
    pub fn request(&self, event: &str, n: usize) -> serde_json::Value {
        let text =
            std::fs::read_to_string(self.dir.path().join(format!("{event}.{n}.json"))).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// The environment of the last `event` call.
    #[must_use]
    pub fn env(&self, event: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(format!("{event}.env"))).unwrap()
    }
}
