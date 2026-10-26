//! Shared fixtures for the bd-cache-sync tests.
//!
//! [`World`] is a sync test world: a `FakeBackend` behind the [`Controlled`]
//! switches, a `FakeStore` and `FakeDrainLock`, and a `ManualClock` at
//! [`START`]. Every part is a shared handle: keep it to drive or inspect the
//! cache. [`script`] writes an executable stub interpreter (Unix).
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
    reason = "test code: a failed unwrap or index is a test failure"
)]

pub mod blocking;
pub mod counting;
pub mod repo;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bd_cache::Cache;
use bd_cache_model::testing::{FakeDrainLock, FakeStore};
use bd_cache_model::{CacheConfig, Scope, ScopeFilter, ScopeId, FilterParts};
use bd_cache_sync::{RunContext, SharedCache, StopFlag};
use blocking::Controlled;
use sc_beads::testing::{FakeBackend, ManualClock, bead};
use sc_beads::{Bead, BeadId, Clock, ProjectRef};

pub use tempfile::TempDir;

/// The cache under test.
pub type TestCache = Cache<FakeStore, FakeDrainLock, Controlled>;

/// Where the [`World`] clock starts.
pub const START: &str = "2026-10-23T10:00:00Z";

/// A bead id.
#[must_use]
pub fn id(s: &str) -> BeadId {
    sc_beads::testing::id(s)
}

/// A scope name.
#[must_use]
pub fn scope(x: &str) -> ScopeId {
    ScopeId::new(x).unwrap()
}

/// A scope selecting the beads that carry `label`.
#[must_use]
pub fn label_scope(name: &str, label: &str) -> Scope {
    Scope::new(
        scope(name),
        ScopeFilter::from_parts(FilterParts {
            labels_all: vec![label.to_owned()],
            ..FilterParts::default()
        })
        .unwrap(),
    )
}

/// The `All` scope.
#[must_use]
pub fn all_scope(name: &str) -> Scope {
    Scope::new(
        scope(name),
        ScopeFilter::from_parts(FilterParts {
            all: true,
            ..FilterParts::default()
        })
        .unwrap(),
    )
}

/// A world: bd, store, lock and clock.
pub struct World {
    /// The clock every part reads.
    pub clock: Arc<ManualClock>,
    /// bd (the fake underneath).
    pub bd: FakeBackend,
    /// The switches in front of bd.
    pub switch: Controlled,
    /// The store.
    pub store: FakeStore,
    /// The drain lock.
    pub lock: FakeDrainLock,
    /// A directory for files the test needs (a cache file stand-in, scripts).
    pub dir: TempDir,
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
        }
    }

    /// Adds a bead with `labels` to bd, stamped with the clock's time.
    pub fn add(&self, name: &str, labels: &[&str], f: impl FnOnce(&mut Bead)) -> Bead {
        let mut b = bead(name);
        b.labels = labels.iter().map(|l| (*l).to_owned()).collect();
        b.updated_at = self.clock.now_utc().as_str().to_owned();
        f(&mut b);
        self.bd.insert(b.clone());
        b
    }

    /// A cache over this world's parts that registers `scopes` on open.
    #[must_use]
    pub fn cache(&self, scopes: Vec<Scope>) -> TestCache {
        Cache::new(
            CacheConfig::new(ProjectRef::local(None), scopes),
            self.store.clone(),
            self.lock.clone(),
            self.switch.clone(),
            Arc::clone(&self.clock) as Arc<dyn Clock>,
        )
        .unwrap()
    }

    /// [`World::cache`] behind the mutex the runs share.
    #[must_use]
    pub fn shared(&self, scopes: Vec<Scope>) -> SharedCache<FakeStore, FakeDrainLock, Controlled> {
        Arc::new(Mutex::new(self.cache(scopes)))
    }

    /// A private file standing in for the cache file (the trust check's subject).
    #[must_use]
    pub fn cache_file(&self) -> PathBuf {
        let path = self.dir.path().join("cache.sqlite3");
        if !path.exists() {
            std::fs::write(&path, b"").unwrap();
            private(&path);
        }
        path
    }

    /// The run context of a built-in run in this world.
    #[must_use]
    pub fn ctx(&self) -> RunContext {
        let stop = StopFlag::new();
        RunContext {
            row: bd_cache_sync::BUILTIN_ROW.to_owned(),
            period: None,
            cache_path: self.cache_file(),
            beads_dir: Some(self.dir.path().to_path_buf()),
            kill: stop.clone(),
            stop,
        }
    }
}

/// Makes `path` private to this user (mode 0600 on Unix).
pub fn private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Writes an executable script through a `sh` child, so this multi-threaded
/// test process never holds a write fd on it that a fork in a concurrent
/// test could inherit and turn the exec into ETXTBSY (os error 26).
#[cfg(unix)]
pub fn script(path: &Path, contents: &str) {
    let status = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg("printf '%s' \"$1\" > \"$2\" && chmod 755 \"$2\"")
        .arg("sh")
        .arg(contents)
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "cannot write {}", path.display());
}

/// Journals `n` closes in `store` and escalates them, as a drain that gave
/// up on them would; returns their seqs, oldest first.
pub fn escalate(world: &World, n: usize) -> Vec<bd_cache_model::Seq> {
    use bd_cache_model::{CacheStore as _, IdemKey, NewEntry, Op};
    let mut store = world.store.clone();
    let at = world.clock.now_utc();
    (0..n)
        .map(|i| {
            let seq = store
                .insert_entry(
                    NewEntry {
                        idem_key: IdemKey::new(format!("k{i}")).unwrap(),
                        scope: None,
                        bead: id(&format!("e{i}")),
                        op: Op::parse("close", "{}").unwrap(),
                        precondition: sc_beads::Guards::default(),
                        actor: sc_beads::testing::actor("tester"),
                        changed: None,
                    },
                    &at,
                )
                .unwrap();
            store.escalate(seq, "BD_CACHE.BD_FAILED: refused", &at).unwrap();
            seq
        })
        .collect()
}

/// A stub of `<python> -I -m bd_cache_rs.run post_sync <module:function>`.
///
/// It records its argv, environment, working directory and each request
/// (`request.<n>.json`) in its directory, then acts on the file `mode`:
/// absent prints a valid answer; `fail` exits 1; `hang` starts a grandchild
/// that holds the named pipe `life`, says `ready` on the named pipe `ready`
/// and sleeps; `flood` writes 2 MiB to stdout; `wrong` answers another protocol.
#[cfg(unix)]
pub struct HookStub {
    /// Its directory.
    pub dir: TempDir,
    /// The stub interpreter.
    pub python: PathBuf,
}

#[cfg(unix)]
impl HookStub {
    /// A stub that answers `ok: true`.
    #[must_use]
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let python = dir.path().join("python");
        let body = r#"#!/bin/sh
d="@DIR@"
[ "$2" = -c ] && exit 0
n=$(( $(cat "$d/count" 2>/dev/null || echo 0) + 1 )); echo $n > "$d/count"
printf '%s\n' "$@" > "$d/argv"
env > "$d/env"
pwd -P > "$d/cwd"
mode=$(cat "$d/mode" 2>/dev/null)
case "$mode" in
  noread) ;;
  *) cat > "$d/request.$n.json" ;;
esac
case "$mode" in
  fail) echo "boom from the hook" >&2; echo '{"protocol":"bd-cache.run.v1","ok":false,"error":{"type":"RuntimeError","message":"boom"}}'; exit 1 ;;
  hang) exec 3> "$d/life"; ( echo up > "$d/ready"; exec sleep 3600 ) & wait; exit 0 ;;
  flood) head -c 2100000 /dev/zero | tr '\0' x; exit 0 ;;
  wrong) echo '{"protocol":"other","ok":true}'; exit 0 ;;
esac
echo '{"protocol":"bd-cache.run.v1","ok":true,"result":{"handled":true}}'
"#
        .replace("@DIR@", &dir.path().display().to_string());
        script(&python, &body);
        Self { dir, python }
    }

    /// Sets the mode for the next calls.
    pub fn mode(&self, mode: &str) {
        std::fs::write(self.dir.path().join("mode"), mode).unwrap();
    }

    /// How many times it ran.
    #[must_use]
    pub fn calls(&self) -> usize {
        std::fs::read_to_string(self.dir.path().join("count"))
            .map(|s| s.trim().parse().unwrap())
            .unwrap_or(0)
    }

    /// The request of call `n` (from 1).
    #[must_use]
    pub fn request(&self, n: usize) -> serde_json::Value {
        let text = std::fs::read_to_string(self.dir.path().join(format!("request.{n}.json"))).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// A file it wrote, as text.
    #[must_use]
    pub fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(name)).unwrap()
    }

    /// Makes the named pipe `name` in its directory.
    pub fn fifo(&self, name: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        let ok = std::process::Command::new("mkfifo").arg(&path).status().unwrap();
        assert!(ok.success());
        path
    }
}

/// Reads the named pipe `path` to its end on a thread of its own: the value
/// arrives when every writer has closed it (a causal gate, never a sleep).
pub fn gate_file(path: PathBuf) -> std::sync::mpsc::Receiver<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Ok(mut f) = std::fs::File::open(path) {
            let _ = std::io::Read::read_to_end(&mut f, &mut Vec::new());
        }
        let _ = tx.send(());
    });
    rx
}
