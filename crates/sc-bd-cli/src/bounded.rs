//! Deadline-bounded process invocation for the cache calls (NFR-BDC-0004).
//!
//! btit's [`run::spawn_json`](crate::run::spawn_json) blocks until the child exits
//! (`Command::output()`), which is right for an app that shows a spinner and wrong
//! for a cache tick that must stop at its deadline. [`run_bounded`] is the
//! bounded counterpart used by [`CliInvoker::run_bounded`](crate::CliInvoker::run_bounded):
//!
//! - the project lock is acquired by polling, never by blocking, so a wait behind a
//!   slow call ends at the deadline too;
//! - the child is polled with `try_wait` and killed (then reaped) when the time is
//!   up, returning [`BoundedError::TimedOut`];
//! - its output is drained by two reader threads, so a chatty child cannot fill a
//!   pipe and stall, and the wait for those threads is bounded by the same deadline.
//!
//! The time limit is a [`Duration`] measured on the real clock from the moment the
//! function is entered. The caller turns its [`Deadline`](sc_beads::Deadline) into
//! that duration with the shared [`Clock`](sc_beads::Clock), so a test clock decides
//! whether a deadline has *already* passed while real elapsed time decides when a
//! running child is stopped.

use std::io::Read;
use std::process::{Child, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::TryLockError;
use std::thread;
use std::time::{Duration, Instant};

use sc_beads::error::BeadsError;
use sc_beads::CliOutput;

use crate::command::new_command;
use crate::locks::ProjectLocks;
use crate::path::get_extended_path;

/// How often a waiting call re-checks the project lock and the child's status.
///
/// Short enough that a finished child is noticed within a few milliseconds, long
/// enough that waiting costs no measurable CPU. A call never sleeps past its
/// deadline: the last sleep is shortened to the time left.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// How long [`run_bounded`] waits, after killing a timed-out child, for the
/// readers to hand back what the child had written.
///
/// A grandchild that inherited the pipes can keep them open after the child is
/// dead; the readers are then abandoned (they end when the pipes close) rather
/// than waited for, so a timeout is never held up by one.
const KILLED_READER_GRACE: Duration = Duration::from_millis(100);

/// Saturation for a timeout too large to add to an [`Instant`]: far enough that no
/// real call reaches it (the same bound as `Deadline::after`).
const FAR_FUTURE: Duration = Duration::from_hours(100 * 365 * 24);

/// Why a bounded invocation produced no output.
#[derive(Debug)]
#[non_exhaustive]
pub enum BoundedError {
    /// The time limit ran out (while waiting for the project lock, for the child,
    /// or for its output). A running child was killed.
    TimedOut,
    /// The call could not be made: the process could not be started or waited on,
    /// or the project reference is not supported. The cause is kept as the
    /// [`BeadsError`] (for a spawn failure, with its `io::Error` source).
    Failed(BeadsError),
}

impl std::fmt::Display for BoundedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimedOut => f.write_str("timed out"),
            Self::Failed(e) => std::fmt::Display::fmt(e, f),
        }
    }
}

impl std::error::Error for BoundedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TimedOut => None,
            Self::Failed(e) => Some(e),
        }
    }
}

impl From<BeadsError> for BoundedError {
    fn from(e: BeadsError) -> Self {
        Self::Failed(e)
    }
}

/// Runs `<binary> <argv…>` in `working_dir` under that directory's project lock and
/// stops at `timeout`.
///
/// The environment is the one of [`spawn_json`](crate::run::spawn_json): the extended
/// `PATH` and `BEADS_PATH` set to the working directory. Standard input is closed.
/// Any exit status is returned as `Ok`; the caller classifies it.
///
/// # Errors
///
/// - [`BoundedError::TimedOut`] when the lock, the child or its output is not ready
///   within `timeout`; a running child is killed and reaped first.
/// - [`BoundedError::Failed`] with [`BeadsError::Spawn`] (`operation` = the first
///   argument) when the process or a reader thread cannot be started, or waiting for
///   the child fails.
pub fn run_bounded(
    binary: &str,
    locks: &ProjectLocks,
    working_dir: &str,
    argv: &[String],
    timeout: Duration,
) -> Result<CliOutput, BoundedError> {
    let start = Instant::now();
    let limit = start
        .checked_add(timeout)
        .or_else(|| start.checked_add(FAR_FUTURE))
        .unwrap_or(start);
    let spawn_error = |source: std::io::Error| {
        BoundedError::Failed(BeadsError::Spawn {
            binary: binary.to_owned(),
            operation: argv.first().cloned(),
            source,
        })
    };

    // The lock serializes bd processes on one project (embedded Dolt crashes when
    // two run at once). It is taken by polling so the wait is bounded; a poisoned
    // lock is recovered because the guarded unit holds no state.
    let project_lock = locks.guard(working_dir);
    let _guard = loop {
        match project_lock.try_lock() {
            Ok(guard) => break guard,
            Err(TryLockError::Poisoned(poisoned)) => break poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => wait_or_time_out(limit)?,
        }
    };

    let mut child = new_command(binary)
        .args(argv)
        .current_dir(working_dir)
        .env("PATH", get_extended_path())
        .env("BEADS_PATH", working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(spawn_error)?;

    let stdout = match drain(child.stdout.take()) {
        Ok(rx) => rx,
        Err(e) => return Err(abort(&mut child, spawn_error(e))),
    };
    let stderr = match drain(child.stderr.take()) {
        Ok(rx) => rx,
        Err(e) => return Err(abort(&mut child, spawn_error(e))),
    };

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if let Err(timed_out) = wait_or_time_out(limit) {
                    kill_and_reap(&mut child);
                    // Best effort: give the readers a moment to hand back what was
                    // written, only so it can be logged; the call has failed.
                    let _ = stdout.recv_timeout(KILLED_READER_GRACE);
                    return Err(timed_out);
                }
            }
            Err(e) => return Err(abort(&mut child, spawn_error(e))),
        }
    };

    let collect = |rx: &Receiver<Vec<u8>>| match rx
        .recv_timeout(limit.saturating_duration_since(Instant::now()))
    {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        // A reader that has not finished by the limit is held open by a process
        // the child left behind; the readers end when that process closes the pipe.
        Err(RecvTimeoutError::Timeout) => Err(BoundedError::TimedOut),
        // A reader thread that ended without sending died on a read error; its
        // bytes are lost, and an incomplete answer is not an answer.
        Err(RecvTimeoutError::Disconnected) => Err(spawn_error(std::io::Error::other(
            "reading the child's output failed",
        ))),
    };
    let stdout = collect(&stdout)?;
    let stderr = collect(&stderr)?;
    Ok(CliOutput {
        status: status.code(),
        success: status.success(),
        stdout,
        stderr,
    })
}

/// Sleeps one poll interval, or fails with [`BoundedError::TimedOut`] when `limit`
/// has been reached.
fn wait_or_time_out(limit: Instant) -> Result<(), BoundedError> {
    let left = limit.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(BoundedError::TimedOut);
    }
    thread::sleep(left.min(POLL_INTERVAL));
    Ok(())
}

/// Starts a thread that reads `pipe` to the end and sends the bytes.
///
/// The returned receiver yields nothing if the read fails (the sender is dropped).
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::io::Result<Receiver<Vec<u8>>> {
    let (tx, rx) = mpsc::channel();
    let Some(mut pipe) = pipe else {
        // Both pipes were requested with `Stdio::piped()`; an absent one yields empty output.
        let _ = tx.send(Vec::new());
        return Ok(rx);
    };
    thread::Builder::new()
        .name("sc-bd-cli-reader".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            if pipe.read_to_end(&mut bytes).is_ok() {
                // The receiver is gone when the call timed out; nothing to report.
                let _ = tx.send(bytes);
            }
        })?;
    Ok(rx)
}

/// Kills the child and waits for it, ignoring errors from a child that already exited.
fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Cleans up a child after an internal failure and passes the error through.
fn abort(child: &mut Child, error: BoundedError) -> BoundedError {
    kill_and_reap(child);
    error
}
