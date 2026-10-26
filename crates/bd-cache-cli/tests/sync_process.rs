//! The sync verbs in real `bdc` processes (REQ-BDC-0720, 0722, 0726, 0729, 0730; Unix):
//! one sync at a time across processes, a holder killed with `kill -9` taken
//! over after expiry, `cron run`'s signals and exit codes, `cron run --once`,
//! and `scopes ingest` on stdin. The Python row runs a stub interpreter that
//! speaks sc-cron's hook protocol (a shell script).

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use bd_cache_cli::scenario::gate_started;
use serde_json::Value;
use support::{BDC, FAKE_BD, TestRepo, envelope_json, hermetic, path_str, two_beads, write_script};

const SCENARIO_ENV: &str = "FAKE_BD_SCENARIO";

fn code(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}

/// A `bdc` child of `repo`, its stdout and stderr piped.
fn spawn(repo: &TestRepo, args: &[&str]) -> Child {
    let root = repo.root();
    let mut full = vec!["--repo", path_str(&root), "--bd", FAKE_BD];
    full.extend_from_slice(args);
    hermetic(BDC)
        .args(&full)
        .env(SCENARIO_ENV, repo.scenario_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bdc starts")
}

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(30);
    while !cond() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn finish(child: Child) -> (i32, Value) {
    let out = child.wait_with_output().unwrap();
    envelope_json(&out.into())
}

/// A stub interpreter: `-c` exits 0; `-m` answers `cron.stop` with
/// `continue` and, on `cron.tick`, counts the call (`tick.count`), touches
/// `tick.started`, then acts on `tick.mode` (`exit:<n>`, `sleep:<s>`).
fn python_stub(dir: &Path) -> PathBuf {
    let python = dir.join("python");
    let body = r#"#!/bin/sh
d="@DIR@"
[ "$2" = -c ] && exit 0
input=$(cat)
case "$input" in *'"cron.stop"'*) echo '{"action": "continue"}'; exit 0 ;; esac
n=$(( $(cat "$d/tick.count" 2>/dev/null || echo 0) + 1 )); echo $n > "$d/tick.count"
: > "$d/tick.started"
mode=$(cat "$d/tick.mode" 2>/dev/null)
case "$mode" in
  exit:*) echo "script failed" >&2; exit "${mode#exit:}" ;;
  sleep:*) sleep "${mode#sleep:}" ;;
esac
exit 0
"#
    .replace("@DIR@", &dir.display().to_string());
    write_script(&python, &body);
    python
}

fn ticks(dir: &Path) -> u32 {
    std::fs::read_to_string(dir.join("tick.count")).map_or(0, |s| s.trim().parse().unwrap())
}

/// A `bdc cache tick` that holds the lease (its bd blocked at the gate) makes
/// a second process's tick reply `skipped: "in_flight"` (exit 0); once the
/// holder is killed with `kill -9`, its lease is taken over after it expires
/// (REQ-BDC-0722).
#[test]
fn a_second_process_is_in_flight_and_a_killed_holder_is_taken_over_after_expiry() {
    let repo = TestRepo::new(&two_beads());
    assert_eq!(
        repo.json(&[
            "cache",
            "schedule",
            "set",
            "0 0 1 1 *",
            "--budget-ms",
            "4000",
            "--hook-reserve-ms",
            "0"
        ])
        .0,
        0
    );
    let gate = repo.gate_file();
    repo.edit(|s| s.faults.gate_file = Some(gate.clone()));
    let mut holder = spawn(&repo, &["cache", "tick"]);
    wait_for("the holder's bd at its gate", || {
        gate_started(&gate).exists()
    });
    let (exit, v) = repo.json(&["cache", "tick"]);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["data"]["skipped"], "in_flight", "{v}");
    assert_eq!(v["data"]["holder"]["pid"], holder.id());
    let expires = v["data"]["holder"]["expires_at"]
        .as_str()
        .unwrap()
        .to_owned();
    holder.kill().unwrap();
    holder.wait().unwrap();
    repo.edit(|s| s.faults.gate_file = None);
    let expiry = sc_beads::Timestamp::parse(&expires).unwrap().unix();
    wait_for("the dead holder's lease to expire", || {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        i64::try_from(now.as_secs()).unwrap() > expiry
    });
    let (exit, v) = repo.json(&["cache", "tick"]);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["data"]["outcome"], "ok", "{v}");
    assert_eq!(
        v["data"]["took_over"]["pid"],
        holder.id(),
        "taken over: {v}"
    );
}

/// `cron run` stops on SIGTERM (143), SIGINT (130) and SIGHUP (129); a second
/// signal during the wait changes neither the wait nor the exit code (REQ-BDC-0730).
#[test]
fn cron_run_exits_128_plus_its_first_signal() {
    for (first, second, exit) in [
        ("TERM", None, 143),
        ("INT", None, 130),
        ("HUP", None, 129),
        ("TERM", Some("INT"), 143),
    ] {
        let repo = TestRepo::new(&two_beads());
        let mut child = spawn(
            &repo,
            &["-v", "cache", "cron", "run", "--shutdown-timeout-ms", "200"],
        );
        let stderr = child.stderr.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if line.contains("worker started") {
                    let _ = tx.send(());
                }
            }
        });
        rx.recv_timeout(Duration::from_secs(30))
            .expect("cron run started its worker");
        let pid = child.id().to_string();
        assert!(
            Command::new("kill")
                .args([&format!("-{first}"), &pid])
                .status()
                .unwrap()
                .success()
        );
        if let Some(second) = second {
            let _ = Command::new("kill")
                .args([&format!("-{second}"), &pid])
                .status();
        }
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(exit), "SIG{first} then {second:?}");
        let (_, v) = envelope_json(&out.into());
        assert_eq!(v["data"]["stopped"], true, "{v}");
    }
}

/// With the Python row enabled, two simultaneous `cron run --once` run its
/// script once (the second finds the lease); a failing row makes `cron run
/// --once` exit 3 with `TICK_RUN_FAILED` naming it; `sync use-python` during a
/// run in flight is `TICK_IN_FLIGHT` (REQ-BDC-0722, 0730).
#[test]
fn the_python_row_runs_once_across_processes_and_a_failing_row_exits_3() {
    let repo = TestRepo::new(&two_beads());
    let stub_dir = tempfile::TempDir::new().unwrap();
    let python = python_stub(stub_dir.path());
    let (exit, v) = repo.json(&[
        "cache",
        "sync",
        "use-python",
        "--python",
        path_str(&python),
        "--module",
        "consumer.sync",
        "--cron",
        "* * * * * *",
        "--timeout-ms",
        "20000",
    ]);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(ticks(stub_dir.path()), 1, "the immediate run");
    std::fs::write(stub_dir.path().join("tick.mode"), "sleep:2").unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    let started = stub_dir.path().join("tick.started");
    let _ = std::fs::remove_file(&started);
    let a = spawn(&repo, &["cache", "cron", "run", "--once"]);
    wait_for("the first run's script", || started.exists());
    let b = spawn(&repo, &["cache", "cron", "run", "--once"]);
    let (in_flight, v) = repo.json(&[
        "cache",
        "sync",
        "use-python",
        "--python",
        path_str(&python),
        "--module",
        "consumer.sync",
        "--cron",
        "* * * * * *",
    ]);
    assert_eq!((in_flight, code(&v)), (3, "BD_CACHE.TICK_IN_FLIGHT"), "{v}");
    let (ea, va) = finish(a);
    let (eb, vb) = finish(b);
    assert_eq!((ea, eb), (0, 0), "{va} {vb}");
    assert_eq!(ticks(stub_dir.path()), 2, "one more run, not two");

    std::fs::write(stub_dir.path().join("tick.mode"), "exit:1").unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    let (exit, v) = repo.json(&["cache", "cron", "run", "--once"]);
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.TICK_RUN_FAILED"), "{v}");
    assert!(
        v["error"]["details"]["row"]
            .as_str()
            .unwrap()
            .contains("bd-cache.sync.python"),
        "{v}"
    );
}

/// `scopes ingest` reads its snapshot on stdin; over 64 MiB is refused with
/// `INGEST_INVALID` (`too_large`) before the input is held whole (REQ-BDC-0726).
#[test]
fn scopes_ingest_reads_stdin_and_refuses_more_than_64_mib() {
    let repo = TestRepo::new(&two_beads());
    assert_eq!(repo.json(&["cache", "scopes", "add", "mine", "--all"]).0, 0);
    let row = r#"{"id": "i-1", "title": "t", "priority": 2, "status": "open", "issue_type": "task", "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"}"#;
    let mut child = spawn(
        &repo,
        &["cache", "scopes", "ingest", "mine", "--revision", "r1"],
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("[{row}]").as_bytes())
        .unwrap();
    let (exit, v) = finish(child);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(
        (v["data"]["stored"].clone(), v["data"]["revision"].clone()),
        (1.into(), "r1".into())
    );

    let mut child = spawn(&repo, &["cache", "scopes", "ingest", "mine"]);
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let chunk = vec![b' '; 1 << 20];
        for _ in 0..80 {
            if stdin.write_all(&chunk).is_err() {
                return;
            }
        }
    });
    let (exit, v) = finish(child);
    writer.join().unwrap();
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.INGEST_INVALID"), "{v}");
    assert_eq!(v["error"]["details"]["reason"], "too_large");
    // `cache scopes` and `scopes list` give the same reply.
    assert_eq!(
        repo.json(&["cache", "scopes"]).1["data"],
        repo.json(&["cache", "scopes", "list"]).1["data"]
    );
}
