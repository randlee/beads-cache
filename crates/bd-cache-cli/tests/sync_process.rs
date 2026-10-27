//! The sync verbs in real `bdc` processes (REQ-BDC-0720, 0722, 0724, 0726,
//! 0729, 0730; Unix): one sync at a time across processes, a holder killed
//! with `kill -9` taken over after expiry, `cron run`'s signals, grace period,
//! tree kills and exit codes, `cron run --once`, `use-python`'s signal
//! restore, the redaction of `--env` values in every reply, outcome and log
//! line, a schedule table of another version, and `scopes ingest` on stdin.
//! The Python row and the post-sync hook run stub interpreters that speak
//! their protocols (shell scripts).

#![cfg(unix)]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
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
/// `tick.started`, then acts on `tick.mode` (`exit:<n>`, `sleep:<s>`;
/// `grandchild`: starts a grandchild that ignores SIGTERM, writes its pid to
/// `gc.pid` and its own to `script.pid`, and waits; `leak`: writes
/// `$SECRET_TOKEN` all over more than 8 KiB of stderr and exits 1).
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
  grandchild) /bin/sh -c 'trap "" TERM; echo $$ > "$1.tmp"; mv "$1.tmp" "$1"; exec sleep 300' sh "$d/gc.pid" &
     while [ ! -f "$d/gc.pid" ]; do sleep 0.01; done
     echo $$ > "$d/script.pid.tmp"; mv "$d/script.pid.tmp" "$d/script.pid"; wait ;;
  leak) i=0; while [ $i -lt 400 ]; do printf 'leak %s %s ' "$SECRET_TOKEN" "$HOME" >&2; i=$((i+1)); done; exit 1 ;;
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

/// The value of the `--env` variable the redaction tests pass.
const SECRET: &str = "s3cr3t-token-value-0451";

/// The `HOME` the redaction tests give bdc (every child keeps `HOME`).
///
/// It is data, not a home directory (cross-platform guidelines, QA-008):
/// the value is a marker the redaction must hide when `--env HOME` names it,
/// nothing resolves a path from it, and this file is Unix-only
/// (`#![cfg(unix)]`), so no Windows `USERPROFILE` behaviour is assumed.
const HOME_VALUE: &str = "/home/redaction-test-0451";

/// A post-sync hook stub: `-c` exits 0; the hook reads its request and acts
/// on `hook.mode` (default: an ok answer; `grandchild`: a TERM-ignoring
/// grandchild, pids in `gc.pid` and `hook.pid`, then waits; `leak`:
/// `$SECRET_TOKEN` over more than 8 KiB of stderr and in its answer's
/// message, exit 1).
fn hook_stub(dir: &Path) -> PathBuf {
    let python = dir.join("hook-python");
    let body = r#"#!/bin/sh
d="@DIR@"
[ "$2" = -c ] && exit 0
cat > /dev/null
mode=$(cat "$d/hook.mode" 2>/dev/null)
case "$mode" in
  grandchild) /bin/sh -c 'trap "" TERM; echo $$ > "$1.tmp"; mv "$1.tmp" "$1"; exec sleep 300' sh "$d/gc.pid" &
     while [ ! -f "$d/gc.pid" ]; do sleep 0.01; done
     echo $$ > "$d/hook.pid.tmp"; mv "$d/hook.pid.tmp" "$d/hook.pid"; wait; exit 0 ;;
  leak) i=0; while [ $i -lt 400 ]; do printf 'leak %s ' "$SECRET_TOKEN" >&2; i=$((i+1)); done
     printf '{"protocol":"bd-cache.run.v1","ok":false,"error":{"type":"E","message":"token %s"}}\n' "$SECRET_TOKEN"; exit 1 ;;
esac
echo '{"protocol":"bd-cache.run.v1","ok":true,"result":null}'
"#
    .replace("@DIR@", &dir.display().to_string());
    write_script(&python, &body);
    python
}

/// The pid a stub wrote whole to `path`.
fn pid_in(path: &Path) -> u32 {
    let mut pid = None;
    wait_for(&format!("{}", path.display()), || {
        pid = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| t.trim().parse().ok());
        pid.is_some()
    });
    pid.unwrap()
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

fn signal(name: &str, pid: u32) {
    let _ = Command::new("kill")
        .args([&format!("-{name}"), &pid.to_string()])
        .status();
}

/// A `cron run` child with `extra` args, its stderr read on a thread; returns
/// once its worker started.
fn cron_run(repo: &TestRepo, extra: &[&str]) -> (Child, std::sync::mpsc::Receiver<String>) {
    let mut args = vec!["-v", "cache", "cron", "run"];
    args.extend_from_slice(extra);
    let mut child = spawn(repo, &args);
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let line = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .expect("cron run starts");
        if line.contains("worker started") {
            break;
        }
    }
    (child, rx)
}

/// Waits for `child` to exit, then kills what is left of `pids` (so a failed
/// test leaves nothing behind) and says which still ran.
fn reap(child: Child, pids: &[u32]) -> (std::process::Output, Vec<u32>) {
    let out = child.wait_with_output().unwrap();
    let alive: Vec<u32> = pids.iter().copied().filter(|p| runs(*p)).collect();
    for p in &alive {
        signal("KILL", *p);
    }
    (out, alive)
}

/// `cron run` with a hook whose child starts a grandchild that ignores
/// SIGTERM: a SIGTERM stops new runs and waits up to `--shutdown-timeout-ms`,
/// then kills both processes of the tree, releases the lease (the next
/// `tick` is not `in_flight`) and exits 143 (acceptance 9, B8Q1-004).
#[test]
fn cron_run_waits_then_kills_a_hooks_tree_and_releases_the_lease() {
    let repo = TestRepo::new(&two_beads());
    let stubs = tempfile::TempDir::new().unwrap();
    let hook = hook_stub(stubs.path());
    let (exit, v) = repo.json(&[
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
        path_str(&hook),
    ]);
    assert_eq!(exit, 0, "{v}");
    std::fs::write(stubs.path().join("hook.mode"), "grandchild").unwrap();
    let (child, _log) = cron_run(&repo, &["--shutdown-timeout-ms", "1500"]);
    let gc = pid_in(&stubs.path().join("gc.pid"));
    let hook_pid = pid_in(&stubs.path().join("hook.pid"));
    let signalled = Instant::now();
    signal("TERM", child.id());
    let (out, alive) = reap(child, &[gc, hook_pid]);
    let waited = signalled.elapsed();
    assert_eq!(
        out.status.code(),
        Some(143),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(alive.is_empty(), "these survived cron run: {alive:?}");
    assert!(
        waited >= Duration::from_millis(1500),
        "the wait was cut short: {waited:?}"
    );
    std::fs::write(stubs.path().join("hook.mode"), "ok").unwrap();
    let (exit, v) = repo.json(&["cache", "tick"]);
    assert_eq!(exit, 0, "{v}");
    assert!(
        v["data"]["skipped"].is_null(),
        "the lease was released: {v}"
    );
}

/// A second signal during the shutdown wait ends it at once (REQ-BDC-0730
/// as amended at the phase-end review, PE1-RSH-007): the hook's tree is
/// killed long before the 60 s `--shutdown-timeout-ms`, the exit code stays
/// the first signal's (143) and the lease is released. Planted negative: a
/// second signal that is ignored waits the full 60 s and fails the bound.
#[test]
fn a_second_signal_ends_cron_runs_shutdown_wait() {
    let repo = TestRepo::new(&two_beads());
    let stubs = tempfile::TempDir::new().unwrap();
    let hook = hook_stub(stubs.path());
    let (exit, v) = repo.json(&[
        "cache",
        "schedule",
        "set",
        "* * * * * *",
        "--budget-ms",
        "120000",
        "--hook-reserve-ms",
        "60000",
        "--post-sync",
        "consumer.hooks:after",
        "--python",
        path_str(&hook),
    ]);
    assert_eq!(exit, 0, "{v}");
    std::fs::write(stubs.path().join("hook.mode"), "grandchild").unwrap();
    let (child, _log) = cron_run(&repo, &["--shutdown-timeout-ms", "60000"]);
    let gc = pid_in(&stubs.path().join("gc.pid"));
    let hook_pid = pid_in(&stubs.path().join("hook.pid"));
    let signalled = Instant::now();
    signal("TERM", child.id());
    std::thread::sleep(Duration::from_millis(200));
    signal("INT", child.id());
    let (out, alive) = reap(child, &[gc, hook_pid]);
    let waited = signalled.elapsed();
    assert_eq!(
        out.status.code(),
        Some(143),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(alive.is_empty(), "these survived cron run: {alive:?}");
    assert!(
        waited < Duration::from_secs(30),
        "the second signal did not end the wait: {waited:?}"
    );
    std::fs::write(stubs.path().join("hook.mode"), "ok").unwrap();
    let (exit, v) = repo.json(&["cache", "tick"]);
    assert_eq!(exit, 0, "{v}");
    assert!(
        v["data"]["skipped"].is_null(),
        "the lease was released: {v}"
    );
}

/// The same with a Python row whose script starts a TERM-ignoring
/// grandchild: both are killed at the shutdown timeout, the lease is free,
/// exit 143 (acceptance 9, B8Q1-004). Planted negative: the script killed
/// only at its own timeout would outlive the 1.5 s wait (its timeout is 60 s).
#[test]
fn cron_run_waits_then_kills_a_python_rows_tree_and_releases_the_lease() {
    let repo = TestRepo::new(&two_beads());
    let stubs = tempfile::TempDir::new().unwrap();
    let python = python_stub(stubs.path());
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
        "60000",
    ]);
    assert_eq!(exit, 0, "{v}");
    std::fs::write(stubs.path().join("tick.mode"), "grandchild").unwrap();
    let (child, _log) = cron_run(&repo, &["--shutdown-timeout-ms", "1500"]);
    let gc = pid_in(&stubs.path().join("gc.pid"));
    let script = pid_in(&stubs.path().join("script.pid"));
    let signalled = Instant::now();
    signal("TERM", child.id());
    let (out, alive) = reap(child, &[gc, script]);
    assert_eq!(
        out.status.code(),
        Some(143),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(alive.is_empty(), "these survived cron run: {alive:?}");
    assert!(signalled.elapsed() >= Duration::from_millis(1500));
    let (_, show) = repo.json(&["cache", "schedule", "show"]);
    assert!(
        show["data"]["lease"].is_null(),
        "the lease was released: {show}"
    );
    assert_eq!(
        repo.json(&["cache", "sync", "use-builtin"]).0,
        3,
        "no built-in row: nothing to switch to"
    );
}

/// A signal to `cron run` while its run's bd call is in flight lets that
/// call finish in the grace period: the run records no failure, and the
/// scope's state shows no failure (Rand's ruling 5, B8Q1-026). Planted
/// negative: `cache tick` keeps the immediate behaviour, its bd call killed
/// at once.
#[test]
fn a_signal_during_a_bd_call_of_cron_run_lets_the_call_finish() {
    let repo = TestRepo::new(&two_beads());
    assert_eq!(repo.json(&["cache", "scopes", "add", "mine", "--all"]).0, 0);
    assert_eq!(repo.json(&["cache", "schedule", "set", "* * * * * *"]).0, 0);
    let gate = repo.gate_file();
    repo.edit(|s| s.faults.gate_file = Some(gate.clone()));
    let (child, _log) = cron_run(&repo, &["--shutdown-timeout-ms", "20000"]);
    wait_for("the run's bd call at its gate", || {
        gate_started(&gate).exists()
    });
    signal("TERM", child.id());
    std::thread::sleep(Duration::from_millis(300));
    repo.release_gate();
    let (out, _) = reap(child, &[]);
    assert_eq!(out.status.code(), Some(143));
    let (_, v) = envelope_json(&out.into());
    assert_eq!(
        v["data"]["drained"], true,
        "the run finished in the grace period: {v}"
    );
    repo.edit(|s| s.faults.gate_file = None);
    let (_, show) = repo.json(&["cache", "schedule", "show"]);
    let builtin = &show["data"]["rows"]["builtin"];
    assert_eq!(builtin["outcome"], "ok", "{show}");
    assert_eq!(builtin["consecutive_failures"], 0);
    for scope in show["data"]["scopes"].as_array().unwrap() {
        assert!(
            scope["failing_since"].is_null(),
            "no failure recorded: {show}"
        );
    }

    std::fs::remove_file(&gate).unwrap();
    let _ = std::fs::remove_file(gate_started(&gate));
    let _ = std::fs::remove_file(bd_cache_cli::scenario::gate_passed(&gate));
    repo.edit(|s| s.faults.gate_file = Some(gate.clone()));
    let tick = spawn(&repo, &["cache", "tick"]);
    wait_for("the tick's bd call at its gate", || {
        gate_started(&gate).exists()
    });
    signal("TERM", tick.id());
    let (out, _) = reap(tick, &[]);
    assert_eq!(out.status.code(), Some(143));
    assert!(
        !support::gate_passed_exists(&gate),
        "tick's bd call was killed, not waited for"
    );
}

/// Over 8 KiB of a script's stderr holding an `--env` value, in the first
/// 512 bytes and beyond: the `TICK_RUN_FAILED` reply is cut to 8 KiB and shows
/// `<redacted:SECRET_TOKEN>` in its stderr and message, as do `schedule
/// show`'s outcome, the `-v` log, and a hook's `error.message` (acceptance 4,
/// B8Q1-002). Planted negatives: without `--env` the value shows (no
/// redaction), and the stderr is never longer than 8 KiB (the cap).
#[test]
fn every_text_a_child_wrote_is_redacted_and_capped() {
    let repo = TestRepo::new(&two_beads());
    let stubs = tempfile::TempDir::new().unwrap();
    let python = python_stub(stubs.path());
    let use_python = |env: bool| {
        let mut args = vec![
            "cache",
            "sync",
            "use-python",
            "--python",
            path_str(&python),
            "--module",
            "consumer.sync",
            "--cron",
            "* * * * * *",
        ];
        if env {
            args.extend(["--env", "SECRET_TOKEN", "--env", "HOME"]);
        }
        let out = hermetic(BDC)
            .args(["--repo", path_str(&repo.root()), "--bd", FAKE_BD])
            .args(&args)
            .env(SCENARIO_ENV, repo.scenario_path())
            .env("SECRET_TOKEN", SECRET)
            .env("HOME", HOME_VALUE)
            .output()
            .unwrap();
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            envelope_json(&out.into()),
        )
    };
    std::fs::write(stubs.path().join("tick.mode"), "leak").unwrap();
    let (raw, (exit, v)) = use_python(true);
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.TICK_RUN_FAILED"), "{v}");
    let stderr = v["error"]["details"]["stderr"].as_str().unwrap();
    assert!(stderr.contains("<redacted:SECRET_TOKEN>"));
    assert_eq!(stderr.len(), 8 * 1024, "cut to 8 KiB");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("<redacted:SECRET_TOKEN>"),
        "{v}"
    );
    assert!(!raw.contains(SECRET), "the reply shows the value");
    assert!(
        !raw.contains(HOME_VALUE),
        "an --env name in the base set is redacted too"
    );
    // Planted negative: HOME reaches every child; not named with --env, its
    // value is not redacted and shows.
    let (raw, (_, v)) = use_python(false);
    assert!(raw.contains(HOME_VALUE), "no --env, no redaction: {v}");

    // A worker run's recorded outcome, shown by schedule show, and the log.
    std::fs::write(stubs.path().join("tick.mode"), "ok").unwrap();
    assert_eq!(use_python(true).1.0, 0);
    std::fs::write(stubs.path().join("tick.mode"), "leak").unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    let out = hermetic(BDC)
        .args([
            "--repo",
            path_str(&repo.root()),
            "--bd",
            FAKE_BD,
            "-v",
            "cache",
            "cron",
            "run",
            "--once",
        ])
        .env(SCENARIO_ENV, repo.scenario_path())
        .env("SECRET_TOKEN", SECRET)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(!log.contains(SECRET), "the log shows the value");
    assert!(!String::from_utf8_lossy(&out.stdout).contains(SECRET));
    let (_, show) = repo.json(&["cache", "schedule", "show"]);
    let outcome = show["data"]["rows"]["python"]["outcome"].as_str().unwrap();
    assert!(
        outcome.contains("<redacted:SECRET_TOKEN>") && !outcome.contains(SECRET),
        "{outcome}"
    );
}

/// A hook's `error.message` in a tick's reply shows `<redacted:SECRET_TOKEN>`
/// and never the value (acceptance 4, B8Q1-002); the stderr kept is capped.
#[test]
fn a_hooks_env_values_are_redacted_from_a_ticks_reply() {
    let stubs = tempfile::TempDir::new().unwrap();
    let hook = hook_stub(stubs.path());
    std::fs::write(stubs.path().join("hook.mode"), "leak").unwrap();
    let hooked = TestRepo::new(&two_beads());
    assert_eq!(
        hooked
            .json(&[
                "cache",
                "schedule",
                "set",
                "0 0 1 1 *",
                "--post-sync",
                "m:f",
                "--python",
                path_str(&hook),
                "--env",
                "SECRET_TOKEN"
            ])
            .0,
        0
    );
    let out = hermetic(BDC)
        .args([
            "--repo",
            path_str(&hooked.root()),
            "--bd",
            FAKE_BD,
            "cache",
            "tick",
        ])
        .env(SCENARIO_ENV, hooked.scenario_path())
        .env("SECRET_TOKEN", SECRET)
        .output()
        .unwrap();
    let raw = String::from_utf8_lossy(&out.stdout).into_owned();
    let (exit, v) = envelope_json(&out.into());
    assert_eq!(exit, 0, "{v}");
    let error = &v["data"]["hook"]["error"];
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("<redacted:SECRET_TOKEN>"),
        "{error}"
    );
    assert!(!raw.contains(SECRET));
}

/// A SIGTERM to `sync use-python` during its immediate run kills the
/// script's tree, restores the mode before the command, and exits 143
/// (REQ-BDC-0724, B8Q1-014). Planted negative: the same run uninterrupted
/// keeps the Python row.
#[test]
fn a_sigterm_to_use_python_mid_run_restores_and_exits_143() {
    let repo = TestRepo::new(&two_beads());
    assert_eq!(
        repo.json(&["cache", "schedule", "set", "*/30 * * * * *"]).0,
        0
    );
    let stubs = tempfile::TempDir::new().unwrap();
    let python = python_stub(stubs.path());
    std::fs::write(stubs.path().join("tick.mode"), "grandchild").unwrap();
    let args = [
        "cache",
        "sync",
        "use-python",
        "--python",
        path_str(&python),
        "--module",
        "consumer.sync",
    ];
    let child = spawn(&repo, &args);
    let gc = pid_in(&stubs.path().join("gc.pid"));
    let script = pid_in(&stubs.path().join("script.pid"));
    signal("TERM", child.id());
    let (out, alive) = reap(child, &[gc, script]);
    assert_eq!(out.status.code(), Some(143));
    assert!(alive.is_empty(), "{alive:?}");
    let (_, v) = envelope_json(&out.into());
    assert_eq!(v["error"]["details"]["reason"], "signal", "{v}");
    assert_eq!(v["error"]["details"]["restored"], true);
    let (_, show) = repo.json(&["cache", "schedule", "show"]);
    assert_eq!(show["data"]["mode"], "builtin", "{show}");
    std::fs::write(stubs.path().join("tick.mode"), "ok").unwrap();
    assert_eq!(repo.json(&args).0, 0);
    assert_eq!(
        repo.json(&["cache", "schedule", "show"]).1["data"]["mode"],
        "python"
    );
}

/// A cache file whose schedule table is of another version makes `schedule
/// set`, `cron run --once` and `tick` fail with `STORE_FAILED` before any
/// work (ADR-BDC-0014, B8Q1-014). Planted negative: the version restored, they serve.
#[test]
fn a_schedule_table_of_another_version_is_store_failed_at_command_level() {
    let repo = TestRepo::new(&two_beads());
    assert_eq!(
        repo.json(&["cache", "schedule", "set", "*/30 * * * * *"]).0,
        0
    );
    let path = repo
        .root()
        .join(".bd-cache")
        .join(format!("{}.bd-cache.sqlite3", support::DATABASE));
    assert!(path.exists(), "{}", path.display());
    let set_version = |v: &str| {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE meta SET value = ?1 WHERE key = 'sc_cron_schedule_version'",
            [v],
        )
        .unwrap();
    };
    set_version("99");
    for args in [
        &["cache", "schedule", "set", "0 * * * * *"][..],
        &["cache", "cron", "run", "--once"],
        &["cache", "tick"],
    ] {
        let (exit, v) = repo.json(args);
        assert_eq!(
            (exit, code(&v)),
            (3, "BD_CACHE.STORE_FAILED"),
            "{args:?}: {v}"
        );
        assert!(
            v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("sc_cron_schedule"),
            "{v}"
        );
    }
    set_version("2");
    assert_eq!(repo.json(&["cache", "schedule", "set", "0 * * * * *"]).0, 0);
}

/// 64 MiB of minimal valid rows on stdin is refused as `too_many_rows` at
/// the row past 50 000, bdc having read only that far (it exits while the
/// writer still has most of the input to send); a snapshot object naming
/// another scope is `BAD_ARGS` (REQ-BDC-0726, B8Q1-013, B8Q1-031). Planted
/// negative: the argv scope's rows from a valid object are stored.
#[test]
fn scopes_ingest_streams_stdin_and_takes_only_rows_and_blocked() {
    let repo = TestRepo::new(&two_beads());
    assert_eq!(repo.json(&["cache", "scopes", "add", "mine", "--all"]).0, 0);
    assert_eq!(
        repo.json(&["cache", "scopes", "add", "other", "--all"]).0,
        0
    );
    let row = r#"{"id":"i-1","title":"t","priority":2,"status":"open","created_at":"2026-10-23T09:00:00Z","updated_at":"2026-10-23T09:00:00Z"}"#;
    let mut child = spawn(&repo, &["cache", "scopes", "ingest", "mine"]);
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let mut sent = 0_usize;
        let mut chunk = String::with_capacity(1 << 20);
        let _ = stdin.write_all(b"[");
        while sent < 64 * 1024 * 1024 {
            chunk.clear();
            while chunk.len() < (1 << 20) - row.len() {
                chunk.push_str(row);
                chunk.push(',');
            }
            if stdin.write_all(chunk.as_bytes()).is_err() {
                return sent;
            }
            sent += chunk.len();
        }
        sent
    });
    let (exit, v) = finish(child);
    let sent = writer.join().unwrap();
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.INGEST_INVALID"), "{v}");
    assert_eq!(v["error"]["details"]["reason"], "too_many_rows");
    assert!(sent < 64 * 1024 * 1024, "bdc read all {sent} bytes");

    let ingest = |body: &str| {
        let mut child = spawn(
            &repo,
            &["cache", "scopes", "ingest", "mine", "--allow-empty"],
        );
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();
        finish(child)
    };
    let (exit, v) = ingest(r#"{"scope": "other", "rows": []}"#);
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.BAD_ARGS"), "{v}");
    let (exit, v) = ingest(&format!(r#"{{"rows": [{row}], "blocked": []}}"#));
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["data"]["scope"], "mine");
}
