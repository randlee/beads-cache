//! The `atm send` sender (REQ-BDC-0704..0706).
//!
//! [`AtmSender`] delivers a message by running
//! `atm send <lead>@<team> --team tool --as cron --stdin` and writing the
//! message to its standard input, as BD-CACHE's cron module does. The body
//! travels on stdin, never in the argument list, so its length and content
//! cannot be read as options. The call is bounded: a child still running
//! after the timeout is killed and the send fails, so a stuck `atm` cannot
//! hold the tick past its budget.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use bd_cache_model::{EscalationSender, SendError};

/// The team `atm send` runs under: the cron module's `--team tool`.
pub const SENDER_TEAM: &str = "tool";
/// The identity `atm send` runs as.
pub const SENDER_IDENTITY: &str = "cron";
/// How long one `atm send` may take.
///
/// A tick has 25 s and may send several messages, each of which a lead is
/// waiting for; 10 s is far above a local mailbox write and keeps one stuck
/// `atm` from using more than a third of the budget.
pub const SEND_TIMEOUT: Duration = Duration::from_secs(10);
/// The most output of the child kept for an error message.
const CAPTURE_LIMIT: usize = 4096;
/// How often a running child is polled.
const POLL: Duration = Duration::from_millis(20);

/// Sends messages by running `atm send`.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::AtmSender;
/// let sender = AtmSender::default();
/// assert_eq!(sender.binary(), "atm");
/// // A lead that could be read as an option is refused before any process starts.
/// use bd_cache_model::EscalationSender;
/// assert!(sender.send("--help", "atm-dev", "hi").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtmSender {
    binary: OsString,
    leading_args: Vec<OsString>,
    sender_team: String,
    identity: String,
    timeout: Duration,
}

impl Default for AtmSender {
    fn default() -> Self {
        Self {
            binary: OsString::from("atm"),
            leading_args: Vec::new(),
            sender_team: SENDER_TEAM.to_owned(),
            identity: SENDER_IDENTITY.to_owned(),
            timeout: SEND_TIMEOUT,
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
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The binary that is run.
    #[must_use]
    pub fn binary(&self) -> &str {
        self.binary.to_str().unwrap_or("atm")
    }

    /// The arguments of one send, after the leading ones.
    fn args(&self, lead: &str, team: &str) -> Vec<OsString> {
        [
            "send",
            &format!("{lead}@{team}"),
            "--team",
            &self.sender_team,
            "--as",
            &self.identity,
            "--stdin",
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }
}

/// True for a name that cannot be read as an option or split into two arguments.
fn addressable(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '@')
}

fn refuse(detail: impl Into<String>) -> SendError {
    SendError {
        detail: detail.into(),
    }
}

/// Reads up to [`CAPTURE_LIMIT`] bytes and discards the rest, so a chatty
/// child never blocks on a full pipe.
fn capture(mut from: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buf = [0_u8; 1024];
        while let Ok(n) = from.read(&mut buf) {
            if n == 0 {
                break;
            }
            if kept.len() < CAPTURE_LIMIT {
                let room = CAPTURE_LIMIT - kept.len();
                kept.extend_from_slice(buf.get(..n.min(room)).unwrap_or_default());
            }
        }
        // The receiver may be gone after a timeout; nobody is waiting.
        let _ = tx.send(String::from_utf8_lossy(&kept).trim().to_owned());
    });
    rx
}

impl EscalationSender for AtmSender {
    fn send(&self, lead: &str, team: &str, body: &str) -> Result<(), SendError> {
        if !addressable(lead) || !addressable(team) {
            return Err(refuse(format!(
                "{lead:?}@{team:?} is not an address atm send can take"
            )));
        }
        let mut child = Command::new(&self.binary)
            .args(&self.leading_args)
            .args(self.args(lead, team))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| refuse(format!("cannot start {}: {e}", self.binary())))?;
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
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= self.timeout => {
                    // Kill, then reap, so no zombie outlives the failed send.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(refuse(format!(
                        "{} did not finish within {} ms and was killed",
                        self.binary(),
                        self.timeout.as_millis()
                    )));
                }
                Ok(None) => thread::sleep(POLL),
                Err(e) => return Err(refuse(format!("waiting for {}: {e}", self.binary()))),
            }
        };
        if status.success() {
            return Ok(());
        }
        let said = |rx: Option<mpsc::Receiver<String>>| {
            rx.and_then(|rx| rx.recv_timeout(POLL * 10).ok())
                .unwrap_or_default()
        };
        let (out, err) = (said(stdout), said(stderr));
        let detail = if err.is_empty() { out } else { err };
        Err(refuse(format!(
            "{} exited with {status}{}",
            self.binary(),
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        )))
    }
}
