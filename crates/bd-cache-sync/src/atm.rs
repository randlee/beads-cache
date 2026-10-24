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
//! A tick that sends several messages passes each one the time left before
//! its end `T` ([`tick`](crate::tick)).

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use bd_cache_model::{EscalationSender, SendError, SendErrorKind};

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
/// The least time given to the output readers after the child has exited,
/// even when the send's budget is spent: a reader that already has its data
/// needs a scheduler slice, not a second.
const READ_GRACE: Duration = Duration::from_millis(100);

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
                format!("{} was not found", self.binary.to_string_lossy()),
            )
        };
        let named = Path::new(&self.binary);
        if named.components().count() > 1 {
            return if named.is_file() {
                Ok(named.to_path_buf())
            } else {
                Err(not_found())
            };
        }
        let path = std::env::var_os("PATH").ok_or_else(not_found)?;
        let extensions: Vec<OsString> = if cfg!(windows) {
            ["", ".exe", ".cmd", ".bat", ".com"]
                .map(OsString::from)
                .to_vec()
        } else {
            vec![OsString::new()]
        };
        for dir in std::env::split_paths(&path) {
            for ext in &extensions {
                let mut file = self.binary.clone();
                file.push(ext);
                let candidate = dir.join(file);
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
        Err(not_found())
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
        while let Ok(n) = from.read(&mut buf) {
            if n == 0 {
                break;
            }
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
fn kill_tree(child: &mut Child) {
    let quiet = |mut c: Command| {
        let _ = c.stdout(Stdio::null()).stderr(Stdio::null()).status();
    };
    #[cfg(unix)]
    {
        // The child leads its own process group (`process_group(0)`), so the
        // negative pid names the group.
        let mut kill = Command::new("kill");
        kill.args(["-KILL", "--", &format!("-{}", child.id())]);
        quiet(kill);
    }
    #[cfg(windows)]
    {
        let mut kill = Command::new("taskkill");
        kill.args(["/T", "/F", "/PID", &child.id().to_string()]);
        quiet(kill);
    }
    // The group kill cannot reach a child that already left it, and is absent
    // on other platforms: kill the child itself too. Reaping stops a zombie.
    let _ = child.kill();
    let _ = child.wait();
}

/// The output of a stream, waited for no longer than `until`.
fn collect(rx: Option<mpsc::Receiver<Captured>>, until: Instant) -> String {
    let Some(rx) = rx else {
        return String::new();
    };
    let wait = until
        .saturating_duration_since(Instant::now())
        .max(READ_GRACE);
    match rx.recv_timeout(wait) {
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
        let (Ok(lead), Ok(team)) = (MemberName::new(lead), MemberName::new(team)) else {
            return Err(SendError::with_kind(
                SendErrorKind::InvalidAddress,
                format!("{lead:?}@{team:?} is not an address atm send can take"),
            ));
        };
        let timeout = timeout.min(self.timeout);
        if timeout.is_zero() {
            return Err(SendError::with_kind(
                SendErrorKind::TimedOut,
                "no time left to send",
            ));
        }
        let name = self.binary.to_string_lossy();
        let mut command = Command::new(&self.binary);
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
        let status = loop {
            if self.stop.is_stopped() {
                kill_tree(&mut child);
                return Err(SendError::with_kind(
                    SendErrorKind::Cancelled,
                    format!("{name} was stopped: the host is shutting down"),
                ));
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= until => {
                    kill_tree(&mut child);
                    return Err(SendError::with_kind(
                        SendErrorKind::TimedOut,
                        format!(
                            "{name} did not finish within {} ms and was killed",
                            timeout.as_millis()
                        ),
                    ));
                }
                Ok(None) => thread::sleep(POLL),
                Err(e) => {
                    kill_tree(&mut child);
                    return Err(SendError::with_kind(
                        SendErrorKind::Exited,
                        format!("waiting for {name}: {e}"),
                    ));
                }
            }
        };
        if status.success() {
            return Ok(());
        }
        let (out, err) = (collect(stdout, until), collect(stderr, until));
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
