//! Deadline-bounded process invocation for the cache calls (NFR-BDC-0004).
//!
//! btit's [`run::spawn_json`](crate::run::spawn_json) blocks until the child exits
//! (`Command::output()`), which is right for an app that shows a spinner and wrong
//! for a cache tick that must stop at its deadline. [`run_bounded`] is the
//! bounded counterpart used by [`CliInvoker::run_bounded`](crate::CliInvoker::run_bounded):
//!
//! - the project lock is acquired by polling, never by blocking, so a wait behind a
//!   slow call ends at the deadline too ([`TimeoutPhase::Lock`]);
//! - the child is started as the leader of its own process group and polled with
//!   `try_wait`; when the time is up the whole group is killed, so a `dolt` that `bd`
//!   started dies with it ([`TimeoutPhase::Child`], PE-RSH-001), and the project lock
//!   is held until that kill has finished or [`KILL_BOUND`] has passed;
//! - a killed child is reaped by polling for at most [`KILL_BOUND`]; one that is still
//!   there is handed to a detached reaper thread instead of being waited for
//!   (PE-RSH-002);
//! - its output is drained by two reader threads, so a chatty child cannot fill a
//!   pipe and stall, the wait for those threads is bounded by the same deadline
//!   ([`TimeoutPhase::Output`], after which the group the child left behind is
//!   killed too), and each stream is capped at [`MAX_OUTPUT_BYTES`]
//!   (the excess is drained and dropped, and the call fails with
//!   [`BoundedError::Output`]).
//!
//! The time limit is a [`Duration`] measured on the real clock from the moment the
//! function is entered. The caller turns its [`Deadline`](sc_beads::Deadline) into
//! that duration with the shared [`Clock`](sc_beads::Clock), so a test clock decides
//! whether a deadline has *already* passed while real elapsed time decides when a
//! running child is stopped.

use std::io::{self, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{OnceLock, TryLockError};
use std::thread;
use std::time::{Duration, Instant};

use sc_beads::error::BeadsError;
use sc_beads::{log_error, log_warn};
use sc_beads::{CliClient, CliOutput};

use crate::command::{lead_process_group, ChildEnv};
use crate::locks::{ProjectLocks, WorkingDir};
use crate::path::get_extended_path;

/// How often a waiting call re-checks the project lock and the child's status.
///
/// Short enough that a finished child is noticed within a few milliseconds, long
/// enough that waiting costs no measurable CPU. A call never sleeps past its
/// deadline: the last sleep is shortened to the time left.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// The most bytes [`run_bounded`] keeps from each of a child's standard output and
/// standard error.
///
/// A 1000-bead phase listed with `bd list` is a few megabytes of JSON, so 64 MiB
/// leaves a wide margin while keeping a runaway child from exhausting memory. Over the
/// cap the rest is read and dropped (so the child is not stalled on a full pipe) and
/// the call fails with [`BoundedError::Output`].
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

/// How long killing a child's process group and reaping the child may take together
/// before they are given up on (PE-RSH-001, PE-RSH-002).
///
/// One second is far above what a `kill` of a process group or a `taskkill /T` takes,
/// and short next to a cache call's budget (the cache caps a call at 30 s). Past it a
/// stuck kill program or a child that has not exited (an uninterruptible state) is
/// handed to the reaper thread, so a bounded call ends at most this long after its
/// deadline.
pub const KILL_BOUND: Duration = Duration::from_secs(1);

/// How often the reaper thread looks at the processes it holds: they were killed, so
/// they normally exit within one look; the interval only keeps an idle reaper cheap.
const REAP_POLL: Duration = Duration::from_millis(200);

/// Saturation for a timeout too large to add to an [`Instant`]: far enough that no
/// real call reaches it (the same bound as `Deadline::after`).
const FAR_FUTURE: Duration = Duration::from_hours(100 * 365 * 24);

/// Where a bounded call was when its time ran out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeoutPhase {
    /// Waiting for the per-project lock; no child was started.
    Lock,
    /// Waiting for the child to exit; it was killed.
    Child,
    /// The child exited but its output was not complete (a process it left behind
    /// holds the pipes open).
    Output,
}

impl TimeoutPhase {
    /// What the call was doing, as a phrase for a reason string.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lock => "waiting for the project lock",
            Self::Child => "waiting for the child to exit",
            Self::Output => "reading the child's output",
        }
    }
}

impl std::fmt::Display for TimeoutPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a bounded invocation produced no output.
#[derive(Debug)]
#[non_exhaustive]
pub enum BoundedError {
    /// The time limit ran out in this phase. A running child was killed.
    TimedOut {
        /// What the call was waiting for.
        phase: TimeoutPhase,
    },
    /// The call could not be made: the process could not be started or waited on,
    /// or the project reference is not supported. The cause is kept as the
    /// [`BeadsError`] (for a spawn failure, with its `io::Error` source).
    Failed(BeadsError),
    /// The child's output could not be read, or exceeded [`MAX_OUTPUT_BYTES`]. The
    /// read error is the source.
    Output(io::Error),
}

impl std::fmt::Display for BoundedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimedOut { phase } => write!(f, "timed out {phase}"),
            Self::Failed(e) => std::fmt::Display::fmt(e, f),
            Self::Output(e) => write!(f, "reading the child's output failed: {e}"),
        }
    }
}

impl std::error::Error for BoundedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TimedOut { .. } => None,
            Self::Failed(e) => Some(e),
            Self::Output(e) => Some(e),
        }
    }
}

impl From<BeadsError> for BoundedError {
    fn from(e: BeadsError) -> Self {
        Self::Failed(e)
    }
}

/// [`run_bounded_capped`] with the cap [`MAX_OUTPUT_BYTES`].
///
/// # Errors
///
/// As [`run_bounded_capped`].
pub fn run_bounded(
    binary: &str,
    locks: &ProjectLocks,
    working_dir: &WorkingDir,
    argv: &[String],
    timeout: Duration,
) -> Result<CliOutput, BoundedError> {
    run_bounded_capped(binary, locks, working_dir, argv, timeout, MAX_OUTPUT_BYTES)
}

/// Runs `<binary> <argv…>` in `working_dir` under that directory's project lock and
/// stops at `timeout`, keeping at most `max_output` bytes of each output stream.
///
/// The environment is the one of [`spawn_json`](crate::run::spawn_json): the extended
/// `PATH` and `BEADS_PATH` set to the working directory. Standard input is closed.
/// Any exit status is returned as `Ok`; the caller classifies it.
///
/// # Errors
///
/// - [`BoundedError::TimedOut`] when the lock, the child or its output is not ready
///   within `timeout`; a running child's process group is killed and the child
///   reaped (or handed to the reaper at [`KILL_BOUND`]) first, under the lock.
/// - [`BoundedError::Failed`] with [`BeadsError::Spawn`] (`operation` = the first
///   argument) when the process cannot be started or waiting for it fails, and with
///   [`BeadsError::Unsupported`] when the canonical path of `working_dir` is not UTF-8
///   ([`ProjectLocks::try_guard_dir`]).
/// - [`BoundedError::Output`] when a reader thread cannot be started, a read fails,
///   or a stream exceeds `max_output`.
pub fn run_bounded_capped(
    binary: &str,
    locks: &ProjectLocks,
    working_dir: &WorkingDir,
    argv: &[String],
    timeout: Duration,
    max_output: usize,
) -> Result<CliOutput, BoundedError> {
    run_bounded_in(
        binary,
        locks,
        working_dir,
        argv,
        timeout,
        max_output,
        &ChildEnv::default(),
    )
}

/// [`run_bounded_capped`] with `env` applied to the child (a
/// [`CliRunner`](crate::CliRunner)'s child environment, ADR-BDCLI-0004).
pub(crate) fn run_bounded_in(
    binary: &str,
    locks: &ProjectLocks,
    working_dir: &WorkingDir,
    argv: &[String],
    timeout: Duration,
    max_output: usize,
    env: &ChildEnv,
) -> Result<CliOutput, BoundedError> {
    let start = Instant::now();
    let limit = start
        .checked_add(timeout)
        .or_else(|| start.checked_add(FAR_FUTURE))
        .unwrap_or(start);

    // The lock serializes bd processes on one project (embedded Dolt crashes when
    // two run at once). It is taken by polling so the wait is bounded; a poisoned
    // lock is recovered because the guarded unit holds no state.
    let project_lock = locks.try_guard_dir(working_dir).map_err(|e| {
        log_error!("[bd] refusing to lock {}: {}", working_dir, e);
        BoundedError::Failed(BeadsError::Unsupported {
            operation: "a project directory whose canonical path is not UTF-8",
            client: CliClient::Unknown,
        })
    })?;
    let _guard = loop {
        match project_lock.try_lock() {
            Ok(guard) => break guard,
            Err(TryLockError::Poisoned(poisoned)) => break poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => wait_or_time_out(limit, TimeoutPhase::Lock)?,
        }
    };

    let mut command = env.command(binary);
    command
        .args(argv)
        .current_dir(working_dir.as_str())
        .env("PATH", get_extended_path())
        .env("BEADS_PATH", working_dir.as_str());
    // `_guard` outlives this call: every kill `run_child` makes on a timeout has
    // finished, or reached `KILL_BOUND`, before the next call on the project starts.
    run_child(
        command,
        binary,
        argv.first().map(String::as_str),
        limit,
        max_output,
    )
}

/// Spawns `command` as the leader of its own process group, with standard input
/// closed and both outputs drained, and stops it at `limit`.
///
/// On a timeout of the child the whole group is killed and the child reaped (within
/// [`KILL_BOUND`]) before this returns; on a timeout of the output (the child exited,
/// a process it left behind holds a pipe open) the group it left behind is killed.
/// `binary` and `operation` name the spawn in a [`BeadsError::Spawn`].
pub(crate) fn run_child(
    mut command: Command,
    binary: &str,
    operation: Option<&str>,
    limit: Instant,
    max_output: usize,
) -> Result<CliOutput, BoundedError> {
    let spawn_error = |source: io::Error| {
        BoundedError::Failed(BeadsError::Spawn {
            binary: binary.to_owned(),
            operation: operation.map(str::to_owned),
            source,
        })
    };
    lead_process_group(&mut command);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(spawn_error)?;

    let stdout = match drain(child.stdout.take(), max_output) {
        Ok(rx) => rx,
        Err(e) => return Err(abort(child, BoundedError::Output(e))),
    };
    let stderr = match drain(child.stderr.take(), max_output) {
        Ok(rx) => rx,
        Err(e) => return Err(abort(child, BoundedError::Output(e))),
    };

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if let Err(timed_out) = wait_or_time_out(limit, TimeoutPhase::Child) {
                    // The readers are abandoned: they end when the pipes close, which
                    // the group kill makes happen.
                    terminate(child);
                    return Err(timed_out);
                }
            }
            Err(e) => return Err(abort(child, spawn_error(e))),
        }
    };

    let pid = child.id();
    let output = collect(&stdout, limit).and_then(|out| Ok((out, collect(&stderr, limit)?)));
    if let Err(BoundedError::TimedOut { .. }) = &output {
        // A reader still open at the limit is held by a process the child left in
        // its group: kill that group too, so it does not outlive the project lock.
        kill_left_behind(pid);
    }
    let (stdout, stderr) = output?;
    Ok(CliOutput {
        status: status.code(),
        success: status.success(),
        stdout,
        stderr,
    })
}

/// The output one reader thread sent, waited for until `limit`.
fn collect(rx: &Receiver<io::Result<Vec<u8>>>, limit: Instant) -> Result<String, BoundedError> {
    match rx.recv_timeout(limit.saturating_duration_since(Instant::now())) {
        Ok(Ok(bytes)) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Ok(Err(e)) => Err(BoundedError::Output(e)),
        Err(RecvTimeoutError::Timeout) => Err(BoundedError::TimedOut {
            phase: TimeoutPhase::Output,
        }),
        Err(RecvTimeoutError::Disconnected) => Err(BoundedError::Output(io::Error::other(
            "the reader thread ended without a result",
        ))),
    }
}

/// Sleeps one poll interval, or fails with [`BoundedError::TimedOut`] in `phase` when
/// `limit` has been reached.
fn wait_or_time_out(limit: Instant, phase: TimeoutPhase) -> Result<(), BoundedError> {
    let left = limit.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(BoundedError::TimedOut { phase });
    }
    thread::sleep(left.min(POLL_INTERVAL));
    Ok(())
}

/// Starts a thread that reads `pipe` to the end and sends what it read, or the read
/// error, or [`io::ErrorKind::Other`] when the stream is longer than `max_output`.
fn drain<R: Read + Send + 'static>(
    pipe: Option<R>,
    max_output: usize,
) -> io::Result<Receiver<io::Result<Vec<u8>>>> {
    let (tx, rx) = mpsc::channel();
    let Some(mut pipe) = pipe else {
        // Both pipes were requested with `Stdio::piped()`; an absent one yields empty output.
        let _ = tx.send(Ok(Vec::new()));
        return Ok(rx);
    };
    thread::Builder::new()
        .name("sc-bd-cli-reader".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            let cap = u64::try_from(max_output).unwrap_or(u64::MAX);
            let read = (&mut pipe)
                .take(cap.saturating_add(1))
                .read_to_end(&mut bytes);
            let result = match read {
                Ok(_) if bytes.len() > max_output => {
                    // Keep reading and discard, so the child is not stalled on a full pipe.
                    let _ = io::copy(&mut pipe, &mut io::sink());
                    Err(io::Error::other(format!(
                        "output exceeds the cap of {max_output} bytes"
                    )))
                }
                Ok(_) => Ok(bytes),
                Err(e) => Err(e),
            };
            // The receiver is gone when the call timed out; nothing to report.
            let _ = tx.send(result);
        })?;
    Ok(rx)
}

/// Kills the child and reaps it, waiting at most [`KILL_BOUND`]; errors from a child
/// that already exited are ignored.
///
/// The child is polled with `try_wait`, never waited for without a bound
/// (PE-RSH-002). When it returns the child has normally been reaped and its pipes are
/// closed. A child still there at the bound (an uninterruptible state) is logged and
/// left with the caller, who owns it: a borrowed `Child` cannot be moved to the
/// reaper thread. The bounded calls of this module own their children and hand such
/// a child to the reaper instead.
///
/// Only the child itself is killed, not a process group: the caller's child need not
/// lead one.
pub fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    if !exited_by(child, Instant::now() + KILL_BOUND) {
        log_warn!(
            "[bd] the killed child {} was not reaped within {:?}; leaving it to its owner",
            child.id(),
            KILL_BOUND
        );
    }
}

/// Cleans up a child after an internal failure and passes the error through.
fn abort(child: Child, error: BoundedError) -> BoundedError {
    terminate(child);
    error
}

/// Kills the process group `child` leads, then the child itself, and reaps it, all
/// within one [`KILL_BOUND`] from now (PE-RSH-001).
///
/// Only called while the child has not been reaped, so its pid, the group's id, cannot
/// name another process. After a group kill that ran, it also waits (within the same
/// bound) until the group has no member left, so a descendant is gone, not only
/// signalled, when the caller releases the project lock.
fn terminate(mut child: Child) {
    let until = Instant::now() + KILL_BOUND;
    let pid = child.id();
    let group = kill_group(pid, until);
    // The group kill may have failed or be absent on this platform: kill the child
    // itself too.
    let _ = child.kill();
    if reap_or_hand_off(child, until) == Reaped::Exited && group == Helper::Done {
        wait_until_group_is_gone(pid, until);
    }
}

/// Kills the process group a reaped child left behind, within [`KILL_BOUND`].
///
/// The child led the group, so its pid is the group's id; a group that still has a
/// member keeps that id from being reused (POSIX), and one that has none makes the
/// kill fail harmlessly.
fn kill_left_behind(pid: u32) {
    let until = Instant::now() + KILL_BOUND;
    if kill_group(pid, until) == Helper::Done {
        wait_until_group_is_gone(pid, until);
    }
}

/// Whether a killed child was reaped in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reaped {
    /// It exited and was reaped.
    Exited,
    /// It was still there at the bound and was handed to the reaper thread.
    HandedOff,
}

/// Reaps `child` (already killed) by `until`, or hands it to the reaper thread
/// (PE-RSH-002): never a `wait` without a bound.
fn reap_or_hand_off(mut child: Child, until: Instant) -> Reaped {
    if exited_by(&mut child, until) {
        Reaped::Exited
    } else {
        log_warn!(
            "[bd] the killed child {} was not reaped in time; handing it to the reaper",
            child.id()
        );
        adopt(child);
        Reaped::HandedOff
    }
}

/// Polls `child` until it has exited or `until` has passed; whether it exited. It is
/// always looked at once, even when `until` has already passed. A `try_wait` error
/// means it cannot be waited for, which counts as not exited.
fn exited_by(child: &mut Child, until: Instant) -> bool {
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(_) => return false,
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        thread::sleep(left.min(POLL_INTERVAL));
    }
}

/// Hands a process that has not been reaped to the one reaper thread, which waits for
/// it so it is not left a zombie.
///
/// The thread is started on first use and lives as long as the host; it polls what it
/// holds every [`REAP_POLL`] and drops each process once it has exited. A process that
/// never exits is held, not waited out, so it cannot keep the others from being reaped.
fn adopt(child: Child) {
    static REAPER: OnceLock<Option<mpsc::Sender<Child>>> = OnceLock::new();
    let reaper = REAPER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Child>();
        match thread::Builder::new()
            .name("sc-bd-cli-reaper".to_owned())
            .spawn(move || reap(&rx))
        {
            Ok(_) => Some(tx),
            Err(e) => {
                log_error!("[bd] cannot start the reaper thread: {}", e);
                None
            }
        }
    });
    let pid = child.id();
    if reaper.as_ref().is_none_or(|tx| tx.send(child).is_err()) {
        log_error!(
            "[bd] no reaper thread; the killed child {} may stay a zombie",
            pid
        );
    }
}

/// The reaper thread: holds the processes it is given until each has exited.
fn reap(rx: &Receiver<Child>) {
    let mut held: Vec<Child> = Vec::new();
    loop {
        let next = if held.is_empty() {
            rx.recv()
                .map_err(|mpsc::RecvError| RecvTimeoutError::Disconnected)
        } else {
            rx.recv_timeout(REAP_POLL)
        };
        match next {
            Ok(child) => held.push(child),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) if held.is_empty() => return,
            Err(RecvTimeoutError::Disconnected) => thread::sleep(REAP_POLL),
        }
        // Keep only what is still running; an error means it cannot be waited for.
        held.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
    }
}

/// How a helper program (the group kill, or the check that a group is empty) ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Helper {
    /// It ran and succeeded.
    Done,
    /// It could not be started, failed, or could not be waited for.
    Failed,
    /// It was still running at the bound: it was killed and handed to the reaper.
    GaveUp,
}

/// Runs a helper program until `until` at most, with no input or output.
fn run_helper(mut command: Command, until: Instant) -> Helper {
    let Ok(mut helper) = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Helper::Failed;
    };
    loop {
        match helper.try_wait() {
            Ok(Some(status)) if status.success() => return Helper::Done,
            Ok(Some(_)) | Err(_) => return Helper::Failed,
            Ok(None) => {}
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            let _ = helper.kill();
            reap_or_hand_off(helper, Instant::now());
            return Helper::GaveUp;
        }
        thread::sleep(left.min(POLL_INTERVAL));
    }
}

/// Kills the process group led by `pid` (Unix), or the process tree rooted at it
/// (Windows), within `until`. A failure is logged; the caller kills the child itself
/// regardless.
fn kill_group(pid: u32, until: Instant) -> Helper {
    let outcome = group_kill_command(pid).map_or(Helper::Failed, |kill| run_helper(kill, until));
    if outcome != Helper::Done {
        log_warn!(
            "[bd] killing the process group of {} did not succeed: {:?}",
            pid,
            outcome
        );
    }
    outcome
}

/// The platform's program that kills the group led by `pid`: `kill -KILL -- -<pid>`
/// on Unix (the negative pid names the group), `taskkill /T /F` on Windows.
///
/// A program, not a libc call: the crate has no `unsafe` and no libc dependency.
/// `None` on a platform with neither.
#[cfg_attr(
    any(unix, target_os = "windows"),
    expect(
        clippy::unnecessary_wraps,
        reason = "None on a platform with neither program"
    )
)]
fn group_kill_command(pid: u32) -> Option<Command> {
    #[cfg(unix)]
    {
        let mut kill = Command::new(unix_kill());
        kill.args(["-KILL", "--", &format!("-{pid}")]);
        Some(kill)
    }
    #[cfg(target_os = "windows")]
    {
        let mut kill = crate::command::new_command("taskkill");
        kill.args(["/T", "/F", "/PID", &pid.to_string()]);
        Some(kill)
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = pid;
        None
    }
}

/// `/bin/kill` when it exists, else `kill` from `PATH`.
#[cfg(unix)]
fn unix_kill() -> &'static str {
    if std::path::Path::new("/bin/kill").is_file() {
        "/bin/kill"
    } else {
        "kill"
    }
}

/// Waits until the group led by `pid` has no member left, or `until` has passed.
///
/// `kill -0 -- -<pid>` succeeds while the group has a member, so a descendant the
/// group kill signalled is known to be gone when it fails. On Windows `taskkill /F`
/// has already terminated the tree, so there is nothing to wait for.
fn wait_until_group_is_gone(pid: u32, until: Instant) {
    #[cfg(unix)]
    loop {
        let mut probe = Command::new(unix_kill());
        probe.args(["-0", "--", &format!("-{pid}")]);
        if run_helper(probe, until) != Helper::Done {
            return;
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            log_warn!(
                "[bd] the process group of {} outlived {:?}",
                pid,
                KILL_BOUND
            );
            return;
        }
        thread::sleep(left.min(POLL_INTERVAL));
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, until);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `pid` names a process that exists and is not a zombie.
    #[cfg(unix)]
    fn running(pid: u32) -> bool {
        Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .is_ok_and(|o| {
                let stat = String::from_utf8_lossy(&o.stdout);
                let stat = stat.trim();
                !stat.is_empty() && !stat.starts_with('Z')
            })
    }

    /// Whether `pid` names a process at all, a zombie included.
    #[cfg(unix)]
    fn exists(pid: u32) -> bool {
        Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .is_ok_and(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
    }

    /// A child still running at the bound is not waited for: the reap gives up at
    /// the bound and hands the child to the reaper thread, which reaps it once it has
    /// exited, so it is no zombie (PE-RSH-002).
    ///
    /// The child is not killed before the reap, which is the condition the bound is
    /// for: a killed child that has not exited (an uninterruptible state).
    #[cfg(unix)]
    #[test]
    fn a_child_still_running_at_the_bound_is_handed_to_the_reaper() {
        const BOUND: Duration = Duration::from_millis(200);
        let child = Command::new("sleep").arg("5").spawn().unwrap();
        let pid = child.id();
        let started = Instant::now();
        let outcome = reap_or_hand_off(child, started + BOUND);
        let took = started.elapsed();
        assert_eq!(outcome, Reaped::HandedOff);
        assert!(took >= BOUND, "gave up before the bound: {took:?}");
        assert!(
            took < Duration::from_secs(3),
            "the reap was not bounded: {took:?}"
        );
        assert!(
            running(pid),
            "the child should still run, handed to the reaper"
        );

        // Now let it exit: the reaper, not this test, reaps it.
        let killed = Command::new(unix_kill())
            .args(["-KILL", &pid.to_string()])
            .status()
            .unwrap();
        assert!(killed.success());
        let until = Instant::now() + Duration::from_secs(10);
        while exists(pid) {
            assert!(Instant::now() < until, "the reaper did not reap {pid}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// `exited_by` gives up at its bound on a child that is running, and reports
    /// one that has exited.
    #[cfg(unix)]
    #[test]
    fn exited_by_is_bounded() {
        let mut child = Command::new("sleep").arg("5").spawn().unwrap();
        let started = Instant::now();
        assert!(!exited_by(&mut child, started + Duration::from_millis(100)));
        assert!(started.elapsed() < Duration::from_secs(3));
        child.kill().unwrap();
        assert!(exited_by(
            &mut child,
            Instant::now() + Duration::from_secs(10)
        ));
    }
}
