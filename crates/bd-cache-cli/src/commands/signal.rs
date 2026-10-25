//! bdc's handling of `SIGTERM`, `SIGINT` and `SIGHUP` (RSH-001, ADR-CLI-0005).
//!
//! Every `bd` child leads its own process group, so a signal sent to bdc (or to
//! the terminal's foreground group) never reaches it. Left to the default
//! action, bdc would die and leave the `bd` (and a `dolt` it started) running,
//! with no project lock held. So on Unix [`install`] takes over the three
//! signals before any command runs. A signal then:
//!
//! 1. asks every bounded `bd` call to stop ([`sc_bd_cli::shutdown::request`]):
//!    a running child's process group is killed and reaped exactly as on a
//!    timeout, and the in-process and cross-process project locks are released
//!    by the call's normal return;
//! 2. sets the command's [`StopFlag`], which `cache tick` passes to the tick and
//!    to its [`AtmSender`](bd_cache_sync::AtmSender), so the tick ends early and
//!    an `atm` child is killed with its group;
//! 3. leaves the command to unwind normally: it writes its envelope, and
//!    [`run_process`](crate::run_process) exits with `128 + signal` (143 for
//!    `SIGTERM`), the shell's convention for a signal death.
//!
//! The signals are received by `signal-hook`'s iterator on a thread of its
//! own, so no code of ours runs in signal context. A second signal while the
//! first is being handled changes nothing: the shutdown is already bounded by
//! the calls' kill bound, and `SIGKILL` remains for an impatient caller (on
//! Linux the `bd` child also gets `PR_SET_PDEATHSIG`, see
//! [`sc_bd_cli::bounded`]).
//!
//! On other platforms [`install`] does nothing: Windows has no such signals,
//! and a console Ctrl-C there is out of this module's scope.

use std::io;

use bd_cache_sync::StopFlag;

/// The exit code of a process stopped by `signal`: `128 + signal`.
///
/// A recorded signal that is not a positive number (a request without one) is
/// reported as `SIGTERM`'s 143.
#[must_use]
pub(crate) fn exit_code(signal: i32) -> u8 {
    /// `SIGTERM`, the default stop signal, on every Unix.
    const SIGTERM: i32 = 15;
    let signal = if signal > 0 { signal } else { SIGTERM };
    u8::try_from(128 + signal).unwrap_or(128 + 15)
}

/// The signals bdc stops on.
#[cfg(unix)]
pub(crate) const SIGNALS: [i32; 3] = [
    signal_hook::consts::SIGTERM,
    signal_hook::consts::SIGINT,
    signal_hook::consts::SIGHUP,
];

/// Takes over [`SIGNALS`] for the rest of the process: each one requests the
/// shutdown of the bounded `bd` calls and sets `stop`.
///
/// # Errors
///
/// The OS error when a handler cannot be registered or the receiving thread
/// cannot be started; the signals then keep their default action.
#[cfg(unix)]
pub(crate) fn install(stop: &StopFlag) -> io::Result<()> {
    let mut signals = signal_hook::iterator::Signals::new(SIGNALS)?;
    let stop = stop.clone();
    std::thread::Builder::new()
        .name("bdc-signals".to_owned())
        .spawn(move || {
            for signal in signals.forever() {
                sc_bd_cli::shutdown::request(signal);
                stop.stop();
            }
        })
        .map(drop)
}

/// Does nothing: this platform has no `SIGTERM`, `SIGINT` or `SIGHUP`.
///
/// # Errors
///
/// Never.
#[cfg(not(unix))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the Unix form can fail; one signature for both"
)]
pub(crate) fn install(stop: &StopFlag) -> io::Result<()> {
    let _ = stop;
    Ok(())
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
            .args(["--repo", repo.to_str().unwrap(), "--bd", bd.to_str().unwrap()])
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
