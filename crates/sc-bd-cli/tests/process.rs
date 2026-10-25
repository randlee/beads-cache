//! Real-process tests of the deadline-bounded run (NFR-BDC-0004, REQ-BDC-0006).
//!
//! The test binary is its own fake `bd`: this target has no libtest harness
//! (`harness = false`), and when the environment variable `BEADS_PATH` (which
//! `sc-bd-cli` sets to the project's working directory for every child) names a
//! directory called `sc-bd-cli-child-<mode>`, the binary behaves as a `bd` in that
//! mode and exits. Otherwise it runs the tests below, each pointing `BdCli` at this
//! very executable with a working directory that selects the mode. This needs no
//! shell, no script file and no installed `bd`, so it runs the same on every
//! platform CI covers.
//!
//! Modes: `ok` (answers `version`, `schema`, `list`), `refuse` (exit 13), `down`
//! (exit 1, "connection refused"), `slow` (prints `ready`, then runs 30 s),
//! `flood` (writes far more than a pipe holds), `huge` (writes one byte more than
//! `MAX_OUTPUT_BYTES` to stdout, then exits 0), `stdin` (reads standard input to the
//! end), `echo` (prints its argv), `spawner` (starts a grandchild that ignores TERM,
//! writes its pid to `grandchild`, prints `ready`, then runs 30 s; with the argument
//! `probe` it only prints whether that grandchild still runs), `leaver` (starts the
//! same grandchild with the child's standard output, then exits at once: the
//! grandchild holds the pipe, so the call times out reading the output; `probe` as
//! for `spawner`) and `orphan` (the Windows grandchild: writes its pid to
//! `$SC_BD_PID_FILE`, then runs 30 s).

use std::fs;
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sc_bd_cli::bounded::{
    kill_and_reap, run_bounded, run_bounded_capped, BoundedError, OutputStream, TimeoutPhase,
    MAX_OUTPUT_BYTES,
};
use sc_bd_cli::{BdCli, ProjectLocks, WorkingDir};
use sc_beads::{
    BeadsCacheError, CacheBackend, Clock, Deadline, Guards, Patch, ProjectRef, Refusal,
    ScopeFilter, SystemClock, BD_SCHEMA_SHA256, BD_VERSION,
};

const CHILD_PREFIX: &str = "sc-bd-cli-child-";
const SCHEMA: &str = include_str!("fixtures/bd-schema-1.3.1.json");

/// What `slow` and the readers get: far longer than any test waits.
const SLOW_CHILD_SECS: u64 = 30;
/// Bytes `flood` writes to stdout and stderr: several times a 64 KiB pipe buffer.
const FLOOD_STDOUT: usize = 4 * 1024 * 1024;
const FLOOD_STDERR: usize = 1024 * 1024;
/// The slowest a bounded call may take to give up in these tests; the children run
/// for 30 s, so a call that waited for one fails this bound by a wide margin.
const GIVE_UP_WITHIN: Duration = Duration::from_secs(10);

fn main() -> ExitCode {
    match child_mode() {
        Some(mode) => child(&mode),
        None => run_tests(),
    }
}

// ---- the fake bd ----------------------------------------------------------------

fn child_mode() -> Option<String> {
    let dir = std::env::var_os("BEADS_PATH")?;
    let name = Path::new(&dir).file_name()?.to_str()?;
    name.strip_prefix(CHILD_PREFIX).map(str::to_owned)
}

fn child(mode: &str) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(String::as_str);
    if mode == "spawner" || mode == "leaver" || mode == "orphan" {
        return process_group_child(mode, first);
    }
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    match (mode, first) {
        ("ok", Some("version")) => {
            let _ = writeln!(out, "bd version {BD_VERSION} (dev)");
            ExitCode::SUCCESS
        }
        ("ok", Some("schema")) => {
            let _ = write!(out, "{SCHEMA}");
            ExitCode::SUCCESS
        }
        ("ok", Some("list")) => {
            let _ = writeln!(out, "[]");
            ExitCode::SUCCESS
        }
        ("ok", _) => ExitCode::SUCCESS,
        ("refuse", _) => {
            let _ = writeln!(err, "Error updating: status mismatch");
            ExitCode::from(13)
        }
        ("down", _) => {
            let _ = writeln!(
                err,
                "Error: failed to open database: Dolt server unreachable at 127.0.0.1:1: dial tcp 127.0.0.1:1: connect: connection refused"
            );
            ExitCode::from(1)
        }
        ("slow", _) => {
            // The pid first, then the readiness marker: a test that waits for the line
            // knows the child is running, and one that finds the pid can ask if it still is.
            if let Some(dir) = std::env::var_os("BEADS_PATH") {
                let _ = fs::write(Path::new(&dir).join("pid"), std::process::id().to_string());
            }
            let _ = writeln!(out, "ready");
            let _ = out.flush();
            std::thread::sleep(Duration::from_secs(SLOW_CHILD_SECS));
            ExitCode::SUCCESS
        }
        ("huge", _) => {
            // The cap in whole chunks, then the one byte over it.
            let chunk = vec![b'x'; 1024 * 1024];
            let mut sent = 0;
            while sent < MAX_OUTPUT_BYTES && out.write_all(&chunk).is_ok() {
                sent += chunk.len();
            }
            let _ = out.write_all(b"x");
            let _ = out.flush();
            ExitCode::SUCCESS
        }
        ("flood", _) => {
            let chunk = vec![b'x'; 64 * 1024];
            // stdout and stderr are written in turn: a parent that drained one pipe
            // only would stall here once the other fills.
            let mut sent = (0, 0);
            while sent.0 < FLOOD_STDOUT || sent.1 < FLOOD_STDERR {
                if sent.0 < FLOOD_STDOUT {
                    let _ = out.write_all(&chunk);
                    sent.0 += chunk.len();
                }
                if sent.1 < FLOOD_STDERR {
                    let _ = err.write_all(&chunk);
                    sent.1 += chunk.len();
                }
            }
            let _ = out.flush();
            ExitCode::SUCCESS
        }
        ("stdin", _) => {
            let mut sink = Vec::new();
            let _ = std::io::stdin().read_to_end(&mut sink);
            let _ = write!(out, "eof after {} bytes", sink.len());
            ExitCode::SUCCESS
        }
        ("echo", _) => {
            for a in &args {
                let _ = writeln!(out, "arg:{a}");
            }
            let beads = std::env::var("BEADS_PATH").unwrap_or_default();
            let _ = writeln!(out, "beads_path:{beads}");
            ExitCode::SUCCESS
        }
        _ => {
            let _ = writeln!(err, "unknown fake-bd mode {mode:?}");
            ExitCode::from(99)
        }
    }
}

/// The modes `spawner`, `leaver` and `orphan`, of the process-group tests.
fn process_group_child(mode: &str, first: Option<&str>) -> ExitCode {
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    match (mode, first) {
        ("spawner" | "leaver", Some("probe")) => {
            let state = std::env::var_os("BEADS_PATH")
                .and_then(|dir| fs::read_to_string(Path::new(&dir).join("grandchild")).ok())
                .map_or("no grandchild", |pid| {
                    if process_runs(pid.trim()) {
                        "alive"
                    } else {
                        "gone"
                    }
                });
            let _ = writeln!(out, "{state}");
            ExitCode::SUCCESS
        }
        ("spawner", _) => {
            let Some(dir) = std::env::var_os("BEADS_PATH") else {
                return ExitCode::from(98);
            };
            let dir = PathBuf::from(dir);
            if spawn_grandchild(&dir.join("grandchild"), false).is_err() {
                let _ = writeln!(err, "cannot start the grandchild");
                return ExitCode::from(97);
            }
            let _ = fs::write(dir.join("pid"), std::process::id().to_string());
            let _ = writeln!(out, "ready");
            let _ = out.flush();
            std::thread::sleep(Duration::from_secs(SLOW_CHILD_SECS));
            ExitCode::SUCCESS
        }
        ("leaver", _) => {
            let Some(dir) = std::env::var_os("BEADS_PATH") else {
                return ExitCode::from(98);
            };
            let _ = writeln!(out, "ready");
            let _ = out.flush();
            if spawn_grandchild(&PathBuf::from(dir).join("grandchild"), true).is_err() {
                let _ = writeln!(err, "cannot start the grandchild");
                return ExitCode::from(97);
            }
            ExitCode::SUCCESS
        }
        ("orphan", _) => {
            if let Some(file) = std::env::var_os("SC_BD_PID_FILE") {
                let _ = fs::write(file, std::process::id().to_string());
            }
            std::thread::sleep(Duration::from_secs(SLOW_CHILD_SECS));
            ExitCode::SUCCESS
        }
        _ => ExitCode::from(99),
    }
}

/// Starts the grandchild of `spawner` or `leaver`, which writes its pid to `pid_file`
/// and runs 30 s. With `keep_stdout` it inherits the child's standard output (so it
/// holds that pipe open after the child exits), else it has no standard streams. It
/// stays in the child's process group, as a `dolt` that `bd` starts does.
///
/// On Unix it is a shell that ignores TERM and then becomes `sleep` (an ignored
/// signal stays ignored across `exec`), so only a KILL of the group ends it. On
/// Windows it is this binary in mode `orphan`.
fn spawn_grandchild(pid_file: &Path, keep_stdout: bool) -> std::io::Result<()> {
    let mut command = if cfg!(windows) {
        let mut c = std::process::Command::new(std::env::current_exe()?);
        let parent = pid_file
            .parent()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."));
        c.env("BEADS_PATH", parent.join(format!("{CHILD_PREFIX}orphan")))
            .env("SC_BD_PID_FILE", pid_file);
        c
    } else {
        let mut c = std::process::Command::new("/bin/sh");
        c.arg("-c")
            .arg(format!(
                "trap '' TERM; echo $$ > \"$1\"; exec sleep {SLOW_CHILD_SECS}"
            ))
            .arg("sh")
            .arg(pid_file);
        c
    };
    command
        .stdin(std::process::Stdio::null())
        .stdout(if keep_stdout {
            std::process::Stdio::inherit()
        } else {
            std::process::Stdio::null()
        })
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(drop)
}

/// Whether `pid` names a running process: not gone, and not a zombie waiting for
/// PID 1 to reap it (a zombie runs nothing).
fn process_runs(pid: &str) -> bool {
    if cfg!(windows) {
        return process_is_alive(pid);
    }
    std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .is_ok_and(|o| {
            let stat = String::from_utf8_lossy(&o.stdout);
            let stat = stat.trim();
            !stat.is_empty() && !stat.starts_with('Z')
        })
}

/// Reads the pid a child wrote to `path`, waiting up to [`GIVE_UP_WITHIN`].
fn pid_in(path: &Path) -> Result<String, String> {
    let until = Instant::now() + GIVE_UP_WITHIN;
    loop {
        if let Ok(pid) = fs::read_to_string(path) {
            if !pid.trim().is_empty() {
                return Ok(pid.trim().to_owned());
            }
        }
        if Instant::now() >= until {
            return Err(format!("{} never appeared", path.display()));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Kills a process the test left behind, so a failed run does not leak it.
fn kill_leftover(pid: &str) {
    let _ = if cfg!(windows) {
        std::process::Command::new("taskkill")
            .args(["/F", "/PID", pid])
            .output()
    } else {
        std::process::Command::new("kill")
            .args(["-KILL", pid])
            .output()
    };
}

// ---- the tests --------------------------------------------------------------------

type TestResult = Result<(), String>;
type Test = fn(&Env) -> TestResult;

struct Env {
    root: PathBuf,
    exe: String,
}

impl Env {
    /// The working directory that selects `mode`, created on first use.
    fn dir(&self, mode: &str) -> Result<String, String> {
        let dir = self.root.join(format!("{CHILD_PREFIX}{mode}"));
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        Ok(dir.to_string_lossy().into_owned())
    }

    fn project(&self, mode: &str) -> Result<ProjectRef, String> {
        Ok(ProjectRef::local(Some(self.dir(mode)?)))
    }

    fn cli(&self) -> BdCli {
        BdCli::new_with_clock(
            &self.exe,
            Arc::new(ProjectLocks::new()),
            Arc::new(SystemClock) as Arc<dyn Clock>,
        )
    }
}

fn require(cond: bool, what: impl Into<String>) -> TestResult {
    if cond {
        Ok(())
    } else {
        Err(what.into())
    }
}

fn after(ms: u64) -> Deadline {
    Deadline::after(&SystemClock, Duration::from_millis(ms))
}

fn scope() -> ScopeFilter {
    ScopeFilter::Label("x".into())
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| (*w).to_owned()).collect()
}

/// `TimedOut` is returned within its bound, whatever the child does: the deadline is
/// generous next to process start-up, and nothing is asserted about the child here
/// (`kill_and_reap_ends_a_running_child` does that, with no timing at all).
fn a_running_child_is_cut_at_the_deadline(env: &Env) -> TestResult {
    let project = env.project("slow")?;
    let pid_file = Path::new(&env.dir("slow")?).join("pid");
    let _ = fs::remove_file(&pid_file);
    let started = Instant::now();
    let r = env.cli().list_scope(&project, &scope(), None, after(1_000));
    let took = started.elapsed();
    require(
        matches!(&r, Err(BeadsCacheError::Unavailable { timed_out: true, reason })
            if reason.contains(TimeoutPhase::Child.as_str())),
        format!("expected a timeout in the child phase, got {r:?}"),
    )?;
    require(
        took < GIVE_UP_WITHIN,
        format!("gave up after {took:?}, the child runs {SLOW_CHILD_SECS} s"),
    )?;
    // `run_bounded` reaps the child before it returns, so if the child got as far as
    // writing its pid the process is already gone: no waiting, no sleeping. (A child
    // that had not started within the deadline leaves no pid and nothing to check;
    // `kill_and_reap_ends_a_running_child` covers the kill itself.)
    if let Ok(pid) = fs::read_to_string(&pid_file) {
        require(
            !process_is_alive(pid.trim()),
            format!("the child {pid} is still running after the timeout"),
        )?;
    }
    Ok(())
}

/// Whether a process with this pid exists (`kill -0`; `tasklist` on Windows).
fn process_is_alive(pid: &str) -> bool {
    if cfg!(windows) {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(pid))
    } else {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .output()
            .is_ok_and(|o| o.status.success())
    }
}

/// `kill_and_reap` ends a directly spawned child: it is reaped, and its stdout
/// reaches end of file. The child says `ready` first, so nothing here sleeps or
/// guesses how long start-up takes; the waits are only upper bounds on a failure.
fn kill_and_reap_ends_a_running_child(env: &Env) -> TestResult {
    let mut child = std::process::Command::new(&env.exe)
        .env("BEADS_PATH", env.dir("slow")?)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (eof_tx, eof_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        let _ = std::io::BufRead::read_line(&mut reader, &mut line);
        let _ = ready_tx.send(line);
        let mut rest = Vec::new();
        let _ = reader.read_to_end(&mut rest);
        let _ = eof_tx.send(());
    });
    let line = ready_rx
        .recv_timeout(GIVE_UP_WITHIN)
        .map_err(|e| format!("the child never became ready: {e}"))?;
    require(line.trim() == "ready", format!("readiness line {line:?}"))?;
    kill_and_reap(&mut child);
    eof_rx
        .recv_timeout(GIVE_UP_WITHIN)
        .map_err(|e| format!("stdout did not reach EOF after the kill: {e}"))?;
    let status = child.try_wait().map_err(|e| format!("try_wait: {e}"))?;
    require(status.is_some(), "the child was not reaped")
}

/// A timed-out bd takes its process group with it (PE-RSH-001): the grandchild it
/// started, which ignores TERM, is gone when the call returns, and the next call on
/// the same project, queued behind the project lock while the first one runs, starts
/// only once the grandchild is gone.
fn a_timed_out_child_takes_its_process_group_with_it(env: &Env) -> TestResult {
    let wd = env.dir("spawner")?;
    let grandchild_file = Path::new(&wd).join("grandchild");
    let _ = fs::remove_file(&grandchild_file);
    let _ = fs::remove_file(Path::new(&wd).join("pid"));
    let locks = Arc::new(ProjectLocks::new());

    let (exe, first_locks, first_wd) = (env.exe.clone(), Arc::clone(&locks), wd.clone());
    let first = std::thread::spawn(move || {
        run_bounded(
            &exe,
            &first_locks,
            &WorkingDir::new(&first_wd),
            &words(&["list"]),
            Duration::from_secs(3),
        )
    });
    // The grandchild runs, so the first call holds the project lock.
    let grandchild = pid_in(&grandchild_file)?;
    let next = run_bounded(
        &env.exe,
        &locks,
        &WorkingDir::new(&wd),
        &words(&["probe"]),
        Duration::from_secs(60),
    );
    let first = first
        .join()
        .map_err(|_panic| "the first call panicked".to_owned())?;
    let still_runs = process_runs(&grandchild);
    if still_runs {
        kill_leftover(&grandchild);
    }
    require(
        matches!(
            first,
            Err(BoundedError::TimedOut {
                phase: TimeoutPhase::Child
            })
        ),
        format!("expected a timeout in the child phase, got {first:?}"),
    )?;
    let next = next.map_err(|e| format!("the next call failed: {e}"))?;
    require(
        next.stdout.trim() == "gone",
        format!(
            "the next call ran while the grandchild {grandchild} was {:?}",
            next.stdout.trim()
        ),
    )?;
    require(
        !still_runs,
        format!("the grandchild {grandchild} outlived the timed-out call"),
    )
}

/// A bd that exits at once, leaving a TERM-ignoring helper that holds its standard
/// output, times out reading the output, and the helper dies before the project lock
/// is released (RCR-001): it is gone when the call returns, and the next call on the
/// project, queued behind the lock, runs only once it is gone. The kill must reach
/// the helper although the bd it descends from has exited (Windows: a `taskkill /T`
/// from the exited bd finds nothing) and must not be sent to a reaped pid.
fn an_output_timeout_takes_the_left_behind_helper_with_it(env: &Env) -> TestResult {
    let wd = env.dir("leaver")?;
    let helper_file = Path::new(&wd).join("grandchild");
    let _ = fs::remove_file(&helper_file);
    let locks = Arc::new(ProjectLocks::new());

    let (exe, first_locks, first_wd) = (env.exe.clone(), Arc::clone(&locks), wd.clone());
    let first = std::thread::spawn(move || {
        run_bounded(
            &exe,
            &first_locks,
            &WorkingDir::new(&first_wd),
            &words(&["list"]),
            Duration::from_secs(3),
        )
    });
    // The helper runs, so the first call holds the project lock.
    let helper = pid_in(&helper_file)?;
    let next = run_bounded(
        &env.exe,
        &locks,
        &WorkingDir::new(&wd),
        &words(&["probe"]),
        Duration::from_secs(60),
    );
    let first = first
        .join()
        .map_err(|_panic| "the first call panicked".to_owned())?;
    let still_runs = process_runs(&helper);
    if still_runs {
        kill_leftover(&helper);
    }
    require(
        matches!(
            first,
            Err(BoundedError::TimedOut {
                phase: TimeoutPhase::Output
            })
        ),
        format!("expected a timeout in the output phase, got {first:?}"),
    )?;
    let next = next.map_err(|e| format!("the next call failed: {e}"))?;
    require(
        next.stdout.trim() == "gone",
        format!(
            "the next call ran while the helper {helper} was {:?}",
            next.stdout.trim()
        ),
    )?;
    require(
        !still_runs,
        format!("the helper {helper} outlived the output timeout"),
    )
}

/// Two spellings of one project directory, the directory and a symlink to it, share
/// its project lock (PE-RBP-F010): a call through the symlink waits for the lock held
/// through the real path, and times out there.
#[cfg(unix)]
fn two_spellings_of_a_project_share_its_lock(env: &Env) -> TestResult {
    let real = env.dir("ok")?;
    // The link's own name selects `ok` too, so a call that wrongly got past the lock
    // runs the fake bd, not this test binary's tests.
    let links = env.root.join("links");
    fs::create_dir_all(&links).map_err(|e| format!("create {}: {e}", links.display()))?;
    let link = links.join(format!("{CHILD_PREFIX}ok"));
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(&real, &link).map_err(|e| format!("symlink: {e}"))?;
    let link = link.to_string_lossy().into_owned();
    let locks = ProjectLocks::new();
    let held = locks.guard(&real);
    let _held = held.lock().map_err(|e| format!("lock: {e}"))?;
    let r = run_bounded(
        &env.exe,
        &locks,
        &WorkingDir::new(&link),
        &words(&["list"]),
        Duration::from_millis(300),
    );
    require(
        matches!(
            r,
            Err(BoundedError::TimedOut {
                phase: TimeoutPhase::Lock
            })
        ),
        format!("the symlink spelling did not wait for the real path's lock: {r:?}"),
    )
}

fn waiting_for_the_project_lock_is_bounded(env: &Env) -> TestResult {
    let wd = env.dir("ok")?;
    let locks = Arc::new(ProjectLocks::new());
    let held = locks.guard(&wd);
    let _held = held.lock().map_err(|e| format!("lock: {e}"))?;
    let started = Instant::now();
    // The binary does not exist: if the call got past the lock it would fail to spawn
    // instead of timing out.
    let r = run_bounded(
        "definitely-not-a-real-cli-binary-xyz",
        &locks,
        &WorkingDir::new(&wd),
        &words(&["list"]),
        Duration::from_millis(300),
    );
    require(
        matches!(
            r,
            Err(BoundedError::TimedOut {
                phase: TimeoutPhase::Lock
            })
        ),
        format!("expected a timeout waiting for the lock, got {r:?}"),
    )?;
    require(
        started.elapsed() < GIVE_UP_WITHIN,
        "the wait for the lock was not bounded",
    )
}

fn an_expired_deadline_starts_nothing(env: &Env) -> TestResult {
    let cli = BdCli::new_with_clock(
        "definitely-not-a-real-cli-binary-xyz",
        Arc::new(ProjectLocks::new()),
        Arc::new(SystemClock) as Arc<dyn Clock>,
    );
    let r = cli.fingerprint(&env.project("ok")?, after(0));
    require(
        matches!(
            r,
            Err(BeadsCacheError::Unavailable {
                timed_out: true,
                ..
            })
        ),
        format!("an expired deadline must time out before spawning, got {r:?}"),
    )
}

fn output_larger_than_a_pipe_is_drained(env: &Env) -> TestResult {
    let locks = ProjectLocks::new();
    let out = run_bounded(
        &env.exe,
        &locks,
        &WorkingDir::new(&env.dir("flood")?),
        &words(&["list"]),
        Duration::from_secs(60),
    )
    .map_err(|e| format!("flood: {e}"))?;
    require(out.success, "flood child failed")?;
    require(
        out.stdout.len() == FLOOD_STDOUT,
        format!("stdout {} bytes", out.stdout.len()),
    )?;
    require(
        out.stderr.len() == FLOOD_STDERR,
        format!("stderr {} bytes", out.stderr.len()),
    )
}

/// A stream over the cap is `OutputTooLarge` naming the stream and the cap, not a
/// read error, and the child is still drained to the end (it exits instead of
/// stalling on a full pipe).
fn output_over_the_cap_is_an_error(env: &Env) -> TestResult {
    const CAP: usize = 1024 * 1024;
    let r = run_bounded_capped(
        &env.exe,
        &ProjectLocks::new(),
        &WorkingDir::new(&env.dir("flood")?),
        &words(&["list"]),
        Duration::from_secs(60),
        CAP,
    );
    require(
        matches!(
            &r,
            Err(BoundedError::OutputTooLarge {
                stream: OutputStream::Stdout,
                limit: CAP,
            })
        ),
        format!("expected OutputTooLarge for stdout at {CAP} bytes, got {r:?}"),
    )
}

/// PE-RBP-F012: a real `bd` whose stdout is one byte over `MAX_OUTPUT_BYTES` reaches
/// the cache caller as `Transport` whose source names stdout and the cap, with its
/// own error kind, not as a generic read error.
fn oversized_stdout_reaches_the_caller_as_transport_naming_the_cap(env: &Env) -> TestResult {
    let e = match env
        .cli()
        .list_ready(&env.project("huge")?, &scope(), after(60_000))
    {
        Ok(ids) => return Err(format!("expected an error, got {} ids", ids.len())),
        Err(e) => e,
    };
    let BeadsCacheError::Transport { what, source } = &e else {
        return Err(format!("expected Transport, got {e:?}"));
    };
    require(what == "bd ready", format!("what = {what}"))?;
    require(
        source.kind() == std::io::ErrorKind::FileTooLarge,
        format!("expected FileTooLarge, got {source:?}"),
    )?;
    let text = source.to_string();
    require(
        text.contains("stdout") && text.contains(&MAX_OUTPUT_BYTES.to_string()),
        format!("the source does not name stdout and the cap: {text}"),
    )?;
    require(!e.is_unavailable(), format!("{e:?} is an outage"))
}

fn standard_input_is_closed(env: &Env) -> TestResult {
    let out = run_bounded(
        &env.exe,
        &ProjectLocks::new(),
        &WorkingDir::new(&env.dir("stdin")?),
        &words(&["list"]),
        Duration::from_secs(30),
    )
    .map_err(|e| format!("stdin: {e}"))?;
    require(
        out.stdout == "eof after 0 bytes",
        format!("got {:?}", out.stdout),
    )
}

fn argv_and_working_directory_reach_the_child_verbatim(env: &Env) -> TestResult {
    let wd = env.dir("echo")?;
    let out = run_bounded(
        &env.exe,
        &ProjectLocks::new(),
        &WorkingDir::new(&wd),
        &words(&[
            "update",
            "b-1",
            "",
            "--append-notes",
            "two words",
            "--actor",
            "san",
        ]),
        Duration::from_secs(30),
    )
    .map_err(|e| format!("echo: {e}"))?;
    let lines: Vec<&str> = out.stdout.lines().collect();
    let want = [
        "arg:update",
        "arg:b-1",
        "arg:",
        "arg:--append-notes",
        "arg:two words",
        "arg:--actor",
        "arg:san",
    ];
    require(
        lines.get(..want.len()) == Some(&want[..]),
        format!("argv was altered: {lines:?}"),
    )?;
    require(
        lines.last() == Some(&format!("beads_path:{wd}").as_str()),
        format!("BEADS_PATH is not the working directory: {lines:?}"),
    )
}

fn exit_13_is_a_precondition_refusal(env: &Env) -> TestResult {
    let patch = Patch {
        status: Some("open".into()),
        ..Patch::default()
    };
    let guards = Guards {
        status: Some("closed".into()),
        assignee: None,
    };
    let r = env.cli().update_guarded(
        &env.project("refuse")?,
        &sc_beads::BeadId::new("b-1").map_err(|e| e.to_string())?,
        &patch,
        &guards,
        &sc_beads::Actor::new("san").map_err(|e| e.to_string())?,
        after(30_000),
    );
    require(
        matches!(r, Err(BeadsCacheError::Refused(Refusal::Precondition))),
        format!("expected a precondition refusal, got {r:?}"),
    )
}

fn an_unreachable_server_is_unavailable(env: &Env) -> TestResult {
    let r = env
        .cli()
        .list_ready(&env.project("down")?, &scope(), after(30_000));
    require(
        matches!(&r, Err(e) if e.is_unavailable() && !e.is_timeout()),
        format!("expected Unavailable (not a timeout), got {r:?}"),
    )
}

fn a_bd_that_cannot_start_is_unavailable_with_its_cause(env: &Env) -> TestResult {
    let cli = BdCli::new_with_clock(
        "definitely-not-a-real-cli-binary-xyz",
        Arc::new(ProjectLocks::new()),
        Arc::new(SystemClock) as Arc<dyn Clock>,
    );
    let r = cli.list_ready(&env.project("ok")?, &scope(), after(30_000));
    let Err(e) = r else {
        return Err("expected an error".into());
    };
    require(e.is_unavailable() && !e.is_timeout(), format!("{e:?}"))?;
    let beads = std::error::Error::source(&e).ok_or("no source on the cache error")?;
    let io = std::error::Error::source(beads).ok_or("no io::Error behind the spawn error")?;
    require(!io.to_string().is_empty(), "empty io::Error")
}

fn the_backend_check_runs_version_and_schema(env: &Env) -> TestResult {
    let fp = env
        .cli()
        .fingerprint(&env.project("ok")?, after(30_000))
        .map_err(|e| e.to_string())?;
    require(
        fp.bd_version == BD_VERSION,
        format!("version {}", fp.bd_version),
    )?;
    require(fp.schema_sha256.as_str() == BD_SCHEMA_SHA256, "schema sha")?;
    require(
        fp.schema_version == "1",
        format!("schema_version {}", fp.schema_version),
    )
}

fn an_empty_list_is_no_rows(env: &Env) -> TestResult {
    let rows = env
        .cli()
        .list_scope(&env.project("ok")?, &scope(), None, after(30_000))
        .map_err(|e| e.to_string())?;
    require(rows.is_empty(), "expected no rows")
}

fn run_tests() -> ExitCode {
    let tests: &[(&str, Test)] = &[
        (
            "a_running_child_is_cut_at_the_deadline",
            a_running_child_is_cut_at_the_deadline,
        ),
        (
            "kill_and_reap_ends_a_running_child",
            kill_and_reap_ends_a_running_child,
        ),
        (
            "a_timed_out_child_takes_its_process_group_with_it",
            a_timed_out_child_takes_its_process_group_with_it,
        ),
        (
            "an_output_timeout_takes_the_left_behind_helper_with_it",
            an_output_timeout_takes_the_left_behind_helper_with_it,
        ),
        #[cfg(unix)]
        (
            "two_spellings_of_a_project_share_its_lock",
            two_spellings_of_a_project_share_its_lock,
        ),
        (
            "output_over_the_cap_is_an_error",
            output_over_the_cap_is_an_error,
        ),
        (
            "oversized_stdout_reaches_the_caller_as_transport_naming_the_cap",
            oversized_stdout_reaches_the_caller_as_transport_naming_the_cap,
        ),
        (
            "waiting_for_the_project_lock_is_bounded",
            waiting_for_the_project_lock_is_bounded,
        ),
        (
            "an_expired_deadline_starts_nothing",
            an_expired_deadline_starts_nothing,
        ),
        (
            "output_larger_than_a_pipe_is_drained",
            output_larger_than_a_pipe_is_drained,
        ),
        ("standard_input_is_closed", standard_input_is_closed),
        (
            "argv_and_working_directory_reach_the_child_verbatim",
            argv_and_working_directory_reach_the_child_verbatim,
        ),
        (
            "exit_13_is_a_precondition_refusal",
            exit_13_is_a_precondition_refusal,
        ),
        (
            "an_unreachable_server_is_unavailable",
            an_unreachable_server_is_unavailable,
        ),
        (
            "a_bd_that_cannot_start_is_unavailable_with_its_cause",
            a_bd_that_cannot_start_is_unavailable_with_its_cause,
        ),
        (
            "the_backend_check_runs_version_and_schema",
            the_backend_check_runs_version_and_schema,
        ),
        ("an_empty_list_is_no_rows", an_empty_list_is_no_rows),
    ];
    let filter = std::env::args().skip(1).find(|a| !a.starts_with('-'));
    let selected: Vec<_> = tests
        .iter()
        .filter(|(name, _)| filter.as_ref().is_none_or(|f| name.contains(f.as_str())))
        .collect();

    let exe = match std::env::current_exe() {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(e) => {
            eprintln!("cannot find the test executable: {e}");
            return ExitCode::FAILURE;
        }
    };
    let root = std::env::temp_dir().join(format!("sc-bd-cli-process-{}", std::process::id()));
    let env = Env { root, exe };

    println!("running {} tests", selected.len());
    let mut failed = 0;
    for (name, test) in selected {
        let outcome = catch_unwind(AssertUnwindSafe(|| test(&env)))
            .unwrap_or_else(|_| Err("panicked".to_owned()));
        match outcome {
            Ok(()) => println!("test {name} ... ok"),
            Err(why) => {
                failed += 1;
                println!("test {name} ... FAILED\n    {why}");
            }
        }
    }
    let _ = fs::remove_dir_all(&env.root);
    if failed == 0 {
        println!("\ntest result: ok.");
        ExitCode::SUCCESS
    } else {
        println!("\ntest result: FAILED. {failed} failed");
        ExitCode::FAILURE
    }
}
