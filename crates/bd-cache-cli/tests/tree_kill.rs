//! A hook's and a Python row's process tree is killed whole, on every OS:
//! a process group on Unix, a job object on Windows (REQ-BDC-0723,
//! REQ-CRON-0018, NFR-BDC-0004; acceptance 7 and 9). And on Windows, a
//! Ctrl-Break to `cache cron run` waits up to `--shutdown-timeout-ms`, then
//! kills the job object's children, releases the lease and exits 130
//! (REQ-BDC-0730).
//!
//! The interpreter is the `fake-python` test binary: its child starts a
//! grandchild of itself and both record their pids and wait. On Windows
//! they ignore Ctrl-C and Ctrl-Break, so only the job object's kill can end
//! them while the test watches. CI runs this file on ubuntu, macOS and
//! windows-latest.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use support::{TestRepo, envelope_json, path_str, two_beads};

/// The stand-in interpreter.
const FAKE_PYTHON: &str = env!("CARGO_BIN_EXE_fake-python");

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(60);
    while !cond() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The pid written whole to `path`.
fn pid_in(path: &Path) -> u32 {
    let mut pid = None;
    wait_for(&path.display().to_string(), || {
        pid = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| t.trim().parse().ok());
        pid.is_some()
    });
    pid.unwrap()
}

/// Whether `pid` still runs.
fn runs(pid: u32) -> bool {
    if cfg!(windows) {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
    } else {
        Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .is_ok_and(|o| {
                let stat = String::from_utf8_lossy(&o.stdout);
                let stat = stat.trim();
                !stat.is_empty() && !stat.starts_with('Z')
            })
    }
}

/// Waits until none of `pids` runs, at most 30 s; returns the ones that still do.
fn gone(pids: &[u32]) -> Vec<u32> {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let alive: Vec<u32> = pids.iter().copied().filter(|p| runs(*p)).collect();
        if alive.is_empty() || Instant::now() >= until {
            return alive;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Kills what is left, so a failed test leaves nothing behind.
fn cleanup(pids: &[u32]) {
    for pid in pids {
        let _ = if cfg!(windows) {
            Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output()
        } else {
            Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .output()
        };
    }
}

/// `bdc` with `FAKE_PY_DIR` set, so `--env FAKE_PY_DIR` hands it to the child.
fn bdc(repo: &TestRepo, dir: &Path, args: &[&str]) -> (i32, serde_json::Value) {
    let root = repo.root();
    let mut full = vec!["--repo", path_str(&root), "--bd", support::FAKE_BD];
    full.extend_from_slice(args);
    let ran = support::hermetic(support::BDC)
        .args(&full)
        .env("FAKE_BD_SCENARIO", repo.scenario_path())
        .env("FAKE_PY_DIR", dir)
        .output()
        .expect("bdc starts");
    envelope_json(&ran.into())
}

/// A hook whose child started a grandchild is killed with it at the hook's
/// deadline; the run still answers `ok` with the hook's `timeout`
/// (acceptance 7). Planted negative: an ordinary hook answers `ok`.
#[test]
fn a_hooks_child_and_grandchild_are_killed_at_its_deadline() {
    let repo = TestRepo::new(&two_beads());
    let dir = tempfile::TempDir::new().unwrap();
    let (exit, v) = bdc(
        &repo,
        dir.path(),
        &[
            "cache",
            "schedule",
            "set",
            "0 0 1 1 *",
            "--budget-ms",
            "6000",
            "--hook-reserve-ms",
            "3000",
            "--post-sync",
            "consumer.hooks:after",
            "--python",
            FAKE_PYTHON,
            "--env",
            "FAKE_PY_DIR",
        ],
    );
    assert_eq!(exit, 0, "{v}");
    std::fs::write(dir.path().join("hook.mode"), "grandchild").unwrap();
    let (exit, v) = bdc(&repo, dir.path(), &["cache", "tick"]);
    let pids = [
        pid_in(&dir.path().join("gc.pid")),
        pid_in(&dir.path().join("child.pid")),
    ];
    let alive = gone(&pids);
    cleanup(&alive);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(
        v["data"]["hook"]["error"]["details"]["reason"], "timeout",
        "{v}"
    );
    assert!(
        alive.is_empty(),
        "these outlived the hook's kill: {alive:?}"
    );
    std::fs::remove_file(dir.path().join("hook.mode")).unwrap();
    let (exit, v) = bdc(&repo, dir.path(), &["cache", "tick"]);
    assert_eq!(
        (exit, v["data"]["hook"]["status"].as_str()),
        (0, Some("ok")),
        "{v}"
    );
}

/// A Python row's script that started a grandchild is killed with it at
/// the row's timeout; `use-python` fails with `TICK_RUN_FAILED` (`timeout`)
/// and restores the mode (acceptance 4 and 7). Planted negative: an
/// ordinary script switches.
#[test]
fn a_python_rows_child_and_grandchild_are_killed_at_its_timeout() {
    let repo = TestRepo::new(&two_beads());
    let dir = tempfile::TempDir::new().unwrap();
    assert_eq!(
        bdc(
            &repo,
            dir.path(),
            &["cache", "schedule", "set", "0 0 1 1 *"]
        )
        .0,
        0
    );
    std::fs::write(dir.path().join("tick.mode"), "grandchild").unwrap();
    let use_python = [
        "cache",
        "sync",
        "use-python",
        "--python",
        FAKE_PYTHON,
        "--module",
        "consumer.sync",
        "--timeout-ms",
        "2000",
        "--env",
        "FAKE_PY_DIR",
    ];
    let (exit, v) = bdc(&repo, dir.path(), &use_python);
    let pids = [
        pid_in(&dir.path().join("gc.pid")),
        pid_in(&dir.path().join("child.pid")),
    ];
    let alive = gone(&pids);
    cleanup(&alive);
    assert_eq!(
        (exit, v["error"]["code"].as_str()),
        (3, Some("BD_CACHE.TICK_RUN_FAILED")),
        "{v}"
    );
    assert_eq!(v["error"]["details"]["reason"], "timeout", "{v}");
    assert_eq!(v["error"]["details"]["restored"], true);
    assert!(
        alive.is_empty(),
        "these outlived the script's kill: {alive:?}"
    );
    std::fs::remove_file(dir.path().join("tick.mode")).unwrap();
    assert_eq!(bdc(&repo, dir.path(), &use_python).0, 0);
}

/// Ctrl-Break to `cache cron run` (in a process group of its own) while a
/// hook's tree runs: it waits up to `--shutdown-timeout-ms`, then the job
/// object's children are gone, the lease is free (the next `tick` is not
/// `in_flight`) and it exits 130 (acceptance 9, REQ-BDC-0730). Planted
/// negative: the children ignore Ctrl-Break, so only the job's kill ends them.
#[cfg(windows)]
#[test]
fn ctrl_break_stops_cron_run_kills_the_job_and_exits_130() {
    use std::os::windows::process::CommandExt as _;

    /// `CREATE_NEW_PROCESS_GROUP`: the child leads a console process group.
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    /// `CTRL_BREAK_EVENT`.
    const CTRL_BREAK_EVENT: u32 = 1;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GenerateConsoleCtrlEvent(event: u32, group: u32) -> i32;
    }

    let repo = TestRepo::new(&two_beads());
    let dir = tempfile::TempDir::new().unwrap();
    let (exit, v) = bdc(
        &repo,
        dir.path(),
        &[
            "cache",
            "schedule",
            "set",
            "* * * * * *",
            "--budget-ms",
            "30000",
            "--hook-reserve-ms",
            "20000",
            "--post-sync",
            "consumer.hooks:after",
            "--python",
            FAKE_PYTHON,
            "--env",
            "FAKE_PY_DIR",
        ],
    );
    assert_eq!(exit, 0, "{v}");
    std::fs::write(dir.path().join("hook.mode"), "grandchild").unwrap();
    let root = repo.root();
    let child = support::hermetic(support::BDC)
        .args(["--repo", path_str(&root), "--bd", support::FAKE_BD])
        .args(["cache", "cron", "run", "--shutdown-timeout-ms", "1500"])
        .env("FAKE_BD_SCENARIO", repo.scenario_path())
        .env("FAKE_PY_DIR", dir.path())
        .creation_flags(CREATE_NEW_PROCESS_GROUP)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("bdc starts");
    let pids = [
        pid_in(&dir.path().join("gc.pid")),
        pid_in(&dir.path().join("child.pid")),
    ];
    let sent = Instant::now();
    // SAFETY: a plain Win32 call with integer arguments; the group is the
    // child's own (CREATE_NEW_PROCESS_GROUP), which shares this console.
    let ok = unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, child.id()) };
    assert_ne!(
        ok,
        0,
        "GenerateConsoleCtrlEvent: {}",
        std::io::Error::last_os_error()
    );
    let out = child.wait_with_output().unwrap();
    let alive = gone(&pids);
    cleanup(&alive);
    assert_eq!(
        out.status.code(),
        Some(130),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        sent.elapsed() >= Duration::from_millis(1500),
        "the wait was cut short"
    );
    assert!(
        alive.is_empty(),
        "the job's children outlived cron run: {alive:?}"
    );
    std::fs::remove_file(dir.path().join("hook.mode")).unwrap();
    let (exit, v) = bdc(&repo, dir.path(), &["cache", "tick"]);
    assert_eq!(exit, 0, "{v}");
    assert!(
        v["data"]["skipped"].is_null(),
        "the lease was released: {v}"
    );
}
