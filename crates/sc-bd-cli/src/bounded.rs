//! Deadline-bounded process invocation for the cache calls (NFR-BDC-0004).
//!
//! btit's [`run::spawn_json`](crate::run::spawn_json) blocks until the child exits
//! (`Command::output()`), which is right for an app that shows a spinner and wrong
//! for a cache tick that must stop at its deadline. [`run_bounded`] is the
//! bounded counterpart used by [`CliInvoker::run_bounded`](crate::CliInvoker::run_bounded):
//!
//! - the project lock is acquired by polling, never by blocking, so a wait behind a
//!   slow call ends at the deadline too ([`TimeoutPhase::Lock`]); it is the
//!   in-process mutex of [`ProjectLocks`] and then the project's cross-process lock
//!   file, so two processes never run `bd` on one project at once either (lead ruling
//!   S81, RSH-004, ADR-BDCLI-0006);
//! - the child is started as the leader of its own process group (Unix) or inside a
//!   job object of its own (Windows) and polled for its exit; when the time is up the
//!   whole group or job is killed, so a `dolt` that `bd` started dies with it
//!   ([`TimeoutPhase::Child`], PE-RSH-001), and the project lock is held until that
//!   kill has finished or [`KILL_BOUND`] has passed;
//! - the kill never names a process that may already be gone (RCR-001): on Unix the
//!   exit is noticed with `waitid(WNOWAIT)`, which leaves the child unreaped, so its
//!   pid, the group's id, cannot be reused until the kill is done; on Windows the job
//!   is killed through its handle, not through a pid;
//! - a killed child is reaped by polling for at most [`KILL_BOUND`]; one that is still
//!   there is handed to a detached reaper thread instead of being waited for
//!   (PE-RSH-002);
//! - its output is drained by two reader threads, so a chatty child cannot fill a
//!   pipe and stall, the wait for those threads is bounded by the same deadline
//!   ([`TimeoutPhase::Output`], after which the group or job the child left behind is
//!   killed too, before the child is reaped), and each stream is capped at [`MAX_OUTPUT_BYTES`]
//!   (the excess is drained and dropped, and the call fails with
//!   [`BoundedError::OutputTooLarge`]);
//! - on Unix the group is killed with `kill(2)` itself ([`process_group`]), never by
//!   running a `kill` program that a minimal image may not have (RSH-002); on Linux the
//!   child also gets `PR_SET_PDEATHSIG` (`SIGKILL`), so it does not outlive a host
//!   that is killed outright;
//! - a shutdown the host asked for ([`shutdown::request`], from its signal handler)
//!   stops every call as a timeout would, with [`BoundedError::Interrupted`]
//!   (RSH-001).
//!
//! The time limit is a [`Duration`] measured on the real clock from the moment the
//! function is entered. The caller turns its [`Deadline`](sc_beads::Deadline) into
//! that duration with the shared [`Clock`](sc_beads::Clock), so a test clock decides
//! whether a deadline has *already* passed while real elapsed time decides when a
//! running child is stopped.

use std::io::{self, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{OnceLock, TryLockError};
use std::thread;
use std::time::{Duration, Instant};

use sc_beads::error::BeadsError;
use sc_beads::{log_error, log_warn};
use sc_beads::{CliClient, CliOutput};

use crate::command::{lead_process_group, ChildEnv};
use crate::locks::{ProjectLocks, WorkingDir};
use crate::path::extended_path_os;
#[cfg(unix)]
use crate::process_group;
use crate::shutdown;

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
/// the call fails with [`BoundedError::OutputTooLarge`].
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

/// Where a bounded call was when its time ran out (or a shutdown stopped it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeoutPhase {
    /// Waiting for the per-project lock (the in-process mutex or the cross-process
    /// lock file); no child was started.
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

/// One of a child's two output streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

impl OutputStream {
    /// The stream's conventional name: `stdout` or `stderr`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

impl std::fmt::Display for OutputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a bounded invocation produced no output.
///
/// The enum is `#[non_exhaustive]`: [`Wait`](Self::Wait) and
/// [`OutputTooLarge`](Self::OutputTooLarge) were added after the first release
/// (PE-RBP-F012), [`Interrupted`](Self::Interrupted) with RSH-001, and a match outside
/// this crate needs a wildcard arm.
#[derive(Debug)]
#[non_exhaustive]
pub enum BoundedError {
    /// The time limit ran out in this phase. A running child was killed.
    TimedOut {
        /// What the call was waiting for.
        phase: TimeoutPhase,
    },
    /// The host asked the process to stop ([`shutdown::request`], RSH-001) while the
    /// call was in this phase. A running child was killed with its process group (job)
    /// and reaped, as on a timeout; in [`TimeoutPhase::Lock`] no child was started.
    Interrupted {
        /// What the call was doing.
        phase: TimeoutPhase,
        /// The signal the host recorded with the request.
        signal: i32,
    },
    /// The call could not be made: the process could not be started, or the project
    /// reference is not supported. The cause is kept as the [`BeadsError`] (for a
    /// spawn failure, with its `io::Error` source).
    Failed(BeadsError),
    /// The child was started, but asking whether it had exited, or reaping it, failed
    /// (PE-RBP-F012). The child's process group (job) was killed. The child may have
    /// run, so what it did is unknown; the OS error is the source.
    Wait(io::Error),
    /// Reading the child's output failed, or a reader thread could not be started or
    /// ended without a result. The read error is the source. A stream over the cap
    /// is [`OutputTooLarge`](Self::OutputTooLarge) instead.
    Output(io::Error),
    /// A stream of the child's output was longer than `limit` bytes (by default
    /// [`MAX_OUTPUT_BYTES`]); the rest was drained and dropped (PE-RBP-F012).
    OutputTooLarge {
        /// The stream that was over the cap.
        stream: OutputStream,
        /// The cap, in bytes.
        limit: usize,
    },
}

impl std::fmt::Display for BoundedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimedOut { phase } => write!(f, "timed out {phase}"),
            Self::Interrupted { phase, signal } => {
                write!(f, "stopped {phase}: the host is shutting down (signal {signal})")
            }
            Self::Failed(e) => std::fmt::Display::fmt(e, f),
            Self::Wait(e) => write!(f, "waiting for the child failed: {e}"),
            Self::Output(e) => write!(f, "reading the child's output failed: {e}"),
            Self::OutputTooLarge { stream, limit } => {
                write!(f, "the child's {stream} exceeds the cap of {limit} bytes")
            }
        }
    }
}

impl std::error::Error for BoundedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TimedOut { .. } | Self::Interrupted { .. } | Self::OutputTooLarge { .. } => {
                None
            }
            Self::Failed(e) => Some(e),
            Self::Wait(e) | Self::Output(e) => Some(e),
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
///   argument) when the process cannot be started, and with
///   [`BeadsError::Unsupported`] when the canonical path of `working_dir` is not UTF-8
///   ([`ProjectLocks::try_guard_dir`]).
/// - [`BoundedError::Wait`] when the started child cannot be polled for its exit or
///   reaped; its process group (job) is killed first.
/// - [`BoundedError::Output`] when a reader thread cannot be started or a read fails.
/// - [`BoundedError::OutputTooLarge`] when a stream exceeds `max_output`.
/// - [`BoundedError::Interrupted`] when the host asked the process to stop
///   ([`shutdown::request`]); a running child is killed as on a timeout.
/// - [`BoundedError::Failed`] with [`BeadsError::Spawn`] when the project's
///   cross-process lock file can be neither opened nor locked (nothing was started).
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

/// [`run_bounded`] that also counts as timed out once `expire` is set: a test's
/// injected deadline, so a timeout can be ordered after what the test waits for
/// (FTQ-002) instead of racing a fixed duration.
///
/// `timeout` stays the fail-safe: the call ends at whichever comes first. Only built
/// with the `test-support` feature.
///
/// # Errors
///
/// As [`run_bounded_capped`]; a set `expire` is [`BoundedError::TimedOut`] in the phase
/// the call is in.
#[cfg(feature = "test-support")]
pub fn run_bounded_expiring(
    binary: &str,
    locks: &ProjectLocks,
    working_dir: &WorkingDir,
    argv: &[String],
    timeout: Duration,
    expire: &AtomicBool,
) -> Result<CliOutput, BoundedError> {
    let mut limit = Limit::after(timeout);
    limit.expire = Some(expire);
    run_bounded_until(
        binary,
        locks,
        working_dir,
        argv,
        limit,
        MAX_OUTPUT_BYTES,
        &ChildEnv::default(),
    )
}

/// When a bounded call must stop: its time limit, a shutdown request
/// ([`shutdown::requested`]) or, in a test, an injected expiry.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limit<'a> {
    /// The instant the call's time is up.
    at: Instant,
    /// Counts as `at` having passed once set (a test's injected deadline).
    expire: Option<&'a AtomicBool>,
}

/// Why a bounded call was cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cut {
    /// Its time ran out.
    Deadline,
    /// The host is shutting down, on this signal.
    Shutdown(i32),
}

impl Cut {
    /// The error of a call cut in `phase`.
    fn error(self, phase: TimeoutPhase) -> BoundedError {
        match self {
            Self::Deadline => BoundedError::TimedOut { phase },
            Self::Shutdown(signal) => BoundedError::Interrupted { phase, signal },
        }
    }
}

impl Limit<'static> {
    /// The limit at the instant `at`.
    pub(crate) fn at(at: Instant) -> Self {
        Self { at, expire: None }
    }

    /// The limit `timeout` from now, saturated far in the future.
    pub(crate) fn after(timeout: Duration) -> Self {
        let start = Instant::now();
        let at = start
            .checked_add(timeout)
            .or_else(|| start.checked_add(FAR_FUTURE))
            .unwrap_or(start);
        Self::at(at)
    }
}

impl Limit<'_> {
    /// Why the call must stop now, if it must: a shutdown first, then the deadline.
    fn cut(&self) -> Option<Cut> {
        if let Some(signal) = shutdown::requested() {
            return Some(Cut::Shutdown(signal));
        }
        let expired = self.expire.is_some_and(|e| e.load(Ordering::SeqCst));
        (expired || Instant::now() >= self.at).then_some(Cut::Deadline)
    }

    /// The time left before `at` (zero once an injected expiry is set).
    fn left(&self) -> Duration {
        if self.expire.is_some_and(|e| e.load(Ordering::SeqCst)) {
            return Duration::ZERO;
        }
        self.at.saturating_duration_since(Instant::now())
    }
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
    run_bounded_until(
        binary,
        locks,
        working_dir,
        argv,
        Limit::after(timeout),
        max_output,
        env,
    )
}

/// [`run_bounded_in`] stopped at `limit`.
fn run_bounded_until(
    binary: &str,
    locks: &ProjectLocks,
    working_dir: &WorkingDir,
    argv: &[String],
    limit: Limit<'_>,
    max_output: usize,
    env: &ChildEnv,
) -> Result<CliOutput, BoundedError> {
    // A process that is shutting down starts no new `bd` (RSH-001).
    if let Some(signal) = shutdown::requested() {
        return Err(Cut::Shutdown(signal).error(TimeoutPhase::Lock));
    }
    // Two locks serialize bd processes on one project (embedded Dolt crashes when two
    // run at once): the in-process mutex, which orders this process's threads, and
    // then the project's lock file, which orders processes (lead ruling S81, RSH-004).
    // Both are taken by polling so the wait is bounded; a poisoned mutex is recovered
    // because the guarded unit holds no state.
    let (project_lock, file_lock) = locks.try_guard_and_file(working_dir).map_err(|e| {
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
    // Dropped (closing the file releases it) on every return below, after the
    // child's tree was killed and reaped: a timeout, a shutdown, a failure.
    let _file_guard = loop {
        match file_lock.try_acquire() {
            Ok(Some(guard)) => break guard,
            Ok(None) => wait_or_time_out(limit, TimeoutPhase::Lock)?,
            Err(source) => {
                log_error!(
                    "[bd] cannot take the cross-process lock of {}: {}",
                    working_dir,
                    source
                );
                return Err(BoundedError::Failed(BeadsError::Spawn {
                    binary: binary.to_owned(),
                    operation: argv.first().cloned(),
                    source: io::Error::new(
                        source.kind(),
                        format!("the cross-process project lock is not usable: {source}"),
                    ),
                }));
            }
        }
    };

    let mut command = env.command(binary);
    command
        .args(argv)
        .current_dir(working_dir.as_str())
        .env("PATH", extended_path_os())
        .env("BEADS_PATH", working_dir.as_str());
    // The guards outlive this call: every kill `run_child` makes on a timeout has
    // finished, or reached `KILL_BOUND`, before the next call on the project starts.
    run_child_until(
        command,
        binary,
        argv.first().map(String::as_str),
        limit,
        max_output,
        has_exited,
    )
}

/// Spawns `command` as the leader of its own process group (inside a job object of
/// its own on Windows), with standard input closed and both outputs drained, and
/// stops it at `limit`.
///
/// On a timeout of the child the whole group (job) is killed and the child reaped
/// (within [`KILL_BOUND`]) before this returns; on a timeout of the output (the child
/// exited, a process it left behind holds a pipe open) the group (job) it left behind
/// is killed the same way, and only then is the child reaped (RCR-001).
/// `binary` and `operation` name the spawn in a [`BeadsError::Spawn`].
pub(crate) fn run_child(
    command: Command,
    binary: &str,
    operation: Option<&str>,
    limit: Instant,
    max_output: usize,
) -> Result<CliOutput, BoundedError> {
    run_child_with(command, binary, operation, limit, max_output, has_exited)
}

/// [`run_child`] with the exit check `exited` in place of [`has_exited`]: the seam
/// a test uses to make the wait fail, which no real child can be made to do.
fn run_child_with(
    command: Command,
    binary: &str,
    operation: Option<&str>,
    limit: Instant,
    max_output: usize,
    exited: fn(&mut Child) -> io::Result<bool>,
) -> Result<CliOutput, BoundedError> {
    run_child_until(
        command,
        binary,
        operation,
        Limit::at(limit),
        max_output,
        exited,
    )
}

/// [`run_child_with`] stopped at `limit`, which a shutdown or a test's expiry can
/// bring forward.
fn run_child_until(
    mut command: Command,
    binary: &str,
    operation: Option<&str>,
    limit: Limit<'_>,
    max_output: usize,
    exited: fn(&mut Child) -> io::Result<bool>,
) -> Result<CliOutput, BoundedError> {
    let spawn_error = |source: io::Error| {
        BoundedError::Failed(BeadsError::Spawn {
            binary: binary.to_owned(),
            operation: operation.map(str::to_owned),
            source,
        })
    };
    lead_process_group(&mut command);
    #[cfg(target_os = "linux")]
    die_with_parent(&mut command);
    #[cfg(windows)]
    job::start_suspended(&mut command);
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(spawn_error)?;
    let mut spawned = Spawned::start(child).map_err(spawn_error)?;

    let stdout = match drain(
        spawned.child.stdout.take(),
        OutputStream::Stdout,
        max_output,
    ) {
        Ok(rx) => rx,
        Err(e) => return Err(abort(spawned, BoundedError::Output(e))),
    };
    let stderr = match drain(
        spawned.child.stderr.take(),
        OutputStream::Stderr,
        max_output,
    ) {
        Ok(rx) => rx,
        Err(e) => return Err(abort(spawned, BoundedError::Output(e))),
    };

    loop {
        match exited(&mut spawned.child) {
            Ok(true) => break,
            Ok(false) => {
                if let Err(cut) = wait_or_time_out(limit, TimeoutPhase::Child) {
                    // A child that exited by the cut is not killed as a running one:
                    // its output is read, and the cut is then seen there (Output).
                    if matches!(exited(&mut spawned.child), Ok(true)) {
                        break;
                    }
                    // The readers are abandoned: they end when the pipes close, which
                    // the group kill makes happen.
                    terminate(spawned);
                    return Err(cut);
                }
            }
            // The child is running or gone, not unstarted: a wait failure, never a
            // spawn failure (PE-RBP-F012).
            Err(e) => return Err(abort(spawned, BoundedError::Wait(e))),
        }
    }

    // The child has exited but is not reaped yet (on Unix), so the group it led
    // keeps its id until `terminate` or `finish` reaps it.
    let output = collect(&stdout, limit).and_then(|out| Ok((out, collect(&stderr, limit)?)));
    if let Err(timed_out @ (BoundedError::TimedOut { .. } | BoundedError::Interrupted { .. })) =
        output
    {
        // A reader still open at the limit is held by a process the child left in
        // its group (job): kill that too, so it does not outlive the project lock.
        return Err(abort(spawned, timed_out));
    }
    let status = spawned.finish().map_err(BoundedError::Wait)?;
    let (stdout, stderr) = output?;
    Ok(CliOutput {
        status: status.code(),
        success: status.success(),
        stdout,
        stderr,
    })
}

/// A started child with what a kill of its process tree is aimed at.
struct Spawned {
    child: Child,
    tree: Tree,
}

/// What a kill of a child's process tree is aimed at.
struct Tree {
    /// The child's pid: on Unix also the id of the process group it leads.
    pid: u32,
    /// The job object holding the child and every process it starts (Windows), when
    /// one could be set up; without one the tree is killed with `taskkill /T`.
    #[cfg(windows)]
    job: Option<job::Job>,
}

impl Spawned {
    /// Takes a child that was just spawned. On Windows it was started suspended: it is
    /// put in a new job object, then resumed, so nothing it starts escapes the job.
    ///
    /// # Errors
    ///
    /// On Windows, when the suspended child cannot be resumed; it is then killed and
    /// reaped (or handed to the reaper) first.
    #[cfg_attr(
        not(windows),
        expect(clippy::unnecessary_wraps, reason = "only the Windows resume can fail")
    )]
    fn start(child: Child) -> io::Result<Self> {
        let pid = child.id();
        #[cfg(windows)]
        {
            let job = match job::Job::new().and_then(|job| job.assign(&child).map(|()| job)) {
                Ok(job) => Some(job),
                Err(e) => {
                    log_warn!(
                        "[bd] no job object for the child {}: {}; a timeout kills its tree with taskkill",
                        pid,
                        e
                    );
                    None
                }
            };
            if let Err(e) = job::resume(&child) {
                let mut child = child;
                let _ = child.kill();
                reap_or_hand_off(child, Instant::now() + KILL_BOUND);
                return Err(e);
            }
            Ok(Self {
                child,
                tree: Tree { pid, job },
            })
        }
        #[cfg(not(windows))]
        Ok(Self {
            child,
            tree: Tree { pid },
        })
    }

    /// Reaps the child, which has exited, and returns its status. On Windows the job
    /// stops killing what is left in it when it is closed, so a process the child left
    /// running (that holds none of its pipes) outlives the call, as it does on Unix.
    fn finish(self) -> io::Result<std::process::ExitStatus> {
        let Self { mut child, tree } = self;
        match child.try_wait() {
            Ok(Some(status)) => {
                tree.release();
                Ok(status)
            }
            Ok(None) => {
                terminate(Self { child, tree });
                Err(io::Error::other(
                    "the child was seen to exit but could not be reaped",
                ))
            }
            Err(e) => Err(e),
        }
    }
}

impl Tree {
    /// Lets the processes in the tree outlive it (Windows: clears the job's
    /// kill-on-close); nothing to do elsewhere.
    fn release(&self) {
        #[cfg(windows)]
        if let Some(job) = &self.job {
            if let Err(e) = job.kill_on_close(false) {
                log_warn!(
                    "[bd] the job of {} still kills what is left in it: {}",
                    self.pid,
                    e
                );
            }
        }
        #[cfg(not(windows))]
        let _ = self;
    }
}

/// Whether `child` has exited, without reaping it on Unix (RCR-001).
///
/// On Unix this is `waitid(P_PID, pid, WEXITED | WNOHANG | WNOWAIT)`: the child
/// stays a zombie until [`Child::try_wait`] reaps it, and POSIX does not reuse a
/// process id, nor a process group id equal to it, while that process exists (XBD
/// 4.14, "Process ID Reuse"). So a group kill sent before the reap names the group
/// the child led, never another one. Elsewhere `try_wait` is used: on Windows the
/// [`Child`] keeps its process handle open after `try_wait`, which keeps the pid
/// from being reused, and the tree is killed through the job anyway.
#[cfg(unix)]
fn has_exited(child: &mut Child) -> io::Result<bool> {
    let pid = libc::id_t::from(child.id());
    loop {
        // SAFETY: `siginfo_t` is a C struct of integers and raw pointers, for which
        // all zero bytes is a valid value.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: `info` is a valid, writable `siginfo_t` that outlives the call, and
        // `waitid` keeps no pointer to it. `WNOWAIT` leaves the child waitable, so the
        // later `try_wait` still reaps it and std's view of the child stays right.
        let rc = unsafe {
            libc::waitid(
                libc::P_PID,
                pid,
                &raw mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if rc == 0 {
            // With `WNOHANG` and no exited child the structure is left as it was
            // (zeroed here); one that has exited fills in `si_signo` (`SIGCHLD`).
            return Ok(info.si_signo != 0);
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// Whether `child` has exited (see the Unix variant on why this may reap here).
#[cfg(not(unix))]
fn has_exited(child: &mut Child) -> io::Result<bool> {
    child.try_wait().map(|status| status.is_some())
}

/// The output one reader thread sent, waited for until `limit`, one poll interval
/// at a time so a shutdown is seen while waiting.
fn collect(rx: &Receiver<Drained>, limit: Limit<'_>) -> Result<String, BoundedError> {
    loop {
        match rx.recv_timeout(limit.left().min(POLL_INTERVAL)) {
            Ok(Ok(bytes)) => return Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Ok(Err(e)) => return Err(e),
            Err(RecvTimeoutError::Timeout) => {
                if let Some(cut) = limit.cut() {
                    return Err(cut.error(TimeoutPhase::Output));
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(BoundedError::Output(io::Error::other(
                    "the reader thread ended without a result",
                )))
            }
        }
    }
}

/// Sleeps one poll interval, or fails with [`BoundedError::TimedOut`] in `phase` when
/// `limit` has been reached ([`BoundedError::Interrupted`] on a shutdown).
fn wait_or_time_out(limit: Limit<'_>, phase: TimeoutPhase) -> Result<(), BoundedError> {
    if let Some(cut) = limit.cut() {
        return Err(cut.error(phase));
    }
    thread::sleep(limit.left().min(POLL_INTERVAL));
    Ok(())
}

/// Linux: the child gets `SIGKILL` when the thread that spawned it dies
/// (`PR_SET_PDEATHSIG`), so a host killed outright (no handler can run on `SIGKILL`)
/// does not leave its `bd` running (RSH-001).
///
/// The "parent" of `PR_SET_PDEATHSIG` is the spawning thread, not the process; that is
/// safe here because the thread that spawns a bounded child polls it until it has
/// exited or been killed ([`run_child_until`]), so it never ends first. A grandchild
/// (`dolt`) is not covered: it is reached by the group kill. A host that died between
/// the fork and the `prctl` is caught by comparing the parent pid, and the spawn
/// fails instead of leaving an orphan.
#[cfg(target_os = "linux")]
fn die_with_parent(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    // SAFETY: `getpid` has no preconditions and touches no memory.
    let parent = unsafe { libc::getpid() };
    let hook = move || {
        // SAFETY: `prctl(PR_SET_PDEATHSIG, sig)` takes integers only and is
        // async-signal-safe, as a `pre_exec` hook must be (it runs between `fork` and
        // `exec`); `PR_SET_PDEATHSIG` reads no further arguments.
        if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `getppid` has no preconditions and is async-signal-safe.
        if unsafe { libc::getppid() } != parent {
            // The host died before the `prctl`: no death signal will come.
            return Err(io::Error::from_raw_os_error(libc::ESRCH));
        }
        Ok(())
    };
    // SAFETY: the hook allocates nothing, takes no lock and only makes the two
    // async-signal-safe calls above, so it is sound in the forked child before `exec`.
    unsafe {
        command.pre_exec(hook);
    }
}

/// What a reader thread sends: the stream's bytes, or why there are none
/// ([`BoundedError::Output`] or [`BoundedError::OutputTooLarge`]).
type Drained = Result<Vec<u8>, BoundedError>;

/// Starts a thread that reads `pipe` (the child's `stream`) to the end and sends what
/// it read, or [`BoundedError::Output`] with the read error, or
/// [`BoundedError::OutputTooLarge`] when the stream is longer than `max_output`.
fn drain<R: Read + Send + 'static>(
    pipe: Option<R>,
    stream: OutputStream,
    max_output: usize,
) -> io::Result<Receiver<Drained>> {
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
                    Err(BoundedError::OutputTooLarge {
                        stream,
                        limit: max_output,
                    })
                }
                Ok(_) => Ok(bytes),
                Err(e) => Err(BoundedError::Output(e)),
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

/// Cleans up a child after a timeout of its output or an internal failure, and
/// passes the error through.
fn abort(spawned: Spawned, error: BoundedError) -> BoundedError {
    terminate(spawned);
    error
}

/// Kills the process group (Windows: the job) the child leads, then the child itself,
/// and reaps it, all within one [`KILL_BOUND`] from now (PE-RSH-001).
///
/// Only called while the child has not been reaped, whether it is still running or
/// has exited ([`has_exited`] does not reap on Unix), so its pid, the group's id,
/// cannot name another process or group (RCR-001). After a group kill that ran, it
/// also waits (within the same bound) until the group or job has no member left, so
/// a descendant is gone, not only signalled, when the caller releases the project
/// lock.
fn terminate(spawned: Spawned) {
    let Spawned { mut child, tree } = spawned;
    let until = Instant::now() + KILL_BOUND;
    let group = kill_group(&tree, until);
    // The group kill may have failed or be absent on this platform: kill the child
    // itself too.
    let _ = child.kill();
    if reap_or_hand_off(child, until) == Reaped::Exited && group == Helper::Done {
        wait_until_group_is_gone(&tree, until);
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

/// How the kill of a child's tree ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Helper {
    /// It was sent (Unix: `kill(2)` succeeded; Windows: the job was terminated or
    /// `taskkill` succeeded).
    Done,
    /// It failed, or the platform has no tree kill.
    Failed,
    /// Windows `taskkill` was still running at the bound: it was killed and handed to
    /// the reaper.
    #[cfg(windows)]
    GaveUp,
}

/// Runs a helper program (Windows `taskkill`) until `until` at most, with no input
/// or output.
#[cfg(windows)]
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

/// Kills the process group led by the child (Unix), or its job (Windows; without one
/// the process tree rooted at it), within `until`. A failure is logged; the caller
/// kills the child itself regardless.
///
/// On Unix this is `kill(-pid, SIGKILL)` through [`process_group::kill`]: no program is
/// started, so a host without `/bin/kill` on its `PATH`, or one that cannot start a
/// process at that moment, still kills the whole group (RSH-002).
fn kill_group(tree: &Tree, until: Instant) -> Helper {
    let pid = tree.pid;
    #[cfg(windows)]
    if let Some(job) = &tree.job {
        // Through the job's handle, not a pid: it reaches every process the child
        // started, whether or not the child still runs.
        return match job.terminate() {
            Ok(()) => Helper::Done,
            Err(e) => {
                log_warn!("[bd] terminating the job of {} failed: {}", pid, e);
                Helper::Failed
            }
        };
    }
    #[cfg(unix)]
    let outcome = {
        let _ = until;
        match process_group::kill(pid) {
            Ok(()) => Helper::Done,
            Err(e) => {
                log_warn!("[bd] kill(-{}, SIGKILL) failed: {}", pid, e);
                Helper::Failed
            }
        }
    };
    #[cfg(windows)]
    let outcome = {
        let mut kill = crate::command::new_command("taskkill");
        kill.args(["/T", "/F", "/PID", &pid.to_string()]);
        run_helper(kill, until)
    };
    #[cfg(not(any(unix, windows)))]
    let outcome = {
        let _ = until;
        Helper::Failed
    };
    if outcome != Helper::Done {
        log_warn!(
            "[bd] killing the process group of {} did not succeed: {:?}",
            pid,
            outcome
        );
    }
    outcome
}

/// Waits until the group led by the child (Windows: its job) has no member left, or
/// `until` has passed.
///
/// On Unix `kill(-pid, 0)` ([`process_group::exists`]) succeeds while the group has a
/// member, so a descendant the group kill signalled is known to be gone when it fails
/// with `ESRCH`. It runs after the reap (a zombie leader would count as a member);
/// signal 0 is delivered to no one, so in the residual case of a group that emptied
/// and whose id was reused meanwhile it at worst waits to `until`. On Windows the
/// job's count of active processes is polled; with no job `taskkill /F` has already
/// terminated the tree, so there is nothing to wait for.
fn wait_until_group_is_gone(tree: &Tree, until: Instant) {
    let pid = tree.pid;
    #[cfg(windows)]
    if let Some(job) = &tree.job {
        loop {
            match job.active_processes() {
                Ok(0) => return,
                Ok(_) => {}
                Err(e) => {
                    log_warn!(
                        "[bd] cannot count the processes in the job of {}: {}",
                        pid,
                        e
                    );
                    return;
                }
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                log_warn!("[bd] the job of {} outlived {:?}", pid, KILL_BOUND);
                return;
            }
            thread::sleep(left.min(POLL_INTERVAL));
        }
    }
    #[cfg(unix)]
    loop {
        match process_group::exists(pid) {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => {
                log_warn!("[bd] cannot probe the process group of {}: {}", pid, e);
                return;
            }
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

/// The Windows job object a bounded child runs in (RCR-001): the smallest wrapper
/// around the Win32 calls, each `unsafe` block with its own safety argument.
///
/// The child is created suspended, put in a new job, then resumed, so every process it
/// starts is in the job from its first instruction. `TerminateJobObject` then kills
/// the whole tree through the job's handle, whether or not the child still runs, and
/// names no pid, so no unrelated process can be hit. The job kills what is in it when
/// its last handle closes (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) while the call runs;
/// [`Tree::release`] clears that on a normal exit, so a process the child left
/// running outlives the call, as on Unix.
#[cfg(windows)]
mod job {
    use std::io;
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
    use std::os::windows::process::CommandExt as _;
    use std::process::{Child, Command};
    use std::ptr;

    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenThread, ResumeThread, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, CREATE_SUSPENDED,
        THREAD_SUSPEND_RESUME,
    };

    /// Adds `CREATE_SUSPENDED` to the flags [`lead_process_group`](crate::command::lead_process_group)
    /// set (`creation_flags` replaces, so they are repeated here).
    pub(super) fn start_suspended(command: &mut Command) {
        command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_SUSPENDED);
    }

    /// An unnamed job object, closed on drop.
    pub(super) struct Job(OwnedHandle);

    impl Job {
        /// A new job that kills its processes when it is closed.
        pub(super) fn new() -> io::Result<Self> {
            // SAFETY: both arguments may be null (default security, no name); the call
            // reads nothing else.
            let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: `raw` is a valid handle that nothing else owns: it was just created.
            let job = Self(unsafe { OwnedHandle::from_raw_handle(raw) });
            job.kill_on_close(true)?;
            Ok(job)
        }

        /// Sets or clears `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (the job's only limit).
        pub(super) fn kill_on_close(&self, on: bool) -> io::Result<()> {
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            if on {
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            }
            let len = size_u32::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()?;
            // SAFETY: the job handle is live (owned by `self`); `info` is a
            // JOBOBJECT_EXTENDED_LIMIT_INFORMATION of `len` bytes, only read during the call.
            check(unsafe {
                SetInformationJobObject(
                    self.raw(),
                    JobObjectExtendedLimitInformation,
                    (&raw const info).cast(),
                    len,
                )
            })
        }

        /// Puts `child` in the job.
        pub(super) fn assign(&self, child: &Child) -> io::Result<()> {
            // SAFETY: both handles are live for the call: the job's is owned by `self`,
            // the process's by `child` (with the full access `CreateProcess` grants).
            check(unsafe { AssignProcessToJobObject(self.raw(), child.as_raw_handle()) })
        }

        /// Terminates every process in the job.
        pub(super) fn terminate(&self) -> io::Result<()> {
            // SAFETY: the job handle is live (owned by `self`); no pointers are passed.
            check(unsafe { TerminateJobObject(self.raw(), 1) })
        }

        /// How many processes in the job have not exited.
        pub(super) fn active_processes(&self) -> io::Result<u32> {
            let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            let len = size_u32::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>()?;
            // SAFETY: the job handle is live; `info` is a writable
            // JOBOBJECT_BASIC_ACCOUNTING_INFORMATION of `len` bytes that outlives the
            // call; the returned-length pointer may be null.
            check(unsafe {
                QueryInformationJobObject(
                    self.raw(),
                    JobObjectBasicAccountingInformation,
                    (&raw mut info).cast(),
                    len,
                    ptr::null_mut(),
                )
            })?;
            Ok(info.ActiveProcesses)
        }

        fn raw(&self) -> HANDLE {
            self.0.as_raw_handle()
        }
    }

    /// Resumes the threads of `child`, which was created suspended (it has exactly
    /// one). The `Child` holds the process handle, so its pid cannot be reused while
    /// the threads are looked up.
    pub(super) fn resume(child: &Child) -> io::Result<()> {
        let pid = child.id();
        // SAFETY: no pointers are passed; the result is checked below.
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` is a valid snapshot handle that nothing else owns.
        let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut entry = THREADENTRY32 {
            dwSize: size_u32::<THREADENTRY32>()?,
            ..THREADENTRY32::default()
        };
        let mut resumed = false;
        // SAFETY: the snapshot handle is live; `entry` is a writable THREADENTRY32
        // with `dwSize` set, as the call requires.
        let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &raw mut entry) } != 0;
        while more {
            if entry.th32OwnerProcessID == pid {
                // SAFETY: no pointers are passed; the result is checked below.
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if thread.is_null() {
                    return Err(io::Error::last_os_error());
                }
                // SAFETY: `thread` is a valid handle that nothing else owns.
                let thread = unsafe { OwnedHandle::from_raw_handle(thread) };
                // SAFETY: the thread handle is live and has THREAD_SUSPEND_RESUME access.
                if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                    return Err(io::Error::last_os_error());
                }
                resumed = true;
            }
            // SAFETY: as for `Thread32First`.
            more = unsafe { Thread32Next(snapshot.as_raw_handle(), &raw mut entry) } != 0;
        }
        if resumed {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "no thread of the suspended child {pid} was found"
            )))
        }
    }

    /// A Win32 `BOOL` result as an `io::Result`.
    fn check(ok: BOOL) -> io::Result<()> {
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// The size of `T` as the `u32` a Win32 length argument takes.
    fn size_u32<T>() -> io::Result<u32> {
        u32::try_from(size_of::<T>()).map_err(io::Error::other)
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
        let killed = Command::new("kill")
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

    /// The exit check does not reap (RCR-001): a child seen to have exited is still
    /// there, a zombie, so its pid and the group it led cannot be reused before the
    /// group kill; `try_wait` reaps it afterwards.
    #[cfg(unix)]
    #[test]
    fn the_exit_check_leaves_the_child_unreaped() {
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        let until = Instant::now() + Duration::from_secs(10);
        while !has_exited(&mut child).unwrap() {
            assert!(Instant::now() < until, "the child {pid} never exited");
            thread::sleep(Duration::from_millis(5));
        }
        assert!(exists(pid), "the exit check reaped the child {pid}");
        assert!(!running(pid), "the child {pid} should be a zombie");
        assert!(has_exited(&mut child).unwrap(), "seen to exit only once");
        assert!(child.try_wait().unwrap().is_some());
        assert!(!exists(pid), "try_wait did not reap the child {pid}");
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

    /// The exit check that fails as `waitid` / `try_wait` would on an OS error.
    fn failing_exit_check(_: &mut Child) -> io::Result<bool> {
        Err(io::Error::other("injected wait failure"))
    }

    /// A child that runs far longer than the test waits.
    fn long_running() -> Command {
        #[cfg(windows)]
        {
            let mut c = Command::new("ping");
            c.args(["-n", "30", "127.0.0.1"]);
            c
        }
        #[cfg(not(windows))]
        {
            let mut c = Command::new("sleep");
            c.arg("30");
            c
        }
    }

    /// PE-RBP-F012: a started child whose exit check fails is a wait failure that
    /// keeps the OS error, never a spawn failure, and the child is killed rather than
    /// waited for.
    ///
    /// No real child can be made to fail `waitid` / `try_wait`, so the exit check is
    /// injected through `run_child_with`; spawning, draining and the kill are real.
    #[test]
    fn a_failed_exit_check_is_a_wait_error_not_a_spawn_failure() {
        let started = Instant::now();
        let r = run_child_with(
            long_running(),
            "sleep",
            Some("30"),
            started + Duration::from_secs(60),
            MAX_OUTPUT_BYTES,
            failing_exit_check,
        );
        match &r {
            Err(BoundedError::Wait(e)) => assert_eq!(e.to_string(), "injected wait failure"),
            other => panic!("expected BoundedError::Wait, got {other:?}"),
        }
        assert!(
            !matches!(&r, Err(BoundedError::Failed(BeadsError::Spawn { .. }))),
            "{r:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the child was waited for: {:?}",
            started.elapsed()
        );
    }
}
