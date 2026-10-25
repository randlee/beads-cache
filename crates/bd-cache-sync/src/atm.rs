//! The `atm send` sender (REQ-BDC-0704..0706; ADR-SYNC-0007).
//!
//! [`AtmSender`] delivers a message the way BD-CACHE's cron module does
//! (`bd_cache_cron.py` at atm-core `05831df`, `send`): it runs
//! `atm send <lead>@<team> --team tool --stdin --json` with
//! `ATM_IDENTITY=<identity>` and `ATM_TEAM=tool` in the environment and writes
//! the message to the child's standard input, so its length and content cannot
//! be read as options.
//!
//! # Identity
//!
//! The identity is `atm-bd` unless configured ([`SenderIdentity`]). It is not
//! `cron`: `cron@tool` is atm's daemon-owned scheduler principal and refuses an
//! ordinary send, so a tick that sent as `cron` would have every message
//! refused (atm-bd 0.19.2 moved to `atm-bd@tool` for this reason).
//!
//! # Bounds
//!
//! A send is bounded three ways. It never runs longer than the sender's
//! timeout ([`SEND_TIMEOUT`], 10 s as in the reference) or the time the tick
//! passes to [`EscalationSender::send_within`], whichever is less. When the
//! timeout runs out, or the [`StopFlag`] the sender was built with is set, the
//! whole process tree is killed (a process group on Unix, with `kill(2)` itself
//! through [`sc_bd_cli::process_group::kill`], never a `kill` program, RSH-002;
//! `taskkill /T` on Windows) and reaped, so a launcher such as `uv run atm` cannot leave a
//! grandchild delivering a message the tick will send again. And the child's
//! output is read by threads that are waited for no longer than the time left
//! in the same budget (clamped to the send deadline, with a floor of
//! `READ_FLOOR`); output beyond a limit is dropped and the error says so.
//!
//! How far a send can run past its deadline (lead ruling S27, REQ-SYNC-0011):
//! the deadline is seen within one `POLL`; a child that exited with a failure
//! then has its output waited for at most `READ_FLOOR` past the deadline, and a
//! child that is killed is killed and reaped within one shared `KILL_BOUND`
//! (the tree kill and the reaping of the child share one deadline, and the
//! last look at the child is one try). Const assertions keep every such
//! overrun, and `READ_GRACE` plus `KILL_BOUND`, inside the tick's send
//! reserve (`SEND_RESERVE`, 1.5 s), so the send ends before the tick's end `T`.
//! On Windows the one step not bounded is starting the tree-kill program
//! itself, a local process start; on Unix the tree kill is one system call.
//!
//! # Finding `atm`
//!
//! A bare program name is looked up in the absolute directories of `PATH`
//! only ([`sc_bd_cli::path::absolute_path_entries`], lead rulings S76 and S77,
//! RSH-003): an empty or relative segment (`:/usr/bin`, `.:/usr/bin`) would
//! resolve against the working directory, the repository root, and run an
//! `atm` committed there.
//!
//! A child or tree-kill program that is still running at the kill deadline is
//! killed and handed to one detached reaper thread, which waits for it, so it
//! is not left a zombie for the life of the host (lead ruling S28,
//! ADR-SYNC-0007).
//!
//! Each send starts at most three threads (the message writer and one reader per
//! output stream), each with `thread::Builder`: one that cannot be started
//! fails the send and the child's tree is killed and reaped (PE-RSH-011). They
//! end when the pipes close, so a descendant that outlives
//! a failed send keeps its readers until it exits (ADR-SYNC-0007). A tick that
//! sends several messages passes each one the time left before
//! its end `T` ([`tick`](crate::tick)). After a successful send nothing is
//! killed or waited for: the child has been reaped, so its process-group id may
//! already belong to another process, and a descendant that outlives a delivered
//! message is the program's own business.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use bd_cache_model::{EscalationSender, MEMBER_NAME_RULE, SendError, SendErrorKind};

use crate::control::{SEND_RESERVE, StopFlag};
use crate::phases::MemberName;
use crate::tick::TICK_BUDGET;

/// The team `atm send` runs under: the cron module's `--team tool`.
pub const SENDER_TEAM: &str = "tool";
/// The identity `atm send` runs as (`ATM_IDENTITY`) unless one is configured.
///
/// `atm-bd@tool` is the installer's notification member; `cron@tool` is atm's
/// own scheduler principal and refuses ordinary sends.
pub const SENDER_IDENTITY: &str = "atm-bd";
/// The longest identity accepted: far above any member name, and a bound on
/// what ends up in a child's environment.
const MAX_IDENTITY_LEN: usize = 64;
/// The identity rule, in words.
pub const IDENTITY_RULE: &str =
    "1 to 64 characters from A-Z, a-z, 0-9, '.', '_' and '-', starting with a letter or digit";

/// An identity `atm send` can run as: the value of `ATM_IDENTITY`.
///
/// 1 to 64 characters from `A-Z a-z 0-9 . _ -`, starting with a letter or a
/// digit. That keeps it out of option position, free of whitespace and control
/// characters, and a plain member name.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::SenderIdentity;
/// assert_eq!(SenderIdentity::default().as_str(), "atm-bd");
/// assert_eq!(SenderIdentity::new("bd-cache.1").unwrap().as_str(), "bd-cache.1");
/// assert!(SenderIdentity::new("").is_err());
/// assert!(SenderIdentity::new("-x").is_err());
/// assert!(SenderIdentity::new("a b").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SenderIdentity(String);

/// An identity [`SenderIdentity`] refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidIdentity {
    /// The value as written.
    pub value: String,
}

impl fmt::Display for InvalidIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the sender identity {:?} is not usable; it must be {IDENTITY_RULE}",
            self.value
        )
    }
}

impl std::error::Error for InvalidIdentity {}

impl SenderIdentity {
    /// Validates `identity` as given (callers trim).
    ///
    /// # Errors
    ///
    /// [`InvalidIdentity`] for a value that breaks [`IDENTITY_RULE`].
    pub fn new(identity: impl Into<String>) -> Result<Self, InvalidIdentity> {
        let identity = identity.into();
        let ok = !identity.is_empty()
            && identity.len() <= MAX_IDENTITY_LEN
            && identity.starts_with(|c: char| c.is_ascii_alphanumeric())
            && identity
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if ok {
            Ok(Self(identity))
        } else {
            Err(InvalidIdentity { value: identity })
        }
    }

    /// The identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SenderIdentity {
    fn default() -> Self {
        Self(SENDER_IDENTITY.to_owned())
    }
}

impl fmt::Display for SenderIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
/// How long one `atm send` may take (the reference's `timeout=10`).
///
/// A tick has 25 s and may send several messages, each of which a lead is
/// waiting for; 10 s is far above a local mailbox write and keeps one stuck
/// `atm` from using more than a third of the budget.
pub const SEND_TIMEOUT: Duration = Duration::from_secs(10);
/// The most output of the child kept for an error message.
const CAPTURE_LIMIT: usize = 4096;
/// How often a running child is polled.
const POLL: Duration = Duration::from_millis(20);
/// The longest the output readers are waited for after the child has exited
/// with a failure: a reader that has its data needs a scheduler slice; one
/// that does not is held open by a descendant.
const READ_GRACE: Duration = Duration::from_millis(250);
/// The least the output readers are waited for even when the send deadline has
/// all but passed: enough for a scheduler slice, and (with [`READ_GRACE`]) below
/// the tick's send reserve, so the overrun of a send deadline stays inside it.
const READ_FLOOR: Duration = Duration::from_millis(50);

// The reserve the tick keeps after the last message (REQ-SYNC-0011) must cover
// the longest wait after a send's deadline (lead ruling S27).
const _: () = assert!(READ_FLOOR.as_millis() <= READ_GRACE.as_millis());
const _: () = assert!(
    READ_GRACE.as_millis() + KILL_BOUND.as_millis() < SEND_RESERVE.as_millis(),
    "SEND_RESERVE must cover READ_GRACE plus the shared kill bound"
);
const _: () = assert!(
    POLL.as_millis() + KILL_BOUND.as_millis() + KILL_POLL.as_millis() < SEND_RESERVE.as_millis(),
    "a killed send must end inside SEND_RESERVE"
);
const _: () = assert!(
    POLL.as_millis() + READ_FLOOR.as_millis() < SEND_RESERVE.as_millis(),
    "a failed send must end inside SEND_RESERVE"
);

/// When to stop reading a failed child's output: [`READ_GRACE`] from `now`, but
/// no later than the send deadline `until`, and never less than [`READ_FLOOR`].
fn read_deadline(now: Instant, until: Instant) -> Instant {
    let left = until.saturating_duration_since(now);
    now + READ_GRACE.min(left).max(READ_FLOOR)
}

/// A timeout the sender refuses to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTimeout(pub Duration);

impl fmt::Display for InvalidTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a send timeout of {:?} is not usable; use more than zero and less than the tick budget ({:?})",
            self.0, TICK_BUDGET
        )
    }
}

impl InvalidTimeout {
    /// A stable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        "invalid_timeout"
    }
}

impl std::error::Error for InvalidTimeout {}

/// Sends messages by running `atm send`.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::AtmSender;
/// let sender = AtmSender::default();
/// assert_eq!(sender.binary().to_string_lossy(), "atm");
/// // A lead that could be read as an option is refused before any process starts.
/// use bd_cache_model::{EscalationSender, SendErrorKind};
/// let err = sender.send("--help", "atm-dev", "hi").unwrap_err();
/// assert_eq!(err.kind, SendErrorKind::InvalidAddress);
/// ```
#[derive(Debug, Clone)]
pub struct AtmSender {
    binary: OsString,
    leading_args: Vec<OsString>,
    sender_team: String,
    identity: SenderIdentity,
    timeout: Duration,
    stop: StopFlag,
}

impl Default for AtmSender {
    fn default() -> Self {
        Self {
            binary: OsString::from("atm"),
            leading_args: Vec::new(),
            sender_team: SENDER_TEAM.to_owned(),
            identity: SenderIdentity::default(),
            timeout: SEND_TIMEOUT,
            stop: StopFlag::new(),
        }
    }
}

impl AtmSender {
    /// A sender that runs `binary` instead of `atm` on the `PATH`.
    #[must_use]
    pub fn with_binary(binary: impl Into<OsString>) -> Self {
        Self {
            binary: binary.into(),
            ..Self::default()
        }
    }

    /// This sender with `args` before `send` on the command line, for a
    /// launcher such as `uv` (`uv run atm send ...`).
    #[must_use]
    pub fn with_leading_args(
        mut self,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Self {
        self.leading_args = args.into_iter().map(Into::into).collect();
        self
    }

    /// This sender with another per-message timeout.
    ///
    /// # Errors
    ///
    /// [`InvalidTimeout`] for zero, or a timeout that is not shorter than the
    /// tick budget ([`TICK_BUDGET`]), which no message could use.
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, InvalidTimeout> {
        if timeout.is_zero() || timeout >= TICK_BUDGET {
            return Err(InvalidTimeout(timeout));
        }
        self.timeout = timeout;
        Ok(self)
    }

    /// This sender running `atm send` as `identity` (`ATM_IDENTITY`).
    #[must_use]
    pub fn with_identity(mut self, identity: SenderIdentity) -> Self {
        self.identity = identity;
        self
    }

    /// The identity `atm send` runs as.
    #[must_use]
    pub fn identity(&self) -> &SenderIdentity {
        &self.identity
    }

    /// This sender stopping its child when `stop` is set (the host's shutdown).
    #[must_use]
    pub fn with_stop(mut self, stop: StopFlag) -> Self {
        self.stop = stop;
        self
    }

    /// The program that is run.
    #[must_use]
    pub fn binary(&self) -> &OsStr {
        &self.binary
    }

    /// The program as a path, found the way the shell would find it, but in the
    /// absolute directories of `PATH` only (RSH-003): an empty or relative
    /// segment is skipped, so an `atm` in the working directory never runs.
    ///
    /// # Errors
    ///
    /// [`SendErrorKind::NotStartable`] when the program is not in an absolute
    /// directory of the `PATH` (or, for a path, not a file).
    pub fn resolve(&self) -> Result<PathBuf, SendError> {
        let not_found = || {
            SendError::with_kind(
                SendErrorKind::NotStartable,
                format!(
                    "{} was not found or is not executable",
                    self.binary.to_string_lossy()
                ),
            )
        };
        let named = Path::new(&self.binary);
        if named.components().count() > 1 {
            return if is_executable(named) {
                Ok(named.to_path_buf())
            } else {
                Err(not_found())
            };
        }
        let path = std::env::var_os("PATH").ok_or_else(not_found)?;
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_PATHEXT.to_owned());
        let file_names = candidate_names(&self.binary, &pathext, cfg!(windows));
        sc_bd_cli::path::absolute_path_entries(&path)
            .into_iter()
            .flat_map(|dir| file_names.iter().map(move |n| dir.join(n)))
            .find(|candidate| is_executable(candidate))
            .ok_or_else(not_found)
    }

    /// The arguments of one send, after the leading ones.
    fn args(&self, lead: &MemberName, team: &MemberName) -> Vec<OsString> {
        [
            "send",
            &format!("{lead}@{team}"),
            "--team",
            &self.sender_team,
            "--stdin",
            "--json",
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }
}

/// The Windows default of `PATHEXT`.
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// The file names a program name can stand for in one `PATH` directory, in the
/// order they are tried. On Windows that is the shell's order: the name with
/// each `PATHEXT` extension in turn, and the bare name only when it already
/// ends in one (a bare `atm` next to `atm.cmd` is an npm shim script that
/// `CreateProcess` cannot run). Elsewhere it is the name.
fn candidate_names(binary: &OsStr, pathext: &str, windows: bool) -> Vec<OsString> {
    if !windows {
        return vec![binary.to_owned()];
    }
    let extensions: Vec<&str> = pathext.split(';').filter(|e| e.starts_with('.')).collect();
    let lowered = binary.to_string_lossy().to_ascii_lowercase();
    if extensions
        .iter()
        .any(|e| lowered.ends_with(&e.to_ascii_lowercase()))
    {
        return vec![binary.to_owned()];
    }
    extensions
        .into_iter()
        .map(|ext| {
            let mut name = binary.to_owned();
            name.push(ext);
            name
        })
        .collect()
}

/// A regular file the user can run: on Unix it has an execute bit.
fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

/// What a reader thread kept of one output stream.
struct Captured {
    text: String,
    /// More was written than [`CAPTURE_LIMIT`].
    truncated: bool,
}

/// Starts one named helper thread of a send (the message writer or an output
/// reader). A parameter of the send, so a test can make it fail (PE-RSH-011).
type StartThread = fn(&'static str, Box<dyn FnOnce() + Send + 'static>) -> io::Result<()>;

/// The real [`StartThread`]: a named thread, whose start failure is an error
/// instead of the panic of `thread::spawn`.
fn start_thread(name: &'static str, body: Box<dyn FnOnce() + Send + 'static>) -> io::Result<()> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(body)
        .map(drop)
}

/// Reads a stream to the end, keeping up to [`CAPTURE_LIMIT`] bytes, so a
/// chatty child never blocks on a full pipe.
///
/// # Errors
///
/// The error of `start` when the reader thread cannot be started.
fn capture(
    mut from: impl Read + Send + 'static,
    name: &'static str,
    start: StartThread,
) -> io::Result<mpsc::Receiver<Captured>> {
    let (tx, rx) = mpsc::channel();
    start(
        name,
        Box::new(move || {
            let mut kept: Vec<u8> = Vec::new();
            let mut truncated = false;
            let mut buf = [0_u8; 1024];
            loop {
                let n = match from.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                let room = CAPTURE_LIMIT - kept.len();
                truncated |= n > room;
                kept.extend(buf.iter().copied().take(n.min(room)));
            }
            // The receiver may be gone after a timeout; nobody is waiting.
            let _ = tx.send(Captured {
                text: String::from_utf8_lossy(&kept).trim().to_owned(),
                truncated,
            });
        }),
    )?;
    Ok(rx)
}

/// The output readers of a running child.
type Readers = (
    Option<mpsc::Receiver<Captured>>,
    Option<mpsc::Receiver<Captured>>,
);

/// Starts the two output readers and the writer of `message` to the child's
/// standard input.
///
/// The writer runs in its own thread: a child that never reads must not
/// block the send past its timeout.
///
/// # Errors
///
/// The error of the first thread that cannot be started; the threads already
/// started end when the child's pipes close.
fn start_helpers(child: &mut Child, message: &str, start: StartThread) -> io::Result<Readers> {
    let stdout = child
        .stdout
        .take()
        .map(|s| capture(s, "atm-send-stdout", start))
        .transpose()?;
    let stderr = child
        .stderr
        .take()
        .map(|s| capture(s, "atm-send-stderr", start))
        .transpose()?;
    let stdin = child.stdin.take();
    let message = message.to_owned();
    start(
        "atm-send-stdin",
        Box::new(move || {
            if let Some(mut stdin) = stdin {
                // A child that exits early closes the pipe; its status says why.
                let _ = stdin.write_all(message.as_bytes());
            }
        }),
    )?;
    Ok((stdout, stderr))
}

/// How long the tree kill and the reaping of the child may take together
/// before they are given up on: one shared kill deadline (lead ruling S27). A
/// stuck `kill` or `taskkill`, or a child in an uninterruptible state, must not
/// hold the tick past its end `T`.
const KILL_BOUND: Duration = Duration::from_secs(1);
/// How often a process being killed is polled.
const KILL_POLL: Duration = Duration::from_millis(5);
/// How often the reaper looks at the processes it holds.
const REAP_POLL: Duration = Duration::from_millis(200);

/// Polls `child` until it has exited or `until` has passed; whether it exited.
/// It is always looked at once, even when `until` has already passed.
fn exited_by(child: &mut Child, until: Instant) -> bool {
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(_) => return false,
        }
        if Instant::now() >= until {
            return false;
        }
        thread::sleep(KILL_POLL);
    }
}

/// Hands a killed process that has not been reaped to the one reaper thread,
/// which waits for it so it is not left a zombie (lead ruling S28).
///
/// The thread is started on first use and lives as long as the host; it polls
/// what it holds every [`REAP_POLL`] and drops each process once it has exited.
/// A process that never exits (an uninterruptible state) is held, not waited
/// out, so it cannot keep the others from being reaped.
fn adopt(child: Child) {
    static REAPER: OnceLock<Option<mpsc::Sender<Child>>> = OnceLock::new();
    let reaper = REAPER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Child>();
        match thread::Builder::new()
            .name("bd-cache-atm-reaper".to_owned())
            .spawn(move || reap(&rx))
        {
            Ok(_) => Some(tx),
            Err(e) => {
                tracing::warn!(error = %e, "cannot start the atm reaper; a killed process may stay a zombie");
                None
            }
        }
    });
    let pid = child.id();
    if reaper.as_ref().is_none_or(|tx| tx.send(child).is_err()) {
        tracing::warn!(pid, "no atm reaper; the killed process may stay a zombie");
    }
}

/// The reaper thread: holds the processes it is given until each has exited.
fn reap(rx: &mpsc::Receiver<Child>) {
    let mut held: Vec<Child> = Vec::new();
    loop {
        let next = if held.is_empty() {
            match rx.recv() {
                Ok(child) => Ok(child),
                Err(mpsc::RecvError) => Err(mpsc::RecvTimeoutError::Disconnected),
            }
        } else {
            rx.recv_timeout(REAP_POLL)
        };
        match next {
            Ok(child) => held.push(child),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) if held.is_empty() => return,
            Err(mpsc::RecvTimeoutError::Disconnected) => thread::sleep(REAP_POLL),
        }
        // Keep only what is still running; an error means it cannot be waited for.
        held.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
    }
}

/// How a tree kill ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreeKill {
    /// The program ran and succeeded.
    Done,
    /// It could not be started, failed or could not be waited for.
    Failed,
    /// It was still running at the kill deadline: it was killed and handed to
    /// the reaper (a tree-kill program: Windows `taskkill`).
    #[cfg(any(windows, test))]
    GaveUp {
        /// The program's pid.
        pid: u32,
    },
}

/// Runs a tree-kill program (Windows `taskkill`) until `until` at most; a
/// failure or an overrun is logged, never waited out.
#[cfg(any(windows, test))]
fn run_tree_kill(mut command: Command, pid: u32, until: Instant) -> TreeKill {
    let mut helper = match command.stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(helper) => helper,
        Err(e) => {
            tracing::warn!(error = %e, pid, "cannot start the atm tree kill; killing the child only");
            return TreeKill::Failed;
        }
    };
    loop {
        match helper.try_wait() {
            Ok(Some(status)) if status.success() => return TreeKill::Done,
            Ok(Some(status)) => {
                tracing::warn!(%status, pid, "killing the atm process tree failed; killing the child only");
                return TreeKill::Failed;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(error = %e, pid, "waiting for the atm tree kill failed; killing the child only");
                return TreeKill::Failed;
            }
        }
        if Instant::now() >= until {
            tracing::warn!(
                pid,
                "the atm tree kill did not finish in time; giving up on it"
            );
            let _ = helper.kill();
            let helper_pid = helper.id();
            adopt(helper);
            return TreeKill::GaveUp { pid: helper_pid };
        }
        thread::sleep(KILL_POLL);
    }
}

/// How the process tree led by a child is killed.
#[derive(Debug)]
enum TreeKiller {
    /// `kill(-pgid, SIGKILL)`: the child leads its own process group
    /// (`process_group(0)`), so its pid names the group. One system call, made
    /// by sc-bd-cli's safe wrapper (RSH-002): no `kill` program, no `PATH`, and
    /// no `unsafe` in this crate (ADR-SYNC-0006).
    #[cfg(unix)]
    Group,
    /// A program that kills the tree: `taskkill /T /F`.
    #[cfg(windows)]
    Program(Command),
}

/// The platform's way to kill the process tree led by `pid`; `None` on a
/// platform with neither.
#[cfg_attr(
    any(unix, windows),
    expect(clippy::unnecessary_wraps, reason = "None on a platform with neither")
)]
fn tree_killer(pid: u32) -> Option<TreeKiller> {
    #[cfg(unix)]
    {
        let _ = pid;
        Some(TreeKiller::Group)
    }
    #[cfg(windows)]
    {
        let mut kill = Command::new("taskkill");
        kill.args(["/T", "/F", "/PID", &pid.to_string()]);
        Some(TreeKiller::Program(kill))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        None
    }
}

/// Kills the child and everything it started, then reaps it, in bounded time.
///
/// Only called while the child has not been reaped, so its process-group id
/// (its pid) cannot belong to anything else.
fn kill_tree(child: Child) {
    let killer = tree_killer(child.id());
    kill_tree_with(child, killer, Instant::now() + KILL_BOUND);
}

/// [`kill_tree`] with the tree killer and the kill deadline given.
///
/// The tree kill and the reaping share one deadline, `until` ([`KILL_BOUND`]
/// from the start, lead ruling S27). A failing or overrunning tree kill is
/// logged and the child itself is killed regardless; a child or tree kill not
/// done by the deadline is handed to the reaper (lead ruling S28).
#[cfg_attr(
    not(windows),
    expect(
        clippy::needless_pass_by_value,
        reason = "only the Windows killer owns a program to run"
    )
)]
fn kill_tree_with(mut child: Child, killer: Option<TreeKiller>, until: Instant) -> TreeKill {
    let pid = child.id();
    let outcome = match killer {
        #[cfg(unix)]
        Some(TreeKiller::Group) => match sc_bd_cli::process_group::kill(pid) {
            Ok(()) => TreeKill::Done,
            Err(e) => {
                tracing::warn!(error = %e, pid, "killing the atm process group failed; killing the child only");
                TreeKill::Failed
            }
        },
        #[cfg(windows)]
        Some(TreeKiller::Program(kill)) => run_tree_kill(kill, pid, until),
        None => TreeKill::Failed,
    };
    // The tree kill is absent on other platforms or may have failed: kill the
    // child itself too, and reap it so it is no zombie.
    let _ = child.kill();
    if !exited_by(&mut child, until) {
        tracing::warn!(
            pid,
            "the atm child was not reaped in time; handing it to the reaper"
        );
        adopt(child);
    }
    outcome
}

/// How long a reader may still be waited for at `now`: nothing once `until`
/// has passed, so the second stream of a failed send waits only for what is
/// left of the one deadline both share.
fn time_left(until: Instant, now: Instant) -> Duration {
    until.saturating_duration_since(now)
}

/// The output of a stream, waited for until `until`: the child has exited, so
/// a reader that still has no end of stream is held open by a descendant, and
/// the send does not wait for it. One deadline serves both streams, so the wait
/// is at most [`READ_GRACE`] in all (see [`read_deadline`]), inside the tick's send reserve.
fn collect(rx: Option<mpsc::Receiver<Captured>>, until: Instant) -> String {
    let Some(rx) = rx else {
        return String::new();
    };
    match rx.recv_timeout(time_left(until, Instant::now())) {
        Ok(c) if c.truncated => format!("{} ... (output truncated)", c.text),
        Ok(c) => c.text,
        Err(_) => "(output not fully read)".to_owned(),
    }
}

impl EscalationSender for AtmSender {
    fn send(&self, lead: &str, team: &str, body: &str) -> Result<(), SendError> {
        self.send_within(lead, team, body, self.timeout)
    }

    fn send_within(
        &self,
        lead: &str,
        team: &str,
        body: &str,
        timeout: Duration,
    ) -> Result<(), SendError> {
        self.send_using(lead, team, body, timeout, start_thread)
    }
}

impl AtmSender {
    /// [`EscalationSender::send_within`], with the function that starts the
    /// send's helper threads.
    fn send_using(
        &self,
        lead: &str,
        team: &str,
        body: &str,
        timeout: Duration,
        start: StartThread,
    ) -> Result<(), SendError> {
        // Defensive: the tick passes names it validated (`MemberName`), but the
        // port takes text, and this text becomes a command-line argument.
        let lead = MemberName::new(lead).map_err(|e| {
            SendError::with_kind(
                SendErrorKind::InvalidAddress,
                format!("the lead {:?} is not an address atm send can take: a name must be {MEMBER_NAME_RULE}", e.value),
            )
        })?;
        let team = MemberName::new(team).map_err(|e| {
            SendError::with_kind(
                SendErrorKind::InvalidAddress,
                format!("the team {:?} is not an address atm send can take: a name must be {MEMBER_NAME_RULE}", e.value),
            )
        })?;
        let timeout = timeout.min(self.timeout);
        if timeout.is_zero() {
            return Err(SendError::with_kind(
                SendErrorKind::TimedOut,
                "no time left to send",
            ));
        }
        let name = self.binary.to_string_lossy();
        // Spawn the program found, not the bare name: on Windows `resolve`
        // accepts `.cmd` and `.bat`, which a bare name would not find.
        let program = self.resolve().map_err(|e| {
            SendError::with_kind(
                SendErrorKind::NotStartable,
                format!("cannot start {name}: {}", e.detail),
            )
        })?;
        let mut command = Command::new(program);
        command
            .args(&self.leading_args)
            .args(self.args(&lead, &team))
            .env("ATM_IDENTITY", self.identity.as_str())
            .env("ATM_TEAM", &self.sender_team)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }
        let mut child = command.spawn().map_err(|e| {
            SendError::with_kind(
                SendErrorKind::NotStartable,
                format!("cannot start {name}: {e}"),
            )
        })?;
        let started = Instant::now();
        let until = started + timeout;
        let (stdout, stderr) = match start_helpers(&mut child, body, start) {
            Ok(readers) => readers,
            Err(e) => {
                // Without its readers or its writer the child could block on a
                // pipe; it is not left running (PE-RSH-011).
                kill_tree(child);
                return Err(SendError::with_kind(
                    SendErrorKind::Other,
                    format!("cannot start a thread for {name}: {e}; it was killed"),
                ));
            }
        };
        let status = self.wait_for(child, &name, until, timeout)?;
        if status.success() {
            return Ok(());
        }
        let read_until = read_deadline(Instant::now(), until);
        let (out, err) = (collect(stdout, read_until), collect(stderr, read_until));
        let said = if err.is_empty() { out } else { err };
        Err(SendError::with_kind(
            SendErrorKind::Exited,
            if said.is_empty() {
                format!("{name} exited with {status}")
            } else {
                format!("{name} exited with {status}: {said}")
            },
        ))
    }

    /// Waits for the child until `until`, polling; kills its tree on a stop or a timeout.
    fn wait_for(
        &self,
        mut child: Child,
        name: &str,
        until: Instant,
        timeout: Duration,
    ) -> Result<std::process::ExitStatus, SendError> {
        loop {
            // A send that finished is a delivered message, even if the host
            // asked to stop in the same instant: look at the child first.
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {}
                Err(e) => {
                    kill_tree(child);
                    return Err(SendError::with_kind(
                        SendErrorKind::Other,
                        format!("waiting for {name}: {e}"),
                    ));
                }
            }
            if self.stop.is_stopped() {
                kill_tree(child);
                return Err(SendError::with_kind(
                    SendErrorKind::Cancelled,
                    format!("{name} was stopped: the host is shutting down"),
                ));
            }
            if Instant::now() >= until {
                kill_tree(child);
                return Err(SendError::with_kind(
                    SendErrorKind::TimedOut,
                    format!(
                        "{name} did not finish within {} ms and was killed",
                        timeout.as_millis()
                    ),
                ));
            }
            thread::sleep(POLL);
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::process::CommandExt as _;

    use super::*;

    fn names(binary: &str, pathext: &str, windows: bool) -> Vec<String> {
        candidate_names(OsStr::new(binary), pathext, windows)
            .into_iter()
            .map(|n| n.to_string_lossy().into_owned())
            .collect()
    }

    /// Windows tries `PATHEXT` extensions in order and never the bare name of a
    /// script shim; a name with an extension is taken as it is (QA3-004).
    #[test]
    fn windows_names_follow_pathext_and_unix_takes_the_name() {
        assert_eq!(
            names("atm", ".COM;.EXE;.BAT;.CMD", true),
            ["atm.COM", "atm.EXE", "atm.BAT", "atm.CMD"]
        );
        assert_eq!(names("atm", ".EXE;.CMD", true), ["atm.EXE", "atm.CMD"]);
        assert_eq!(names("atm.cmd", ".COM;.EXE;.BAT;.CMD", true), ["atm.cmd"]);
        assert_eq!(names("atm.EXE", ".exe;.cmd", true), ["atm.EXE"]);
        assert_eq!(names("atm", ".EXE;junk;;", true), ["atm.EXE"]);
        assert_eq!(names("atm", ".COM;.EXE", false), ["atm"]);
    }

    /// The read deadline is the grace, cut to the send deadline, never below the floor.
    #[test]
    fn the_read_deadline_is_clamped_to_the_send_deadline_with_a_floor() {
        let now = Instant::now();
        assert_eq!(
            read_deadline(now, now + Duration::from_secs(5)),
            now + READ_GRACE
        );
        let near = Duration::from_millis(100);
        assert_eq!(read_deadline(now, now + near), now + near);
        assert_eq!(read_deadline(now, now), now + READ_FLOOR);
    }

    /// Two silent readers share one deadline (RSH-026): the first waits for
    /// it; the second is collected once it has passed and gets no wait of its
    /// own (FTQ-019: shown by the deadline, not by an upper bound on the clock).
    #[test]
    fn collect_waits_one_shared_grace_for_both_streams() {
        let (_keep_out, out) = mpsc::channel::<Captured>();
        let (_keep_err, err) = mpsc::channel::<Captured>();
        let until = Instant::now() + READ_FLOOR;
        assert_eq!(collect(Some(out), until), "(output not fully read)");
        let now = Instant::now();
        assert!(now >= until, "the first stream waited for the deadline");
        assert_eq!(time_left(until, now), Duration::ZERO);
        assert_eq!(collect(Some(err), until), "(output not fully read)");
        // A stream that has its data is still read after the deadline.
        let (tx, ready) = mpsc::channel();
        tx.send(Captured {
            text: "said".to_owned(),
            truncated: false,
        })
        .unwrap();
        assert_eq!(collect(Some(ready), until), "said");
    }

    /// Whether a process with this pid exists (a zombie still does).
    #[cfg(unix)]
    fn exists(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    /// Whether the process is a zombie: dead, not yet reaped by its parent.
    /// On Linux the state field of `/proc/<pid>/stat` (after the `)` that ends
    /// the command name), elsewhere `ps -o stat=`; both say `Z`.
    #[cfg(target_os = "linux")]
    fn zombie(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| {
                let (_, rest) = stat.rsplit_once(')')?;
                Some(rest.trim_start().starts_with('Z'))
            })
            .unwrap_or(false)
    }

    /// See the Linux form.
    #[cfg(all(unix, not(target_os = "linux")))]
    fn zombie(pid: u32) -> bool {
        Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .stderr(Stdio::null())
            .output()
            .is_ok_and(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim_start()
                    .starts_with('Z')
            })
    }

    /// Waits until `alive(pid)` is false, up to 10 s.
    #[cfg(unix)]
    fn wait_until_not(alive: impl Fn(u32) -> bool, pid: u32, what: &str, state: &str) {
        let until = Instant::now() + Duration::from_secs(10);
        while alive(pid) {
            assert!(Instant::now() < until, "{what} (pid {pid}) {state}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// Waits until a process this code owns (a child it spawned, or one it
    /// handed to the reaper) no longer exists: killed and reaped. A zombie
    /// fails it: reaping it is this code's job (S28).
    #[cfg(unix)]
    fn assert_reaped(pid: u32, what: &str) {
        wait_until_not(
            exists,
            pid,
            what,
            "still exists: not killed or left a zombie",
        );
    }

    /// Waits until a process this code does not own (a grandchild, reparented
    /// to PID 1 when its parent died) is dead. A zombie counts as gone: its
    /// reaping is PID 1's, and a container's PID 1 may never do it (FTQ-024).
    #[cfg(unix)]
    fn assert_gone(pid: u32, what: &str) {
        wait_until_not(|p| exists(p) && !zombie(p), pid, what, "is still running");
    }

    /// Waits for the pid a script wrote to `path`.
    #[cfg(unix)]
    fn pid_in(path: &Path) -> u32 {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(pid) = std::fs::read_to_string(path)
                .ok()
                .and_then(|t| t.trim().parse().ok())
            {
                return pid;
            }
            assert!(Instant::now() < until, "{} never appeared", path.display());
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// A stuck tree kill is given up on at the shared deadline, and the reaper
    /// waits for it so it is no zombie (S27, S28).
    #[cfg(unix)]
    #[test]
    fn a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped() {
        let mut stuck = Command::new("sleep");
        stuck.arg("30");
        // The deadline is the test's; that KILL_BOUND fits the send reserve is
        // the const assertion's (FTQ-021): no clock bound here.
        let outcome = run_tree_kill(stuck, 0, Instant::now() + READ_FLOOR);
        let TreeKill::GaveUp { pid } = outcome else {
            panic!("given up on at the deadline: {outcome:?}");
        };
        assert_reaped(pid, "the stuck tree kill");
    }

    /// The real tree kill (`tree_killer`) of a child that ignores TERM,
    /// with a descendant that ignores it too: the group kill finishes, the
    /// child is reaped and the descendant is dead (S27, S28).
    ///
    /// The kill deadline is generous (FTQ-025): this test covers the group
    /// kill, not the deadline, so no host is slow enough to cut it short. That
    /// a tree kill still running at the deadline is given up on is
    /// `a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped`'s; that
    /// `KILL_BOUND` fits the send reserve is the const assertions'.
    #[cfg(unix)]
    #[test]
    fn a_child_that_ignores_term_and_its_descendant_are_killed_by_the_real_tree_kill() {
        /// Far above any host's group kill; never reached by a passing run.
        const GENEROUS: Duration = Duration::from_secs(30);
        let dir = tempfile::tempdir().unwrap();
        let (child_pid, grandchild_pid) = (dir.path().join("pid"), dir.path().join("grandchild"));
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(r#"trap '' TERM; sleep 30 & echo $! > "$2"; echo $$ > "$1"; wait"#)
            .arg("sh")
            .arg(&child_pid)
            .arg(&grandchild_pid)
            .process_group(0);
        let child = command.spawn().unwrap();
        // Ready: both processes have started and ignore TERM.
        let (pid, grandchild) = (pid_in(&child_pid), pid_in(&grandchild_pid));
        assert_eq!(pid, child.id());
        let killer = tree_killer(pid);
        assert!(killer.is_some(), "unix has a tree kill");
        let outcome = kill_tree_with(child, killer, Instant::now() + GENEROUS);
        assert_eq!(outcome, TreeKill::Done, "the group kill finished");
        assert_reaped(pid, "the child");
        assert_gone(grandchild, "the grandchild");
    }

    /// A child given up on at the kill deadline is reaped by the reaper (S28).
    #[cfg(unix)]
    #[test]
    fn a_child_that_is_not_reaped_in_time_is_reaped_by_the_reaper() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        child.kill().unwrap();
        adopt(child);
        assert_reaped(pid, "the adopted child");
    }

    #[cfg(unix)]
    thread_local! {
        /// The helper thread [`failing_start`] refuses to start on this thread.
        static REFUSED: std::cell::Cell<&'static str> = const { std::cell::Cell::new("") };
        /// The file the scripted child writes its pid to.
        static PID_FILE: std::cell::RefCell<PathBuf> = const { std::cell::RefCell::new(PathBuf::new()) };
        /// The pid it wrote, seen by [`failing_start`] before it refused.
        static CHILD_PID: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }

    /// A [`StartThread`] that refuses the thread named in [`REFUSED`] (once
    /// the child has written its pid, so the test knows whom to look for) and
    /// starts the others.
    #[cfg(unix)]
    fn failing_start(
        name: &'static str,
        body: Box<dyn FnOnce() + Send + 'static>,
    ) -> io::Result<()> {
        if name != REFUSED.get() {
            return start_thread(name, body);
        }
        CHILD_PID.set(PID_FILE.with_borrow(|p| pid_in(p)));
        Err(io::Error::new(
            io::ErrorKind::OutOfMemory,
            "no thread can be started",
        ))
    }

    /// A send whose stdout reader, stderr reader or stdin writer cannot be
    /// started is an error, not a panic, and its child is killed and reaped
    /// rather than left running (PE-RSH-011).
    #[cfg(unix)]
    #[test]
    fn a_helper_thread_that_cannot_start_fails_the_send_and_kills_the_child() {
        for refused in ["atm-send-stdout", "atm-send-stderr", "atm-send-stdin"] {
            let dir = tempfile::tempdir().unwrap();
            let pid_file = dir.path().join("pid");
            let atm = dir.path().join("atm");
            // Written by a child so no inherited write fd can make the exec ETXTBSY.
            crate::test_script::write(
                &atm,
                &format!(
                    "#!/bin/sh\necho $$ > '{}'\nexec sleep 30\n",
                    pid_file.display()
                ),
            );
            REFUSED.set(refused);
            PID_FILE.set(pid_file);
            CHILD_PID.set(0);

            let sender = AtmSender::with_binary(&atm);
            let err = sender
                .send_using("lead-x", "atm-dev", "hello", SEND_TIMEOUT, failing_start)
                .unwrap_err();

            assert_eq!(err.kind, SendErrorKind::Other, "{refused}: {err:?}");
            assert!(
                err.detail.contains("cannot start a thread") && err.detail.contains("killed"),
                "{refused}: {err:?}"
            );
            let pid = CHILD_PID.get();
            assert_ne!(pid, 0, "{refused}: the refusal saw the child");
            assert_reaped(pid, refused);
        }
    }
}
