//! A repository on the real composition: `SqliteStore` (with its
//! `CacheScheduleStore`), `FlockDrainLock` and `BdCli` over `FakeBd`, with
//! sc-cron's `ManualClock` for the schedule rows, and a stub Python
//! interpreter that speaks sc-cron's hook protocol (Unix).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bd_cache_model::{RepoConfig, Scope};
use bd_cache_store::{CacheScheduleStore, FlockDrainLock, SqliteStore};
use bd_cache_sync::{SharedCache, StopFlag, SyncContext, compose_with_config_and_backend};
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
    /// The scheduler over it.
    pub scheduler: RealScheduler,
    /// The cache file.
    pub cache_path: PathBuf,
    /// The beads directory.
    pub beads_dir: PathBuf,
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
        let composed = compose_with_config_and_backend(
            dir.path(),
            RepoConfig::default(),
            |_| BdCli::with_invoker(Box::new(fake)),
            Arc::new(ProjectLocks::new()),
            Arc::clone(&clock) as Arc<dyn Clock>,
            None,
        )
        .unwrap();
        let store = composed.schedule_store().unwrap();
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
            scheduler,
            cache_path,
            beads_dir,
        }
    }

    /// The sync context of this repository, with fresh flags.
    #[must_use]
    pub fn ctx(&self) -> SyncContext<SqliteStore, FlockDrainLock, BdCli, CronClock> {
        SyncContext {
            cache: Arc::clone(&self.cache),
            clock: Arc::clone(&self.cron_clock),
            cache_path: self.cache_path.clone(),
            beads_dir: Some(self.beads_dir.clone()),
            stop: StopFlag::new(),
            kill: StopFlag::new(),
        }
    }

    /// The run context of an on-demand tick.
    #[must_use]
    pub fn run_ctx(&self) -> bd_cache_sync::RunContext {
        let stop = StopFlag::new();
        bd_cache_sync::RunContext {
            row: bd_cache_sync::BUILTIN_ROW.to_owned(),
            period: None,
            cache_path: self.cache_path.clone(),
            beads_dir: Some(self.beads_dir.clone()),
            kill: stop.clone(),
            stop,
        }
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
/// (`exit:<n>`, `sleep`, `stderr:<text>`, `grandchild`).
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
  if [ -f "$d/stop" ]; then cat "$d/stop"; else echo '{"action": "continue"}'; fi
  exit 0
fi
mode=$(cat "$d/tick" 2>/dev/null)
case "$mode" in
  exit:*) echo "script failed: ${mode#exit:}" >&2; exit "${mode#exit:}" ;;
  sleep) exec sleep 3600 ;;
  stderr:*) printf '%s' "${mode#stderr:}" >&2; exit 0 ;;
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
