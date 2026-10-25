//! `AtmSender` against a real child process (REQ-BDC-0704..0706; ADR-SYNC-0007).
//!
//! The test binary is its own fake `atm` (no libtest harness): run with
//! `--fake-atm=<mode> <out-prefix> send <to> ...` it behaves as the named
//! mode of `atm`, so the sender spawns and talks to a real process on every
//! platform without a shell script. The invocation checked is the reference's
//! (`bd_cache_cron.py` at atm-core `05831df`): `send <lead>@<team> --team tool
//! --stdin --json` with `ATM_IDENTITY=cron` and `ATM_TEAM=tool`.
//!
//! Two tests need a process of their own, a **host** (`--host-<mode>`): one
//! that can start no program when its send is stopped (RSH-002), and one whose
//! `PATH` and working directory are the test's (RSH-003).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bd_cache_model::{EscalationSender, SendErrorKind};
use bd_cache_sync::{AtmSender, StopFlag, TICK_BUDGET};

const FLAG: &str = "--fake-atm=";
/// Runs this binary as a host that sends once while it can start no program.
const HOST_NO_HELPERS: &str = "--host-no-helpers=";
/// Runs this binary as a host that sends once with the default `atm`.
const HOST_RESOLVE: &str = "--host-resolve";
/// How long a test waits for a child to show it started, or to be gone.
const WAIT: Duration = Duration::from_secs(10);
/// How long a hanging fake lives at most, so a failing test leaves no long-lived orphan.
const FAKE_LIFETIME: Duration = Duration::from_secs(25);
/// The sender's own timeout in the stop-flag tests: far above a stop that
/// comes once the fakes are ready, so the kill is the stop's, never the timer's.
const GENEROUS: Duration = Duration::from_secs(20);

/// Writes this process's pid to `path` whole: a temporary file renamed into
/// place, newline-terminated, so a reader sees no file or the whole pid, never
/// part of one (FTQ-023).
fn write_pid(path: &str) {
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, format!("{}\n", std::process::id())).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

/// The pid in a file [`write_pid`] wrote; `None` while it is absent, empty or
/// not newline-terminated (not written whole).
fn read_pid(path: &str) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()?
        .strip_suffix('\n')?
        .trim()
        .parse()
        .ok()
}

/// The fake `atm`: `<exe> --fake-atm=<mode> <out-prefix> <atm args...>`.
fn fake_atm(mode: &str, out: &str, args: &[String]) -> i32 {
    match mode {
        "record" | "fail" | "chatty" => {
            let mut stdin = Vec::new();
            std::io::stdin().read_to_end(&mut stdin).unwrap();
            std::fs::write(format!("{out}.args"), args.join("\n")).unwrap();
            std::fs::write(format!("{out}.stdin"), stdin).unwrap();
            let env = format!(
                "{}\n{}",
                std::env::var("ATM_IDENTITY").unwrap_or_default(),
                std::env::var("ATM_TEAM").unwrap_or_default()
            );
            std::fs::write(format!("{out}.env"), env).unwrap();
            match mode {
                "fail" => {
                    eprintln!("error: mailbox full");
                    3
                }
                "chatty" => {
                    // Far more than the sender keeps.
                    let noise = "z".repeat(256 * 1024);
                    let _ = std::io::stderr().write_all(noise.as_bytes());
                    3
                }
                _ => 0,
            }
        }
        // Exits at once without reading its input.
        "deaf" => 0,
        // Records its pid at once, then never finishes.
        "hang" => {
            write_pid(&format!("{out}.pid"));
            std::thread::sleep(FAKE_LIFETIME);
            0
        }
        // Starts a grandchild that hangs, then hangs itself.
        "tree" => {
            write_pid(&format!("{out}.pid"));
            let exe = std::env::current_exe().unwrap();
            #[expect(
                clippy::zombie_processes,
                reason = "the grandchild is meant to outlive its parent unless the group is killed"
            )]
            let _grandchild = std::process::Command::new(exe)
                .args([&format!("{FLAG}hang"), &format!("{out}.grandchild")])
                .spawn()
                .unwrap();
            std::thread::sleep(FAKE_LIFETIME);
            0
        }
        // Fails with a message while a descendant keeps its output pipes open.
        "fail-orphan" => {
            let exe = std::env::current_exe().unwrap();
            #[expect(
                clippy::zombie_processes,
                reason = "the grandchild outlives its parent on purpose"
            )]
            let _grandchild = std::process::Command::new(exe)
                .args([&format!("{FLAG}hang"), &format!("{out}.grandchild")])
                .spawn()
                .unwrap();
            eprintln!("error: mailbox full");
            3
        }
        other => panic!("unknown fake atm mode {other}"),
    }
}

fn sender(mode: &str, out: &Path) -> AtmSender {
    AtmSender::with_binary(std::env::current_exe().unwrap())
        .with_leading_args([format!("{FLAG}{mode}"), out.display().to_string()])
}

/// Waits (up to [`WAIT`]) until `path` holds a whole pid and returns it.
fn pid_in(path: &str) -> u32 {
    let until = Instant::now() + WAIT;
    loop {
        if let Some(pid) = read_pid(path) {
            return pid;
        }
        assert!(
            Instant::now() < until,
            "{path} never appeared: the fake did not start"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether a process with this pid exists (on Unix a zombie still does).
fn exists(pid: u32) -> bool {
    #[cfg(unix)]
    let status = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success());
    #[cfg(windows)]
    let status = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()));
    status.unwrap_or(false)
}

/// Whether the process is a zombie: dead, not yet reaped by its parent. On
/// Linux the state field of `/proc/<pid>/stat` (after the `)` that ends the
/// command name), on other Unixes `ps -o stat=`; both say `Z`. Windows has none.
fn zombie(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    let z = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            let (_, rest) = stat.rsplit_once(')')?;
            Some(rest.trim_start().starts_with('Z'))
        })
        .unwrap_or(false);
    #[cfg(all(unix, not(target_os = "linux")))]
    let z = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok_and(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim_start()
                .starts_with('Z')
        });
    #[cfg(not(unix))]
    let z = {
        let _ = pid;
        false
    };
    z
}

/// Waits (up to [`WAIT`]) until `running(pid)` is false.
fn wait_until_not(running: impl Fn(u32) -> bool, pid: u32, what: &str, state: &str) {
    let until = Instant::now() + WAIT;
    while running(pid) {
        assert!(Instant::now() < until, "{what} (pid {pid}) {state}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Waits until a process the sender owns (the `atm` child it spawned) no
/// longer exists: killed and reaped. A zombie fails it: reaping its own child
/// is the sender's job (S28).
fn assert_reaped(pid: u32, what: &str) {
    wait_until_not(
        exists,
        pid,
        what,
        "still exists: not killed or left a zombie",
    );
}

/// Kills the pids it holds when dropped, so a failing test leaves no orphan
/// sleeping for a minute.
#[derive(Default)]
struct Reaper(Arc<Mutex<Vec<u32>>>);

impl Reaper {
    /// Keeps `pid` to be killed.
    fn add(&self, pid: u32) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(pid);
    }

    /// Reads the pid in `path` (waiting for it) and keeps it to be killed.
    fn track(&self, path: &str) -> u32 {
        let pid = pid_in(path);
        self.add(pid);
        pid
    }

    /// A handle to the same list, for a thread that discovers pids: what it
    /// found before it panicked is still killed when the reaper drops.
    fn shared(&self) -> Arc<Mutex<Vec<u32>>> {
        Arc::clone(&self.0)
    }
}

impl Drop for Reaper {
    fn drop(&mut self) {
        let pids = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for pid in pids.iter() {
            #[cfg(unix)]
            let _ = std::process::Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status();
            #[cfg(windows)]
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output();
        }
    }
}

fn delivers_the_body_on_stdin_with_the_reference_invocation() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("sent");
    let body = "bd-cache: write #7 needs you.\nError: \"quoted\" -- --option $HOME\n";
    sender("record", &out)
        .send("lead-x", "atm-dev", body)
        .unwrap();
    let args = std::fs::read_to_string(format!("{}.args", out.display())).unwrap();
    // The fake receives its own flags first; `atm`'s are the rest.
    let atm_args: Vec<&str> = args.lines().skip(2).collect();
    assert_eq!(
        atm_args,
        [
            "send",
            "lead-x@atm-dev",
            "--team",
            "tool",
            "--stdin",
            "--json"
        ]
    );
    let env = std::fs::read_to_string(format!("{}.env", out.display())).unwrap();
    assert_eq!(env, "cron\ntool", "ATM_IDENTITY and ATM_TEAM");
    let stdin = std::fs::read_to_string(format!("{}.stdin", out.display())).unwrap();
    assert_eq!(stdin, body, "the message is on stdin, not in the arguments");
}

fn a_large_message_is_not_limited_by_the_argument_list() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("big");
    let body = "x".repeat(512 * 1024);
    sender("record", &out).send("l", "t", &body).unwrap();
    let got = std::fs::metadata(format!("{}.stdin", out.display()))
        .unwrap()
        .len();
    assert_eq!(got, 512 * 1024);
}

fn a_failing_atm_is_a_send_error_that_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("failed");
    let err = sender("fail", &out).send("l", "t", "hi").unwrap_err();
    assert_eq!(err.kind, SendErrorKind::Exited);
    assert!(err.detail.contains("exit"), "{err}");
    assert!(err.detail.contains("mailbox full"), "{err}");
    assert!(!err.fix().is_empty());
}

fn output_beyond_the_limit_is_dropped_and_the_error_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("chatty");
    let err = sender("chatty", &out).send("l", "t", "hi").unwrap_err();
    assert!(
        err.detail.contains("(output truncated)"),
        "{}",
        err.detail.len()
    );
    assert!(err.detail.len() < 8 * 1024, "{} bytes", err.detail.len());
}

/// Runs one send of the fake `mode` under a stop flag and the [`GENEROUS`]
/// timeout, and stops it once every pid file of `ready` (suffixes of `out`)
/// was written whole (FTQ-026): a watcher waits for them, bounded by
/// [`pid_in`], then calls `stop()`, so the fakes are ready before the kill by
/// construction. Returns the error and the pids, in the order of `ready`;
/// each pid is kept by `reaper` as soon as it is read, so a panic leaves no
/// orphan.
fn stopped_once_ready(
    mode: &str,
    out: &Path,
    ready: &[&str],
    reaper: &Reaper,
) -> (bd_cache_model::SendError, Vec<u32>) {
    let (err, pids, ()) = stopped_once_inspected(mode, out, ready, reaper, |_| ());
    (err, pids)
}

/// [`stopped_once_ready`], with `inspect` run on the pids once all are ready
/// and before the stop, while every fake is still running; its result is
/// returned with the error and the pids.
fn stopped_once_inspected<T: Send + 'static>(
    mode: &str,
    out: &Path,
    ready: &[&str],
    reaper: &Reaper,
    inspect: impl FnOnce(&[u32]) -> T + Send + 'static,
) -> (bd_cache_model::SendError, Vec<u32>, T) {
    let stop = StopFlag::new();
    let watcher = {
        let flag = stop.clone();
        let found = reaper.shared();
        let paths: Vec<String> = ready
            .iter()
            .map(|suffix| format!("{}{suffix}", out.display()))
            .collect();
        std::thread::spawn(move || {
            let pids: Vec<u32> = paths
                .iter()
                .map(|path| {
                    let pid = pid_in(path);
                    found
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(pid);
                    pid
                })
                .collect();
            // Stopped even if `inspect` panics, so the send never runs to its timeout.
            let inspected =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inspect(&pids)));
            flag.stop();
            (pids, inspected)
        })
    };
    let result = sender(mode, out)
        .with_timeout(GENEROUS)
        .unwrap()
        .with_stop(stop)
        .send("l", "t", "hi");
    let (pids, inspected) = watcher.join().unwrap();
    let inspected = inspected.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    (result.unwrap_err(), pids, inspected)
}

/// A hung `atm` is killed and reaped (stop-flag form, FTQ-026: the kill comes
/// after the fake is ready, never racing its start).
fn an_atm_that_hangs_is_killed_and_reaped() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let (err, pids) = stopped_once_ready("hang", &dir.path().join("hung"), &[".pid"], &reaper);
    assert_kind(&err, SendErrorKind::Cancelled);
    assert_reaped(pids[0], "the hung atm");
}

/// The stop flag kills the send, child and tree (stop-flag form, FTQ-026).
///
/// Only the child's reaping is asserted here (FTQ-030): the tree kill runs
/// under the production 1 s `KILL_BOUND`, so a host stalled past 1 s gives
/// it up and a descendant check would fail on the host, not on the code.
/// That the tree kill takes the descendant is the unit test
/// `a_child_that_ignores_term_and_its_descendant_are_killed_by_the_real_tree_kill`
/// in `src/atm.rs`, under its injected 30 s deadline. The grandchild's pid
/// is still tracked, so the reaper kills it if it is left.
fn the_stop_flag_kills_the_child_and_its_descendants() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let (err, pids) = stopped_once_ready(
        "tree",
        &dir.path().join("stopped"),
        &[".pid", ".grandchild.pid"],
        &reaper,
    );
    assert_kind(&err, SendErrorKind::Cancelled);
    assert_reaped(pids[0], "the child");
    assert_eq!(pids.len(), 2, "the grandchild was started and is tracked");
}

/// The process group of `pid` (`ps -o pgid=`); `None` once it is gone.
#[cfg(unix)]
fn pgid(pid: u32) -> Option<u32> {
    let out = std::process::Command::new("ps")
        .args(["-o", "pgid=", "-p", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// The `atm` child leads its own process group and its descendants are in
/// it, so the tree kill (`kill -KILL -- -<child pid>`) reaches them. Without
/// `process_group(0)` the child is in the test's group: the group kill finds
/// no group of that id and only the child dies, leaving the descendant
/// running (the planted bug "atm kills only the direct child").
///
/// Causal, no clock (FTQ-030): the groups are read while both fakes are
/// running, before the stop, so a stalled host cannot change the answer.
/// That the group kill takes a descendant is the unit test
/// `a_child_that_ignores_term_and_its_descendant_are_killed_by_the_real_tree_kill`.
#[cfg(unix)]
fn the_atm_child_leads_a_process_group_that_holds_its_descendants() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let (err, pids, groups) = stopped_once_inspected(
        "tree",
        &dir.path().join("group"),
        &[".pid", ".grandchild.pid"],
        &reaper,
        |pids| (pgid(pids[0]), pgid(pids[1]), pgid(std::process::id())),
    );
    assert_kind(&err, SendErrorKind::Cancelled);
    let (child, grandchild) = (pids[0], pids[1]);
    let (child_group, grandchild_group, own_group) = groups;
    assert_eq!(
        child_group,
        Some(child),
        "the child (pid {child}) leads its own process group, not the sender's ({own_group:?})"
    );
    assert_eq!(
        grandchild_group,
        Some(child),
        "the grandchild (pid {grandchild}) is in the child's group, which the tree kill reaches"
    );
    assert_ne!(
        own_group,
        Some(child),
        "the sender is not in the child's group"
    );
    assert_reaped(child, "the child");
}
#[cfg(not(unix))]
fn the_atm_child_leads_a_process_group_that_holds_its_descendants() {}

/// The one timer-driven kill (FTQ-026): the time a tick passes bounds the
/// send, shown by the kind and the detail (the tick's 250 ms, not the
/// sender's own timeout), never by the clock (FTQ-027). Whether the fake got
/// as far as its pid file before the kill is not asserted; a pid it wrote is
/// only kept to be killed. Kill and reap mechanics are the stop-flag tests'.
fn the_time_a_tick_passes_bounds_the_send() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("within");
    let err = sender("hang", &out)
        .send_within("l", "t", "hi", Duration::from_millis(250))
        .unwrap_err();
    if let Some(pid) = read_pid(&format!("{}.pid", out.display())) {
        reaper.add(pid);
    }
    assert_kind(&err, SendErrorKind::TimedOut);
    assert!(err.detail.contains("did not finish within 250 ms"), "{err}");
    let none = sender("hang", &dir.path().join("none"))
        .send_within("l", "t", "hi", Duration::ZERO)
        .unwrap_err();
    assert_kind(&none, SendErrorKind::TimedOut);
}

/// A failing atm whose descendant keeps the output pipes open: the send fails
/// at once with what it could read, and does not wait for the descendant (QA3-003).
fn a_failing_atm_with_a_descendant_holding_its_pipes_does_not_wait_for_it() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("orphan");
    let err = sender("fail-orphan", &out)
        .send("l", "t", "hi")
        .unwrap_err();
    let grandchild = reaper.track(&format!("{}.grandchild.pid", out.display()));
    assert_kind(&err, SendErrorKind::Exited);
    // The descendant holds the pipes for its whole life: it still runs once
    // the send has returned, so the send did not wait for it (FTQ-027: an
    // order, not a clock bound).
    assert!(
        exists(grandchild) && !zombie(grandchild),
        "the send waited for the descendant (pid {grandchild})"
    );
    assert!(
        err.detail.contains("mailbox full") || err.detail.contains("not fully read"),
        "{err}"
    );
}

/// On Unix a file without an execute bit is not a program (QA3-008).
#[cfg(unix)]
fn a_file_without_an_execute_bit_does_not_resolve() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("atm");
    std::fs::write(&file, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let s = AtmSender::with_binary(&file);
    assert_eq!(s.resolve().unwrap_err().kind, SendErrorKind::NotStartable);
    assert_eq!(
        s.send("l", "t", "hi").unwrap_err().kind,
        SendErrorKind::NotStartable
    );
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(s.resolve().unwrap(), file);
}
#[cfg(not(unix))]
fn a_file_without_an_execute_bit_does_not_resolve() {}

fn assert_kind(err: &bd_cache_model::SendError, kind: SendErrorKind) {
    assert_eq!(err.kind, kind, "{err}");
}

fn an_atm_that_exits_without_reading_still_counts_as_delivered_by_its_status() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("deaf");
    // Bigger than a pipe buffer, so the write meets a closed pipe.
    let body = "y".repeat(4 * 1024 * 1024);
    assert!(sender("deaf", &out).send("l", "t", &body).is_ok());
}

fn a_missing_binary_is_a_send_error_and_does_not_resolve() {
    let s = AtmSender::with_binary("definitely-not-a-binary-bdc");
    let err = s.send("l", "t", "hi").unwrap_err();
    assert_eq!(err.kind, SendErrorKind::NotStartable);
    assert!(err.detail.contains("cannot start"), "{err}");
    assert_eq!(s.resolve().unwrap_err().kind, SendErrorKind::NotStartable);
    // A real program resolves, by path.
    let exe = std::env::current_exe().unwrap();
    assert_eq!(AtmSender::with_binary(&exe).resolve().unwrap(), exe);
}

fn an_address_that_could_be_read_as_an_option_is_refused_before_any_process_starts() {
    let s = AtmSender::with_binary("definitely-not-a-binary-bdc");
    for (lead, team) in [
        ("--help", "t"),
        ("l", "-t"),
        ("a b", "t"),
        ("l", "t@x"),
        ("", "t"),
        ("l", ""),
    ] {
        let err = s.send(lead, team, "hi").unwrap_err();
        assert_eq!(err.kind, SendErrorKind::InvalidAddress, "{lead:?} {team:?}");
        assert!(
            err.detail.contains("not an address"),
            "{lead:?} {team:?}: {err}"
        );
    }
}

/// The host of [`the_tree_kill_needs_no_kill_program`]: a `tree` send stopped
/// once both fakes run and this process may start no process at all
/// (`RLIMIT_NPROC` 0; its `PATH` is empty too). Prints the error kind and
/// whether spawning was really forbidden.
#[cfg(unix)]
fn host_without_helpers(out: &str) -> i32 {
    let stop = StopFlag::new();
    let watcher = {
        let (flag, out) = (stop.clone(), out.to_owned());
        std::thread::spawn(move || {
            let _ = pid_in(&format!("{out}.pid"));
            let _ = pid_in(&format!("{out}.grandchild.pid"));
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            // SAFETY: `limit` is a valid, writable `rlimit` that outlives the call.
            let got = unsafe { libc::getrlimit(libc::RLIMIT_NPROC, &raw mut limit) };
            limit.rlim_cur = 0;
            // SAFETY: `limit` is a valid `rlimit`, only read by the call. Lowering
            // this test host's soft limit is the point of the test.
            let set = unsafe { libc::setrlimit(libc::RLIMIT_NPROC, &raw const limit) };
            let forbidden = got == 0
                && set == 0
                && std::process::Command::new("/bin/sh")
                    .args(["-c", "true"])
                    .status()
                    .is_err();
            flag.stop();
            forbidden
        })
    };
    let result = sender("tree", Path::new(out))
        .with_timeout(GENEROUS)
        .unwrap()
        .with_stop(stop)
        .send("l", "t", "hi");
    let forbidden = watcher.join().unwrap_or(false);
    println!("forbidden:{forbidden}");
    println!("kind:{:?}", result.err().map(|e| e.kind));
    0
}

/// RSH-002: the tree kill needs no `kill` program. The host's send is stopped
/// when it can start no process and has an empty `PATH`; the TERM-ignoring
/// grandchild still dies with the group. A root host is exempt from
/// `RLIMIT_NPROC`, so there only the kill itself is checked.
#[cfg(unix)]
fn the_tree_kill_needs_no_kill_program() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("nohelpers");
    let host = std::process::Command::new(std::env::current_exe().unwrap())
        .arg(format!("{HOST_NO_HELPERS}{}", out.display()))
        .env("PATH", "")
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&host.stdout).into_owned();
    let grandchild = reaper.track(&format!("{}.grandchild.pid", out.display()));
    assert!(host.status.success(), "the host failed: {host:?}");
    assert!(said.contains("kind:Some(Cancelled)"), "{said}");
    // SAFETY: `geteuid` has no preconditions.
    let root = unsafe { libc::geteuid() } == 0;
    assert!(
        root || said.contains("forbidden:true"),
        "the host could still start a process: {said}"
    );
    wait_until_not(
        |p| exists(p) && !zombie(p),
        grandchild,
        "the grandchild",
        "survived the tree kill",
    );
}

#[cfg(not(unix))]
fn the_tree_kill_needs_no_kill_program() {}

/// RSH-003: an empty or relative `PATH` segment never finds the `atm` in the
/// working directory. With `PATH` `:/usr/bin` and then `.:/usr/bin`, and an
/// executable `atm` in the cwd that writes a marker, the default sender does
/// not run it.
#[cfg(unix)]
fn a_relative_path_segment_never_runs_an_atm_in_the_cwd() {
    use std::os::unix::fs::PermissionsExt as _;
    for path in [":/usr/bin", ".:/usr/bin"] {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker");
        let atm = dir.path().join("atm");
        std::fs::write(
            &atm,
            format!("#!/bin/sh\necho ran > '{}'\n", marker.display()),
        )
        .unwrap();
        std::fs::set_permissions(&atm, std::fs::Permissions::from_mode(0o755)).unwrap();
        let host = std::process::Command::new(std::env::current_exe().unwrap())
            .arg(HOST_RESOLVE)
            .env("PATH", path)
            .current_dir(dir.path())
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&host.stdout).into_owned();
        assert!(host.status.success(), "{path}: the host failed: {host:?}");
        assert!(
            !marker.exists(),
            "{path}: the atm in the working directory ran: {said}"
        );
        assert!(said.contains("resolved:none"), "{path}: {said}");
    }
}

#[cfg(not(unix))]
fn a_relative_path_segment_never_runs_an_atm_in_the_cwd() {}

/// The host of [`a_relative_path_segment_never_runs_an_atm_in_the_cwd`]: what
/// the default sender resolves, and one send.
fn host_resolve() -> i32 {
    let sender = AtmSender::default();
    match sender.resolve() {
        Ok(p) => println!("resolved:{}", p.display()),
        Err(_) => println!("resolved:none"),
    }
    let result = sender.send("lead-x", "atm-dev", "hi");
    println!("kind:{:?}", result.err().map(|e| e.kind));
    0
}

fn a_timeout_that_no_message_could_use_is_refused() {
    let s = AtmSender::default();
    assert!(s.clone().with_timeout(Duration::ZERO).is_err());
    assert!(s.clone().with_timeout(TICK_BUDGET).is_err());
    assert!(s.with_timeout(Duration::from_secs(5)).is_ok());
}

#[expect(
    clippy::too_many_lines,
    reason = "the dispatch to the fakes and hosts, then the test table"
)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(mode) = args.first().and_then(|a| a.strip_prefix(FLAG)) {
        let out = args.get(1).cloned().unwrap_or_default();
        let code = fake_atm(mode, &out, &args);
        std::io::stdout().flush().unwrap();
        std::process::exit(code);
    }
    #[cfg(unix)]
    if let Some(out) = args.first().and_then(|a| a.strip_prefix(HOST_NO_HELPERS)) {
        let code = host_without_helpers(out);
        std::io::stdout().flush().unwrap();
        std::process::exit(code);
    }
    if args.first().map(String::as_str) == Some(HOST_RESOLVE) {
        let code = host_resolve();
        std::io::stdout().flush().unwrap();
        std::process::exit(code);
    }
    let tests: [(&str, fn()); 16] = [
        (
            "the_tree_kill_needs_no_kill_program",
            the_tree_kill_needs_no_kill_program,
        ),
        (
            "a_relative_path_segment_never_runs_an_atm_in_the_cwd",
            a_relative_path_segment_never_runs_an_atm_in_the_cwd,
        ),
        (
            "delivers_the_body_on_stdin_with_the_reference_invocation",
            delivers_the_body_on_stdin_with_the_reference_invocation,
        ),
        (
            "a_large_message_is_not_limited_by_the_argument_list",
            a_large_message_is_not_limited_by_the_argument_list,
        ),
        (
            "a_failing_atm_is_a_send_error_that_says_why",
            a_failing_atm_is_a_send_error_that_says_why,
        ),
        (
            "output_beyond_the_limit_is_dropped_and_the_error_says_so",
            output_beyond_the_limit_is_dropped_and_the_error_says_so,
        ),
        (
            "an_atm_that_hangs_is_killed_and_reaped",
            an_atm_that_hangs_is_killed_and_reaped,
        ),
        (
            "the_time_a_tick_passes_bounds_the_send",
            the_time_a_tick_passes_bounds_the_send,
        ),
        (
            "the_stop_flag_kills_the_child_and_its_descendants",
            the_stop_flag_kills_the_child_and_its_descendants,
        ),
        (
            "the_atm_child_leads_a_process_group_that_holds_its_descendants",
            the_atm_child_leads_a_process_group_that_holds_its_descendants,
        ),
        (
            "a_failing_atm_with_a_descendant_holding_its_pipes_does_not_wait_for_it",
            a_failing_atm_with_a_descendant_holding_its_pipes_does_not_wait_for_it,
        ),
        (
            "a_file_without_an_execute_bit_does_not_resolve",
            a_file_without_an_execute_bit_does_not_resolve,
        ),
        (
            "an_atm_that_exits_without_reading_still_counts_as_delivered_by_its_status",
            an_atm_that_exits_without_reading_still_counts_as_delivered_by_its_status,
        ),
        (
            "a_missing_binary_is_a_send_error_and_does_not_resolve",
            a_missing_binary_is_a_send_error_and_does_not_resolve,
        ),
        (
            "an_address_that_could_be_read_as_an_option_is_refused_before_any_process_starts",
            an_address_that_could_be_read_as_an_option_is_refused_before_any_process_starts,
        ),
        (
            "a_timeout_that_no_message_could_use_is_refused",
            a_timeout_that_no_message_could_use_is_refused,
        ),
    ];
    let mut failed = 0;
    for (name, test) in tests {
        // A failing case is named and the others still run.
        if std::panic::catch_unwind(test).is_ok() {
            println!("test {name} ... ok");
        } else {
            failed += 1;
            println!("test {name} ... FAILED");
        }
    }
    println!(
        "\ntest result: {}. {} passed; {failed} failed",
        if failed == 0 { "ok" } else { "FAILED" },
        tests.len() - failed
    );
    if failed > 0 {
        std::process::exit(101);
    }
}
