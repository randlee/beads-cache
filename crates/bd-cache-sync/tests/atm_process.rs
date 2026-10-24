//! `AtmSender` against a real child process (REQ-BDC-0704..0706; ADR-SYNC-0007).
//!
//! The test binary is its own fake `atm` (no libtest harness): run with
//! `--fake-atm=<mode> <out-prefix> send <to> ...` it behaves as the named
//! mode of `atm`, so the sender spawns and talks to a real process on every
//! platform without a shell script. The invocation checked is the reference's
//! (`bd_cache_cron.py` at atm-core `05831df`): `send <lead>@<team> --team tool
//! --stdin --json` with `ATM_IDENTITY=cron` and `ATM_TEAM=tool`.
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
/// How long a test waits for a child to show it started, or to be gone.
const WAIT: Duration = Duration::from_secs(10);
/// How long a hanging fake lives at most, so a failing test leaves no long-lived orphan.
const FAKE_LIFETIME: Duration = Duration::from_secs(25);

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
            std::fs::write(format!("{out}.pid"), std::process::id().to_string()).unwrap();
            std::thread::sleep(FAKE_LIFETIME);
            0
        }
        // Starts a grandchild that hangs, then hangs itself.
        "tree" => {
            std::fs::write(format!("{out}.pid"), std::process::id().to_string()).unwrap();
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

/// Waits until `path` exists and returns the pid in it.
fn pid_in(path: &str) -> u32 {
    let until = Instant::now() + WAIT;
    loop {
        if let Some(pid) = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| text.trim().parse().ok())
        {
            return pid;
        }
        assert!(
            Instant::now() < until,
            "{path} never appeared: the fake did not start"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether a process with this pid is running.
fn alive(pid: u32) -> bool {
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

/// Waits (up to [`WAIT`]) until the process is gone.
fn assert_gone(pid: u32, what: &str) {
    let until = Instant::now() + WAIT;
    while alive(pid) {
        assert!(
            Instant::now() < until,
            "{what} (pid {pid}) is still running"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
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

/// The pid in `path`, if the fake has written it by now.
fn pid_now(path: &str) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse().ok())
}

/// After a timer-driven kill: whatever the fakes had started by then is gone.
/// (A fake killed before it wrote its pid has nothing left to leak; the
/// process-tree guarantee itself is asserted by the stop-flag test, which
/// waits for both processes before it stops them.)
fn assert_started_ones_are_gone(prefix: &str, reaper: &Reaper) {
    for file in [format!("{prefix}.pid"), format!("{prefix}.grandchild.pid")] {
        if let Some(pid) = pid_now(&file) {
            reaper.add(pid);
            assert_gone(pid, &file);
        }
    }
}

fn an_atm_that_hangs_is_killed_and_gone_at_the_timeout() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("hung");
    let err = sender("hang", &out)
        .with_timeout(Duration::from_secs(3))
        .unwrap()
        .send("l", "t", "hi")
        .unwrap_err();
    assert_kind(&err, SendErrorKind::TimedOut);
    assert!(err.detail.contains("did not finish"), "{err}");
    assert_started_ones_are_gone(&out.display().to_string(), &reaper);
}

fn the_whole_process_tree_is_killed_at_the_timeout_not_just_the_child() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("tree");
    let err = sender("tree", &out)
        .with_timeout(Duration::from_secs(6))
        .unwrap()
        .send("l", "t", "hi")
        .unwrap_err();
    assert_kind(&err, SendErrorKind::TimedOut);
    assert_started_ones_are_gone(&out.display().to_string(), &reaper);
}

fn the_time_a_tick_passes_bounds_the_send() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("within");
    let started = Instant::now();
    let err = sender("hang", &out)
        .send_within("l", "t", "hi", Duration::from_millis(250))
        .unwrap_err();
    assert_kind(&err, SendErrorKind::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert_started_ones_are_gone(&out.display().to_string(), &reaper);
    let none = sender("hang", &out)
        .send_within("l", "t", "hi", Duration::ZERO)
        .unwrap_err();
    assert_kind(&none, SendErrorKind::TimedOut);
}

fn the_stop_flag_kills_the_child_and_its_descendants() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("stopped");
    let stop = StopFlag::new();
    // The stop comes only once both processes have started.
    let watcher = {
        let (flag, prefix, found) = (stop.clone(), out.display().to_string(), reaper.shared());
        std::thread::spawn(move || {
            // Each pid is recorded in the reaper's list as soon as it is read,
            // so a panic after the first leaves it known and killed.
            let track = |path: String| {
                let pid = pid_in(&path);
                found
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(pid);
                pid
            };
            let pids = (
                track(format!("{prefix}.pid")),
                track(format!("{prefix}.grandchild.pid")),
            );
            flag.stop();
            pids
        })
    };
    let started = Instant::now();
    let result = sender("tree", &out)
        .with_timeout(Duration::from_secs(20))
        .unwrap()
        .with_stop(stop)
        .send("l", "t", "hi");
    let (child, grandchild) = watcher.join().unwrap();
    let err = result.unwrap_err();
    assert_kind(&err, SendErrorKind::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(15));
    assert_gone(child, "the child");
    assert_gone(grandchild, "the grandchild");
}

/// A failing atm whose descendant keeps the output pipes open: the send fails
/// at once with what it could read, and does not wait for the descendant (QA3-003).
fn a_failing_atm_with_a_descendant_holding_its_pipes_does_not_wait_for_it() {
    let reaper = Reaper::default();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("orphan");
    let started = Instant::now();
    let err = sender("fail-orphan", &out)
        .send("l", "t", "hi")
        .unwrap_err();
    let took = started.elapsed();
    reaper.track(&format!("{}.grandchild.pid", out.display()));
    assert_kind(&err, SendErrorKind::Exited);
    assert!(
        took < Duration::from_secs(3),
        "waited {took:?} for the descendant"
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

fn a_timeout_that_no_message_could_use_is_refused() {
    let s = AtmSender::default();
    assert!(s.clone().with_timeout(Duration::ZERO).is_err());
    assert!(s.clone().with_timeout(TICK_BUDGET).is_err());
    assert!(s.with_timeout(Duration::from_secs(5)).is_ok());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(mode) = args.first().and_then(|a| a.strip_prefix(FLAG)) {
        let out = args.get(1).cloned().unwrap_or_default();
        let code = fake_atm(mode, &out, &args);
        std::io::stdout().flush().unwrap();
        std::process::exit(code);
    }
    let tests: [(&str, fn()); 14] = [
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
            "an_atm_that_hangs_is_killed_and_gone_at_the_timeout",
            an_atm_that_hangs_is_killed_and_gone_at_the_timeout,
        ),
        (
            "the_whole_process_tree_is_killed_at_the_timeout_not_just_the_child",
            the_whole_process_tree_is_killed_at_the_timeout_not_just_the_child,
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
