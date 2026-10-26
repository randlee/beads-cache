//! bdc's handling of `SIGTERM`, `SIGINT` and `SIGHUP` (RSH-001, ADR-CLI-0005),
//! and of Ctrl-C and Ctrl-Break for `cron run` on Windows (REQ-BDC-0730).
//!
//! Every `bd` child leads its own process group, so a signal sent to bdc (or to
//! the terminal's foreground group) never reaches it. Left to the default
//! action, bdc would die and leave the `bd` (and a `dolt` it started) running,
//! with no project lock held. So on Unix [`install`] takes over the three
//! signals before any command runs. A signal is handled by [`Signals`]:
//!
//! 1. its number is recorded (the first one; a later one changes nothing);
//! 2. the command's [`StopFlag`] is set: `cache tick` and `sync use-python`
//!    stop between steps and kill a running hook's or script's process tree,
//!    and `cache cron run` starts no new run;
//! 3. every bounded `bd` call is asked to stop ([`sc_bd_cli::shutdown::request`]:
//!    a running child's process group is killed and reaped exactly as on a
//!    timeout, and the project locks are released by the call's normal
//!    return) — at once for every command but `cache cron run`, which defers
//!    it ([`Signals::defer_bd_kill`]): its runs in flight, their bd calls
//!    included, get up to `--shutdown-timeout-ms`, and only when that expires
//!    are the bd calls stopped ([`Signals::kill_bd`]) and the trees killed
//!    (Rand's ruling 5, REQ-BDC-0730);
//! 4. the command unwinds normally: it writes its envelope, and
//!    [`run_process`](crate::run_process) exits with `128 + signal` (143 for
//!    `SIGTERM`), the shell's convention for a signal death.
//!
//! The signals are received by `signal-hook`'s iterator on a thread of its
//! own, so no code of ours runs in signal context. On Linux the `bd` child
//! also gets `PR_SET_PDEATHSIG` (see [`sc_bd_cli::bounded`]). On Windows,
//! where there are no such signals, `cron run` registers Ctrl-C and
//! Ctrl-Break itself ([`console_stop`]), recorded as `SIGINT` (exit 130).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use bd_cache_sync::StopFlag;
#[cfg(windows)]
use sc_beads::logging::tracing;

/// `SIGTERM`'s number on every Unix: the default stop signal.
const SIGTERM: i32 = 15;

/// The exit code of a process stopped by `signal`: `128 + signal`.
///
/// A recorded signal that is not a positive number (a request without one) is
/// reported as `SIGTERM`'s 143.
#[must_use]
pub(crate) fn exit_code(signal: i32) -> u8 {
    let signal = if signal > 0 { signal } else { SIGTERM };
    u8::try_from(128 + signal).unwrap_or(128 + 15)
}

/// What the process's signals did: the flag the command watches, the first
/// signal's number, and whether `bd` calls are stopped at once (module docs).
#[derive(Debug, Clone, Default)]
pub(crate) struct Signals {
    stop: StopFlag,
    first: Arc<AtomicI32>,
    defer: Arc<AtomicBool>,
}

impl Signals {
    /// No signal yet.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The flag a signal sets.
    #[must_use]
    pub(crate) fn stop(&self) -> &StopFlag {
        &self.stop
    }

    /// The first signal received, if any.
    #[must_use]
    pub(crate) fn first(&self) -> Option<i32> {
        match self.first.load(Ordering::SeqCst) {
            0 => None,
            n => Some(n),
        }
    }

    /// Handles `signal` (module docs, steps 1 to 3).
    pub(crate) fn received(&self, signal: i32) {
        let _first =
            self.first
                .compare_exchange(0, signal.max(1), Ordering::SeqCst, Ordering::SeqCst);
        self.stop.stop();
        if !self.defer.load(Ordering::SeqCst) {
            self.kill_bd();
        }
    }

    /// From now on a signal leaves the `bd` calls running: `cron run` stops
    /// them itself when its shutdown timeout expires ([`Signals::kill_bd`]).
    pub(crate) fn defer_bd_kill(&self) {
        self.defer.store(true, Ordering::SeqCst);
    }

    /// Asks every bounded `bd` call to stop, as the first signal would.
    pub(crate) fn kill_bd(&self) {
        sc_bd_cli::shutdown::request(self.first().unwrap_or(SIGTERM));
    }
}

/// The signals bdc stops on.
#[cfg(unix)]
pub(crate) const SIGNALS: [i32; 3] = [
    signal_hook::consts::SIGTERM,
    signal_hook::consts::SIGINT,
    signal_hook::consts::SIGHUP,
];

/// Takes over [`SIGNALS`] for the rest of the process: each one goes to
/// [`Signals::received`]. Unix only: the caller is cfg-gated, so other
/// platforms have no such function at all.
///
/// # Errors
///
/// The OS error when a handler cannot be registered or the receiving thread
/// cannot be started; the signals then keep their default action.
#[cfg(unix)]
pub(crate) fn install(signals: &Signals) -> std::io::Result<()> {
    let mut iter = signal_hook::iterator::Signals::new(SIGNALS)?;
    let signals = signals.clone();
    std::thread::Builder::new()
        .name("bdc-signals".to_owned())
        .spawn(move || {
            for signal in iter.forever() {
                signals.received(signal);
            }
        })
        .map(drop)
}

/// `SIGINT`'s number: Ctrl-C and Ctrl-Break exit 130 as Ctrl-C does (REQ-BDC-0730).
#[cfg(windows)]
const SIGINT: i32 = 2;

/// Ctrl-C and Ctrl-Break stop `cron run` on Windows (exit 130, REQ-BDC-0730):
/// a task on the current runtime that hands the first of them to
/// [`Signals::received`]. A handler that cannot be registered is logged at
/// `WARN`: `cron run` then cannot be stopped gracefully that way.
#[cfg(windows)]
pub(crate) fn console_stop(signals: &Signals) -> tokio::task::JoinHandle<()> {
    let signals = signals.clone();
    tokio::spawn(async move {
        let mut c = tokio::signal::windows::ctrl_c()
            .inspect_err(|e| {
                tracing::warn!(error = %e, "cannot handle Ctrl-C; cron run cannot be stopped with it");
            })
            .ok();
        let mut b = tokio::signal::windows::ctrl_break()
            .inspect_err(|e| {
                tracing::warn!(error = %e, "cannot handle Ctrl-Break; cron run cannot be stopped with it");
            })
            .ok();
        let ctrl_c = async {
            match c.as_mut() {
                Some(c) => {
                    c.recv().await;
                }
                None => std::future::pending().await,
            }
        };
        let ctrl_break = async {
            match b.as_mut() {
                Some(b) => {
                    b.recv().await;
                }
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            () = ctrl_c => {}
            () = ctrl_break => {}
        }
        signals.received(SIGINT);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exit_code_is_128_plus_the_signal() {
        assert_eq!(exit_code(15), 143);
        assert_eq!(exit_code(2), 130);
        assert_eq!(exit_code(1), 129);
        assert_eq!(exit_code(-1), 143);
    }

    /// The first signal is the one recorded, and it sets the stop flag; a
    /// deferred process leaves the bd calls running (cron run's grace).
    /// Planted negative: a second signal does not replace the first.
    #[test]
    fn the_first_signal_is_recorded_and_sets_the_flag() {
        let signals = Signals::new();
        signals.defer_bd_kill();
        assert_eq!(signals.first(), None);
        signals.received(15);
        signals.received(2);
        assert_eq!(signals.first(), Some(15));
        assert!(signals.stop().is_stopped());
        assert_eq!(
            sc_bd_cli::shutdown::requested(),
            None,
            "deferred: no bd kill yet"
        );
    }
}

/// The real-process test of RSH-001: a `bdc` whose `bd` hangs, with a
/// TERM-ignoring grandchild, gets `SIGTERM`; the grandchild is gone and bdc
/// exits 143 with one envelope on stdout.
///
/// It runs the `bdc` binary next to this test's directory (`target/<profile>/`),
/// which `cargo test` builds whenever the package's integration tests are part
/// of the run (`cargo test --workspace`, `cargo test -p bd-cache-cli`).
#[cfg(all(test, unix))]
mod process_tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        reason = "test code: a failed unwrap is a test failure"
    )]

    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// How long the test waits for a pid file or an exit at most.
    const WAIT: Duration = Duration::from_secs(20);

    /// The `bdc` binary of this build.
    fn bdc() -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let profile = exe.parent().and_then(Path::parent).unwrap();
        let bdc = profile.join("bdc");
        assert!(
            bdc.is_file(),
            "{} is missing: run the test through `cargo test -p bd-cache-cli`, which builds it",
            bdc.display()
        );
        bdc
    }

    /// The pid a script wrote whole to `path`, waited for up to [`WAIT`].
    fn pid_in(path: &Path) -> u32 {
        let until = Instant::now() + WAIT;
        loop {
            if let Some(pid) = std::fs::read_to_string(path)
                .ok()
                .and_then(|t| t.strip_suffix('\n').and_then(|t| t.trim().parse().ok()))
            {
                return pid;
            }
            assert!(Instant::now() < until, "{} never appeared", path.display());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Whether `pid` runs: it exists and is not a zombie.
    fn runs(pid: u32) -> bool {
        Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .is_ok_and(|o| {
                let stat = String::from_utf8_lossy(&o.stdout);
                let stat = stat.trim();
                !stat.is_empty() && !stat.starts_with('Z')
            })
    }

    fn kill(signal: &str, pid: u32) {
        let _ = Command::new("kill")
            .args([signal, &pid.to_string()])
            .stderr(Stdio::null())
            .status();
    }

    #[test]
    fn sigterm_kills_the_bd_group_and_exits_143() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let beads = repo.join(".beads");
        std::fs::create_dir_all(&beads).unwrap();
        std::fs::write(
            beads.join("metadata.json"),
            r#"{"dolt_database": "beads", "project_id": "proj-1"}"#,
        )
        .unwrap();
        std::fs::write(beads.join("bd-cache.toml"), "scopes = [\"x\"]\n").unwrap();
        let (bd_pid, grandchild_pid) = (dir.path().join("bd.pid"), dir.path().join("gc.pid"));
        // The fake bd: any call starts a TERM-ignoring grandchild in its group,
        // says its own pid last, then hangs.
        let bd = dir.path().join("bd");
        std::fs::write(
            &bd,
            format!(
                "#!/bin/sh\n\
                 /bin/sh -c 'trap \"\" TERM; echo $$ > \"$1.tmp\"; mv \"$1.tmp\" \"$1\"; exec sleep 300' sh '{gc}' &\n\
                 while [ ! -f '{gc}' ]; do sleep 0.01; done\n\
                 echo $$ > '{bd}.tmp'; mv '{bd}.tmp' '{bd}'\n\
                 exec sleep 300\n",
                gc = grandchild_pid.display(),
                bd = bd_pid.display(),
            ),
        )
        .unwrap();
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&bd, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut bdc = Command::new(bdc())
            .args([
                "--repo",
                repo.to_str().unwrap(),
                "--bd",
                bd.to_str().unwrap(),
            ])
            .args(["cache", "refresh", "scopes", "x"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("TMPDIR", std::env::temp_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Ready: the bd and its grandchild run, so bdc is inside a bounded call.
        let grandchild = pid_in(&grandchild_pid);
        let bd_leader = pid_in(&bd_pid);
        kill("-TERM", bdc.id());
        let until = Instant::now() + WAIT;
        let status = loop {
            if let Some(status) = bdc.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= until {
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let grandchild_ran = runs(grandchild);
        for pid in [grandchild, bd_leader] {
            if runs(pid) {
                kill("-KILL", pid);
            }
        }
        let Some(status) = status else {
            let _ = bdc.kill();
            panic!("bdc did not exit after SIGTERM");
        };
        let out = bdc.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(
            status.code(),
            Some(143),
            "{status:?}, stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!grandchild_ran, "the grandchild {grandchild} survived bdc");
        assert!(!runs(bd_leader), "the bd {bd_leader} survived bdc");
        let envelope: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
        assert!(envelope.get("ok").is_some(), "{stdout}");
    }
}
