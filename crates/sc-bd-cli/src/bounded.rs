//! Deadline-bounded process invocation for the cache calls (NFR-BDC-0004).
//!
//! btit's [`run::spawn_json`](crate::run::spawn_json) blocks until the child exits
//! (`Command::output()`), which is right for an app that shows a spinner and wrong
//! for a cache tick that must stop at its deadline. [`run_bounded`] is the
//! bounded counterpart used by [`CliInvoker::run_bounded`](crate::CliInvoker::run_bounded):
//!
//! - the project lock is acquired by polling, never by blocking, so a wait behind a
//!   slow call ends at the deadline too ([`TimeoutPhase::Lock`]);
//! - the child is polled with `try_wait` and killed (then reaped) when the time is
//!   up ([`TimeoutPhase::Child`]);
//! - its output is drained by two reader threads, so a chatty child cannot fill a
//!   pipe and stall, the wait for those threads is bounded by the same deadline
//!   ([`TimeoutPhase::Output`]), and each stream is capped at [`MAX_OUTPUT_BYTES`]
//!   (the excess is drained and dropped, and the call fails with
//!   [`BoundedError::Output`]).
//!
//! The time limit is a [`Duration`] measured on the real clock from the moment the
//! function is entered. The caller turns its [`Deadline`](sc_beads::Deadline) into
//! that duration with the shared [`Clock`](sc_beads::Clock), so a test clock decides
//! whether a deadline has *already* passed while real elapsed time decides when a
//! running child is stopped.

use std::io::{self, Read};
use std::process::{Child, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::TryLockError;
use std::thread;
use std::time::{Duration, Instant};

use sc_beads::error::BeadsError;
use sc_beads::CliOutput;

use crate::command::new_command;
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
///   within `timeout`; a running child is killed and reaped first.
/// - [`BoundedError::Failed`] with [`BeadsError::Spawn`] (`operation` = the first
///   argument) when the process cannot be started or waiting for it fails.
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
    let start = Instant::now();
    let limit = start
        .checked_add(timeout)
        .or_else(|| start.checked_add(FAR_FUTURE))
        .unwrap_or(start);
    let spawn_error = |source: io::Error| {
        BoundedError::Failed(BeadsError::Spawn {
            binary: binary.to_owned(),
            operation: argv.first().cloned(),
            source,
        })
    };

    // The lock serializes bd processes on one project (embedded Dolt crashes when
    // two run at once). It is taken by polling so the wait is bounded; a poisoned
    // lock is recovered because the guarded unit holds no state.
    let project_lock = locks.guard_dir(working_dir);
    let _guard = loop {
        match project_lock.try_lock() {
            Ok(guard) => break guard,
            Err(TryLockError::Poisoned(poisoned)) => break poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => wait_or_time_out(limit, TimeoutPhase::Lock)?,
        }
    };

    let mut child = new_command(binary)
        .args(argv)
        .current_dir(working_dir.as_str())
        .env("PATH", get_extended_path())
        .env("BEADS_PATH", working_dir.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(spawn_error)?;

    let stdout = match drain(child.stdout.take(), max_output) {
        Ok(rx) => rx,
        Err(e) => return Err(abort(&mut child, BoundedError::Output(e))),
    };
    let stderr = match drain(child.stderr.take(), max_output) {
        Ok(rx) => rx,
        Err(e) => return Err(abort(&mut child, BoundedError::Output(e))),
    };

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if let Err(timed_out) = wait_or_time_out(limit, TimeoutPhase::Child) {
                    // The readers are abandoned: they end when the pipes close, even
                    // if a process the child left behind holds them.
                    kill_and_reap(&mut child);
                    return Err(timed_out);
                }
            }
            Err(e) => return Err(abort(&mut child, spawn_error(e))),
        }
    };

    let collect = |rx: &Receiver<io::Result<Vec<u8>>>| {
        match rx.recv_timeout(limit.saturating_duration_since(Instant::now())) {
            Ok(Ok(bytes)) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Ok(Err(e)) => Err(BoundedError::Output(e)),
            // A reader still running at the limit is held open by a process the
            // child left behind; it ends when that process closes the pipe.
            Err(RecvTimeoutError::Timeout) => Err(BoundedError::TimedOut {
                phase: TimeoutPhase::Output,
            }),
            Err(RecvTimeoutError::Disconnected) => Err(BoundedError::Output(io::Error::other(
                "the reader thread ended without a result",
            ))),
        }
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

/// Kills the child and waits for it, ignoring errors from a child that already exited.
///
/// After it returns the child is reaped and its pipes are closed.
pub fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Cleans up a child after an internal failure and passes the error through.
fn abort(child: &mut Child, error: BoundedError) -> BoundedError {
    kill_and_reap(child);
    error
}
