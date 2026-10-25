//! Shared support for the enabled-flag, stop-hook and Python-hook tests.
//!
//! Time is a `ManualClock`, so a test moves time and never waits on it. A
//! test waits only on a signal: the manual clock's park, a write to the
//! recording store, a job's counters, or, for a Python hook, a named pipe the
//! stub interpreter writes to. The longest real wait, [`WAIT`], bounds only a
//! failing run; a passing run never reaches it.

#![allow(
    dead_code,
    unused_imports,
    reason = "each test crate uses a subset of these helpers"
)]
#![expect(
    clippy::expect_used,
    clippy::clone_on_ref_ptr,
    clippy::unused_self,
    reason = "test code: a failed expectation is the test failure"
)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sc_cron::{
    CronError, Generation, HookTarget, Job, ManualClock, NewSchedule, RunOutcome, Schedule,
    ScheduleId, ScheduleName, ScheduleSpec, ScheduleStore, Scheduler, SeqJitter,
    SqliteScheduleStore, StopDecision, StoreError, StoreErrorKind, Timestamp, WorkerHandle,
};
use serde_json::{Value, json};
use tokio::sync::watch;

/// Longest a failing test waits for a signal.
pub const WAIT: Duration = Duration::from_secs(10);

pub fn at(text: &str) -> DateTime<Utc> {
    text.parse().expect("timestamp")
}

pub fn ts(text: &str) -> Timestamp {
    text.parse().expect("timestamp")
}

pub fn name(text: &str) -> ScheduleName {
    ScheduleName::new(text).expect("name")
}

/// Waits until `condition` holds for the value in `sender`.
pub async fn wait_on<T>(sender: &watch::Sender<T>, what: &str, condition: impl FnMut(&T) -> bool) {
    let mut receiver = sender.subscribe();
    let reached = tokio::time::timeout(WAIT, receiver.wait_for(condition)).await;
    assert!(matches!(reached, Ok(Ok(_))), "timed out waiting for {what}");
}

/// One store write, and whether the store applied it.
#[derive(Clone, Debug, PartialEq)]
pub enum Write {
    Run {
        id: ScheduleId,
        outcome: RunOutcome,
        changed: bool,
    },
    Next {
        id: ScheduleId,
        changed: bool,
    },
    Stop {
        id: ScheduleId,
        reason: String,
        changed: bool,
    },
    Disable {
        id: ScheduleId,
        reason: String,
        changed: bool,
    },
}

/// A store that logs every run-state write after it completes, counts its
/// `list_active` calls, and can fail one `stop` or `disable` call on demand.
pub struct RecordingStore {
    inner: SqliteScheduleStore,
    writes: watch::Sender<Vec<Write>>,
    lists: watch::Sender<usize>,
    failures: watch::Sender<usize>,
    fail_stop: AtomicBool,
    fail_disable: AtomicBool,
}

impl RecordingStore {
    pub fn new() -> Self {
        Self {
            inner: SqliteScheduleStore::open_in_memory().expect("store"),
            writes: watch::Sender::new(Vec::new()),
            lists: watch::Sender::new(0),
            failures: watch::Sender::new(0),
            fail_stop: AtomicBool::new(false),
            fail_disable: AtomicBool::new(false),
        }
    }

    /// Makes the next `stop` call fail with a store error, once.
    pub fn fail_next_stop(&self) {
        self.fail_stop.store(true, Ordering::SeqCst);
    }

    /// Makes the next `disable` call fail with a store error, once.
    pub fn fail_next_disable(&self) {
        self.fail_disable.store(true, Ordering::SeqCst);
    }

    /// How many `list_active` calls the store has answered.
    pub fn lists(&self) -> usize {
        *self.lists.borrow()
    }

    pub async fn wait_failures(&self, count: usize) {
        wait_on(
            &self.failures,
            &format!("{count} planted failures"),
            |seen| *seen >= count,
        )
        .await;
    }

    /// Returns a planted failure if `flag` is set, clearing it.
    fn planted(&self, flag: &AtomicBool, what: &str) -> Result<(), CronError> {
        if flag.swap(false, Ordering::SeqCst) {
            self.failures.send_modify(|seen| *seen += 1);
            return Err(CronError::Store(StoreError::new(
                StoreErrorKind::Other,
                format!("planted {what} failure"),
            )));
        }
        Ok(())
    }

    fn log(&self, write: Write) {
        self.writes.send_modify(|writes| writes.push(write));
    }

    pub fn writes(&self) -> Vec<Write> {
        self.writes.borrow().clone()
    }

    pub async fn wait_write(&self, what: &str, condition: impl Fn(&Write) -> bool) {
        wait_on(&self.writes, what, |writes| writes.iter().any(&condition)).await;
    }

    pub async fn wait_recorded(&self, schedule: &Schedule, outcome: RunOutcome) {
        let id = schedule.id;
        self.wait_write(&format!("{outcome} recorded"), |write| {
            *write
                == Write::Run {
                    id,
                    outcome: outcome.clone(),
                    changed: true,
                }
        })
        .await;
    }

    pub async fn wait_stopped(&self, schedule: &Schedule, reason: &str) {
        let id = schedule.id;
        self.wait_write("schedule stopped", |write| {
            matches!(write, Write::Stop { id: stopped, reason: why, changed: true }
                if *stopped == id && why == reason)
        })
        .await;
    }

    pub async fn wait_disabled(&self, schedule: &Schedule, reason: &str) {
        let id = schedule.id;
        self.wait_write("schedule disabled", |write| {
            matches!(write, Write::Disable { id: disabled, reason: why, changed: true }
                if *disabled == id && why == reason)
        })
        .await;
    }

    pub async fn wait_next_refused(&self, schedule: &Schedule) {
        let id = schedule.id;
        self.wait_write(
            "set_next refused",
            |write| matches!(write, Write::Next { id: refused, changed: false } if *refused == id),
        )
        .await;
    }
}

impl ScheduleStore for RecordingStore {
    fn create(&self, schedule: &NewSchedule, created_at: Timestamp) -> Result<Schedule, CronError> {
        self.inner.create(schedule, created_at)
    }

    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError> {
        self.inner.get(id)
    }

    fn list_active(&self) -> Result<Vec<Schedule>, CronError> {
        let rows = self.inner.list_active();
        self.lists.send_modify(|lists| *lists += 1);
        rows
    }

    fn record_run(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        outcome: &RunOutcome,
    ) -> Result<bool, CronError> {
        let changed = self.inner.record_run(id, generation, at, outcome)?;
        self.log(Write::Run {
            id,
            outcome: outcome.clone(),
            changed,
        });
        Ok(changed)
    }

    fn set_next(
        &self,
        id: ScheduleId,
        generation: Generation,
        next_run_at: Timestamp,
    ) -> Result<bool, CronError> {
        let changed = self.inner.set_next(id, generation, next_run_at)?;
        self.log(Write::Next { id, changed });
        Ok(changed)
    }

    fn stop(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        reason: &str,
    ) -> Result<bool, CronError> {
        self.planted(&self.fail_stop, "stop")?;
        let changed = self.inner.stop(id, generation, at, reason)?;
        self.log(Write::Stop {
            id,
            reason: reason.to_owned(),
            changed,
        });
        Ok(changed)
    }

    fn set_enabled(&self, name: &ScheduleName, enabled: bool) -> Result<bool, CronError> {
        self.inner.set_enabled(name, enabled)
    }

    fn disable(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        reason: &str,
    ) -> Result<bool, CronError> {
        self.planted(&self.fail_disable, "disable")?;
        let changed = self.inner.disable(id, generation, at, reason)?;
        self.log(Write::Disable {
            id,
            reason: reason.to_owned(),
            changed,
        });
        Ok(changed)
    }
}

/// What the stop check of a [`ScriptedJob`] does on one call.
pub enum StopScript {
    Decide(StopDecision),
    /// Never answer.
    Hang,
}

/// A Rust job with a scripted stop check, that counts its calls.
pub struct ScriptedJob {
    stops: Mutex<VecDeque<StopScript>>,
    stop_checks: watch::Sender<usize>,
    runs: watch::Sender<usize>,
    run_deadlines: Mutex<Vec<Instant>>,
}

impl ScriptedJob {
    pub fn new() -> Self {
        Self {
            stops: Mutex::new(VecDeque::new()),
            stop_checks: watch::Sender::new(0),
            runs: watch::Sender::new(0),
            run_deadlines: Mutex::new(Vec::new()),
        }
    }

    /// Queues the answer of the next stop check; later checks continue.
    pub fn then_stop(&self, script: StopScript) {
        self.stops.lock().expect("stops").push_back(script);
    }

    pub fn stop_checks(&self) -> usize {
        *self.stop_checks.borrow()
    }

    pub async fn wait_stop_checks(&self, count: usize) {
        wait_on(
            &self.stop_checks,
            &format!("{count} stop checks"),
            |checks| *checks >= count,
        )
        .await;
    }

    pub fn runs(&self) -> usize {
        *self.runs.borrow()
    }

    pub fn deadlines(&self) -> Vec<Instant> {
        self.run_deadlines.lock().expect("deadlines").clone()
    }

    pub async fn wait_runs(&self, count: usize) {
        wait_on(&self.runs, &format!("{count} runs"), |runs| *runs >= count).await;
    }
}

#[async_trait]
impl Job for ScriptedJob {
    async fn run(&self, _payload: &Value, deadline: Instant) -> RunOutcome {
        self.run_deadlines.lock().expect("deadlines").push(deadline);
        self.runs.send_modify(|runs| *runs += 1);
        RunOutcome::Ok
    }

    async fn should_stop(&self, _payload: &Value) -> StopDecision {
        self.stop_checks.send_modify(|checks| *checks += 1);
        let script = self.stops.lock().expect("stops").pop_front();
        match script {
            None => StopDecision::Continue,
            Some(StopScript::Decide(decision)) => decision,
            Some(StopScript::Hang) => std::future::pending().await,
        }
    }
}

pub type TestScheduler = Scheduler<RecordingStore, ManualClock, SeqJitter>;

/// A scheduler on a manual clock over a recording in-memory store.
pub struct Fixture {
    pub store: Arc<RecordingStore>,
    pub clock: Arc<ManualClock>,
    pub scheduler: TestScheduler,
}

impl Fixture {
    pub fn new() -> Self {
        Self::build(|scheduler| scheduler)
    }

    /// A fixture whose worker reloads every `interval` while a disabled
    /// schedule exists.
    pub fn with_reload_poll(interval: Duration) -> Self {
        Self::build(|scheduler| scheduler.with_reload_poll(interval))
    }

    fn build(configure: impl FnOnce(TestScheduler) -> TestScheduler) -> Self {
        let store = Arc::new(RecordingStore::new());
        let clock = Arc::new(ManualClock::new(at("2026-10-19T00:00:00Z")));
        let scheduler = configure(Scheduler::new(
            store.clone(),
            clock.clone(),
            Arc::new(SeqJitter::default()),
        ));
        Self {
            store,
            clock,
            scheduler,
        }
    }

    /// A spec that runs every minute at :00 with a 20 s timeout.
    pub fn spec(&self, name: &str) -> ScheduleSpec {
        ScheduleSpec::new(
            name,
            "0 * * * * *",
            Duration::from_secs(20),
            json!({"name": name, "n": 1}),
        )
        .expect("spec")
    }

    pub fn start(&self, spec: ScheduleSpec) -> Schedule {
        self.scheduler.start(spec).expect("start")
    }

    /// Spawns the worker for `target` and waits until it has settled.
    pub async fn worker(&self, target: HookTarget) -> WorkerHandle {
        let parks = self.clock.parks();
        let handle = self.scheduler.spawn_worker_for(target);
        self.clock.wait_parks_above(parks).await;
        handle
    }

    /// Moves the clock `seconds` and waits until the worker has seen it.
    pub async fn step(&self, seconds: i64) {
        self.clock
            .advance_settled(chrono::Duration::seconds(seconds))
            .await;
    }

    pub fn row(&self, schedule: &Schedule) -> Schedule {
        self.store
            .get(schedule.id)
            .expect("get")
            .expect("row exists")
    }
}

pub async fn shutdown(handle: WorkerHandle) {
    let stopped = handle.shutdown(tokio::time::Instant::now() + WAIT).await;
    assert!(stopped, "worker stopped before the deadline");
}

#[cfg(unix)]
pub use stub::{Call, Stub, read_pipe, wait_pipe};

/// A stub Python interpreter: a shell script that records how it was called
/// and behaves as the test says, so no test needs Python.
#[cfg(unix)]
mod stub {
    use std::fs;
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::mpsc;

    use sc_cron::PythonHook;
    use tempfile::TempDir;

    use super::WAIT;

    /// The script. `argv`, `cwd` and `stdin` of call N land in `calls/N/`.
    /// What it does next is read from `<event>.mode`, `<event>.out` and
    /// `<event>.exit`, where `<event>` is `stop` or `tick`:
    ///
    /// - mode `hang`: signal `ready` (a named pipe, if the test made one)
    ///   and sleep for an hour;
    /// - mode `fork`: open the named pipe `life` for writing on fd 3 and start
    ///   a background process that inherits it, signals `ready` and sleeps. The
    ///   signal comes last and nothing forks after it, so every process of the
    ///   hook exists when the test hears it, and `life` reaches end of file
    ///   exactly when all of them are dead;
    /// - mode `flood`: write 200000 bytes to stdout and to stderr;
    /// - otherwise print `<event>.out` and exit with `<event>.exit` (default 0).
    const SCRIPT: &str = r#"#!/bin/sh
dir='@DIR@'
n=1
while [ -d "$dir/calls/$n" ]; do n=$((n + 1)); done
call="$dir/calls/$n"
mkdir -p "$call"
printf '%s\n' "$@" > "$call/argv"
pwd -P > "$call/cwd"
cat > "$call/stdin"
event=$(sed -n 's/.*"event":"cron\.\([a-z]*\)".*/\1/p' "$call/stdin")
mode=$(cat "$dir/$event.mode" 2>/dev/null)
signal() { if [ -p "$dir/ready" ]; then echo up > "$dir/ready"; fi; }
case "$mode" in
  hang) signal; exec sleep 3600 ;;
  fork) exec 3> "$dir/life"; ( signal; exec sleep 3600 ) & wait; exit 0 ;;
  flood) head -c 200000 /dev/zero | tr '\0' x; head -c 200000 /dev/zero | tr '\0' y >&2 ;;
esac
if [ -f "$dir/$event.out" ]; then cat "$dir/$event.out"; fi
if [ -f "$dir/$event.exit" ]; then exit "$(cat "$dir/$event.exit")"; fi
exit 0
"#;

    /// One recorded call of the stub.
    #[derive(Debug)]
    pub struct Call {
        pub argv: Vec<String>,
        pub cwd: PathBuf,
        pub stdin: String,
    }

    pub struct Stub {
        dir: TempDir,
        interpreter: PathBuf,
    }

    impl Stub {
        pub fn new() -> Self {
            let dir = tempfile::Builder::new()
                .prefix("sc-cron-stub-")
                .tempdir()
                .expect("stub dir");
            let interpreter = dir.path().join("python");
            let script = SCRIPT.replace("@DIR@", &dir.path().display().to_string());
            fs::write(&interpreter, script).expect("write stub");
            fs::set_permissions(&interpreter, fs::Permissions::from_mode(0o755)).expect("chmod");
            Self { dir, interpreter }
        }

        pub fn dir(&self) -> &Path {
            self.dir.path()
        }

        pub fn hook(&self, module: &str) -> PythonHook {
            PythonHook::new(&self.interpreter, module).expect("hook")
        }

        fn put(&self, file: &str, content: &str) {
            fs::write(self.dir.path().join(file), content).expect("write stub file");
        }

        /// What the hook prints for `event` (`stop` or `tick`).
        pub fn reply(&self, event: &str, stdout: &str) {
            self.put(&format!("{event}.out"), stdout);
        }

        /// The exit code of the hook for `event`.
        pub fn exit(&self, event: &str, code: i32) {
            self.put(&format!("{event}.exit"), &code.to_string());
        }

        /// The special behaviour for `event`: `hang`, `fork` or `flood`.
        pub fn mode(&self, event: &str, mode: &str) {
            self.put(&format!("{event}.mode"), mode);
        }

        /// Makes the named pipe `name` in the stub directory.
        pub fn fifo(&self, name: &str) -> PathBuf {
            let path = self.dir.path().join(name);
            let status = Command::new("mkfifo").arg(&path).status().expect("mkfifo");
            assert!(status.success(), "mkfifo {}", path.display());
            path
        }

        /// The calls so far, oldest first.
        pub fn calls(&self) -> Vec<Call> {
            let mut calls = Vec::new();
            for n in 1.. {
                let call = self.dir.path().join("calls").join(n.to_string());
                if !call.is_dir() {
                    break;
                }
                let read = |file: &str| fs::read_to_string(call.join(file)).expect("call file");
                calls.push(Call {
                    argv: read("argv").lines().map(str::to_owned).collect(),
                    cwd: PathBuf::from(read("cwd").trim_end()),
                    stdin: read("stdin"),
                });
            }
            calls
        }
    }

    /// Reads the named pipe `path` to its end on a thread of its own.
    ///
    /// Opening blocks until a writer opens the pipe, and the read ends when
    /// every writer has closed it, so the receiver's value arrives exactly
    /// when the stub has written (`ready`) or every process holding the pipe
    /// has died (`life`). The thread is detached, so a pipe nobody ever
    /// writes cannot hang the test process.
    pub fn read_pipe(path: PathBuf) -> mpsc::Receiver<Vec<u8>> {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Ok(mut pipe) = fs::File::open(path) {
                // A read error leaves what was read; the end of file is the signal.
                let _ = pipe.read_to_end(&mut bytes);
            }
            // The test may have given up; nobody to tell then.
            let _ = sender.send(bytes);
        });
        receiver
    }

    /// Waits for the end of the data of a pipe read by [`read_pipe`], failing
    /// after [`WAIT`].
    pub async fn wait_pipe(receiver: mpsc::Receiver<Vec<u8>>, what: &str) -> Vec<u8> {
        let heard = tokio::task::spawn_blocking(move || receiver.recv_timeout(WAIT))
            .await
            .expect("join");
        assert!(heard.is_ok(), "timed out waiting for {what}");
        heard.unwrap_or_default()
    }
}
