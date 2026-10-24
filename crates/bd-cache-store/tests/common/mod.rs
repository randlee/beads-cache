//! Shared fixtures for bd-cache-store tests: temporary workspaces, the store
//! contract fixture, and bounded child processes.
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
    reason = "test code: a failed unwrap is a test failure"
)]

use std::io::{BufRead, BufReader, Read, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bd_cache_model::testing::contract::StoreFixture;
use bd_cache_store::{DatabaseIdentity, DoltDatabase, FlockDrainLock, ProjectId, SqliteStore};

/// A directory removed on drop.
#[derive(Debug)]
pub struct TempDir(PathBuf);

impl TempDir {
    /// A fresh, empty directory under the system temp dir.
    pub fn new(tag: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "bdc-store-{tag}-{}-{nanos}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes `<root>/.beads/metadata.json` (the Python suite's `workspace`).
pub fn workspace(root: &Path, database: &str, project: &str) {
    std::fs::create_dir_all(root.join(".beads")).unwrap();
    std::fs::write(
        root.join(".beads").join("metadata.json"),
        format!(r#"{{"dolt_database": "{database}", "project_id": "{project}"}}"#),
    )
    .unwrap();
}

/// The identity `testdb` / `p-1`.
pub fn identity() -> DatabaseIdentity {
    identity_of("testdb", "p-1")
}

pub fn identity_of(database: &str, project: &str) -> DatabaseIdentity {
    DatabaseIdentity {
        dolt_database: DoltDatabase::new(database).unwrap(),
        project_id: ProjectId::new(project).unwrap(),
    }
}

/// The store contract fixture: a fresh cache file per `open`, in its own temp dir.
pub struct SqliteFixture {
    dir: TempDir,
    opened: u32,
    current: Option<PathBuf>,
}

impl SqliteFixture {
    pub fn new() -> Self {
        Self {
            dir: TempDir::new("contract"),
            opened: 0,
            current: None,
        }
    }
}

impl StoreFixture for SqliteFixture {
    type Store = SqliteStore;
    type Lock = FlockDrainLock;

    fn open(&mut self) -> SqliteStore {
        self.opened += 1;
        let path = self
            .dir
            .path()
            .join(format!("c{}", self.opened))
            .join("testdb.bd-cache.sqlite3");
        let store = SqliteStore::open_at(&path, &identity()).unwrap();
        self.current = Some(path);
        store
    }

    fn reopen(&mut self, store: SqliteStore) -> SqliteStore {
        let path = store.path().to_path_buf();
        drop(store);
        SqliteStore::open_at(&path, &identity()).unwrap()
    }

    fn locks(&mut self) -> (FlockDrainLock, FlockDrainLock) {
        let cache = self.dir.path().join("lock.bd-cache.sqlite3");
        (
            FlockDrainLock::for_cache_file(&cache),
            FlockDrainLock::for_cache_file(&cache),
        )
    }
}

/// How long any child process may run before the test fails.
pub const CHILD_TIMEOUT: Duration = Duration::from_secs(180);

/// Runs `cmd` to completion within `timeout`, killing it and failing the test past it.
pub fn run_bounded(cmd: &mut Command, timeout: Duration) -> Output {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot start {cmd:?}: {e}"));
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = wait_bounded(&mut child, timeout, &format!("{cmd:?}"));
    Output {
        status,
        stdout: stdout.recv_timeout(timeout).unwrap_or_default(),
        stderr: stderr.recv_timeout(timeout).unwrap_or_default(),
    }
}

/// Waits for `child` up to `timeout`; kills it and panics past that.
pub fn wait_bounded(child: &mut Child, timeout: Duration, what: &str) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{what} did not finish within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Reads a pipe to the end on its own thread (so a full pipe never blocks the child).
fn drain(pipe: Option<impl Read + Send + 'static>) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut buf);
        }
        let _ = tx.send(buf);
    });
    rx
}

/// A child that holds a resource until its stdin closes.
pub struct Holder {
    child: Child,
    stdin: Option<ChildStdin>,
    what: String,
}

impl Holder {
    /// Spawns `cmd` and waits up to `timeout` for a stdout line containing `marker`, which it returns.
    pub fn spawn(cmd: &mut Command, marker: &'static str, timeout: Duration) -> (Self, String) {
        let what = format!("{cmd:?}");
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("cannot start {what}: {e}"));
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => return,
                    Ok(_) if line.contains(marker) => {
                        let _ = tx.send(line);
                        // Keep reading so the child never blocks on a full pipe.
                        let _ = std::io::copy(&mut reader, &mut std::io::sink());
                        return;
                    }
                    Ok(_) => {}
                }
            }
        });
        let Ok(line) = rx.recv_timeout(timeout) else {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{what} did not print {marker:?} within {timeout:?}");
        };
        (Self { child, stdin, what }, line)
    }

    /// Closes the child's stdin and waits (bounded) for it to exit successfully.
    pub fn release(mut self, timeout: Duration) {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.flush();
        }
        let status = wait_bounded(&mut self.child, timeout, &self.what);
        assert!(status.success(), "{} failed: {status}", self.what);
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        // Only reached without `release` (a failing test): never leave the child behind.
        if self.stdin.is_some() {
            self.stdin = None;
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Reruns this test binary, running only the test `name` (which must be `#[ignore]`d).
pub fn this_test_binary(name: &str) -> Command {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        name,
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd
}
