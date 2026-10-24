//! The `atm send` sender (REQ-BDC-0704..0706; ADR-SYNC-0007).
//!
//! [`AtmSender`] delivers a message the way BD-CACHE's cron module does
//! (`bd_cache_cron.py` at atm-core `05831df`, `send`): it runs
//! `atm send <lead>@<team> --team tool --stdin --json` with
//! `ATM_IDENTITY=cron` and `ATM_TEAM=tool` in the environment and writes the
//! message to the child's standard input, so its length and content cannot be
//! read as options.
//!
//! # Bounds
//!
//! A send is bounded three ways. It never runs longer than the sender's
//! timeout ([`SEND_TIMEOUT`], 10 s as in the reference) or the time the tick
//! passes to [`EscalationSender::send_within`], whichever is less. When the
//! timeout runs out, or the [`StopFlag`] the sender was built with is set, the
//! whole process tree is killed (a process group on Unix, `taskkill /T` on
//! Windows) and reaped, so a launcher such as `uv run atm` cannot leave a
//! grandchild delivering a message the tick will send again. And the child's
//! output is read by threads that are waited for no longer than the time left
//! in the same budget; output beyond a limit is dropped and the error says so.
//! Each send starts at most three threads (the message writer and one reader per
//! output stream); they end when the pipes close, so a descendant that outlives
//! a failed send keeps its readers until it exits (ADR-SYNC-0007). A tick that
//! sends several messages passes each one the time left before
//! its end `T` ([`tick`](crate::tick)). After a successful send nothing is
//! killed or waited for: the child has been reaped, so its process-group id may
//! already belong to another process, and a descendant that outlives a delivered
//! message is the program's own business.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use bd_cache_model::{EscalationSender, MEMBER_NAME_RULE, SendError, SendErrorKind};

use crate::control::StopFlag;
use crate::phases::MemberName;
use crate::tick::TICK_BUDGET;

/// The team `atm send` runs under: the cron module's `--team tool`.
pub const SENDER_TEAM: &str = "tool";
/// The identity `atm send` runs as (`ATM_IDENTITY`).
pub const SENDER_IDENTITY: &str = "cron";
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
    identity: String,
    timeout: Duration,
    stop: StopFlag,
}

impl Default for AtmSender {
    fn default() -> Self {
        Self {
            binary: OsString::from("atm"),
            leading_args: Vec::new(),
            sender_team: SENDER_TEAM.to_owned(),
            identity: SENDER_IDENTITY.to_owned(),
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

    /// The program as a path, found the way the shell would find it.
    ///
    /// # Errors
    ///
    /// [`SendErrorKind::NotStartable`] when the program is not on the `PATH`
    /// (or, for a path, not a file).
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
        std::env::split_paths(&path)
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

/// Reads a stream to the end, keeping up to [`CAPTURE_LIMIT`] bytes, so a
/// chatty child never blocks on a full pipe.
fn capture(mut from: impl Read + Send + 'static) -> mpsc::Receiver<Captured> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut kept: Vec<u8> = Vec::new();
        let mut truncated = false;
        let mut buf = [0_u8; 1024];
        loop {
            let n = match from.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
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
    });
    rx
}

/// Kills the child and everything it started, then reaps it.
///
/// Only called while the child has not been reaped, so its process-group id
/// (its pid) cannot belong to anything else. The tree is killed with the
/// platform's own program (`kill` with a negative pid, `taskkill /T`): the
/// crate has no `unsafe` and no `libc` dependency (ADR-SYNC-0006). A failing
/// tree kill is logged and the child itself is killed regardless.
fn kill_tree(child: &mut Child) {
    let run = |mut c: Command| match c.stdout(Stdio::null()).stderr(Stdio::null()).status() {
        Ok(status) if status.success() => {}
        outcome => tracing::warn!(
            ?outcome,
            pid = child.id(),
            "killing the atm process tree failed; killing the child only"
        ),
    };
    #[cfg(unix)]
    {
        // The child leads its own process group (`process_group(0)`), so the
        // negative pid names the group.
        let program = if Path::new("/bin/kill").is_file() {
            "/bin/kill"
        } else {
            "kill"
        };
        let mut kill = Command::new(program);
        kill.args(["-KILL", "--", &format!("-{}", child.id())]);
        run(kill);
    }
    #[cfg(windows)]
    {
        let mut kill = Command::new("taskkill");
        kill.args(["/T", "/F", "/PID", &child.id().to_string()]);
        run(kill);
    }
    // The tree kill is absent on other platforms or may have failed: kill the
    // child itself too. Reaping stops a zombie.
    let _ = child.kill();
    let _ = child.wait();
}

/// The output of a stream, waited for until `until`: the child has exited, so
/// a reader that still has no end of stream is held open by a descendant, and
/// the send does not wait for it. One deadline serves both streams, so the wait
/// is at most [`READ_GRACE`] in all, inside the tick's send reserve.
fn collect(rx: Option<mpsc::Receiver<Captured>>, until: Instant) -> String {
    let Some(rx) = rx else {
        return String::new();
    };
    match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
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
            .env("ATM_IDENTITY", &self.identity)
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
        let stdout = child.stdout.take().map(capture);
        let stderr = child.stderr.take().map(capture);
        // Written from a thread: a child that never reads must not block us
        // past the timeout.
        let stdin = child.stdin.take();
        let message = body.to_owned();
        thread::spawn(move || {
            if let Some(mut stdin) = stdin {
                // A child that exits early closes the pipe; its status says why.
                let _ = stdin.write_all(message.as_bytes());
            }
        });
        let status = self.wait_for(&mut child, &name, until, timeout)?;
        if status.success() {
            return Ok(());
        }
        let read_until = Instant::now() + READ_GRACE;
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
}

impl AtmSender {
    /// Waits for the child until `until`, polling; kills its tree on a stop or a timeout.
    fn wait_for(
        &self,
        child: &mut Child,
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
}
