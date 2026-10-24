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
use std::time::{Duration, Instant};

use bd_cache_model::{EscalationSender, SendErrorKind};
use bd_cache_sync::{AtmSender, StopFlag, TICK_BUDGET};

const FLAG: &str = "--fake-atm=";
/// How long a hanging fake waits before it proves it is still alive.
const ALIVE_AFTER: Duration = Duration::from_millis(1500);

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
        // Never finishes; proves it was not killed by writing `<out>.alive`.
        "hang" => {
            std::thread::sleep(ALIVE_AFTER);
            std::fs::write(format!("{out}.alive"), "alive").unwrap();
            std::thread::sleep(Duration::from_secs(60));
            0
        }
        // Starts a grandchild that hangs, then hangs itself.
        "tree" => {
            let exe = std::env::current_exe().unwrap();
            #[expect(
                clippy::zombie_processes,
                reason = "the grandchild is meant to outlive its parent unless the group is killed"
            )]
            let _grandchild = std::process::Command::new(exe)
                .args([&format!("{FLAG}hang"), &format!("{out}.grandchild")])
                .spawn()
                .unwrap();
            std::fs::write(format!("{out}.spawned"), "yes").unwrap();
            std::thread::sleep(Duration::from_secs(60));
            0
        }
        other => panic!("unknown fake atm mode {other}"),
    }
}

fn sender(mode: &str, out: &Path) -> AtmSender {
    AtmSender::with_binary(std::env::current_exe().unwrap())
        .with_leading_args([format!("{FLAG}{mode}"), out.display().to_string()])
}

/// True when the fake wrote its proof of life after the kill window.
fn alive_marker(prefix: &str) -> bool {
    Path::new(&format!("{prefix}.alive")).exists()
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

fn an_atm_that_hangs_is_killed_and_gone_at_the_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("hung");
    let started = Instant::now();
    let err = sender("hang", &out)
        .with_timeout(Duration::from_millis(300))
        .unwrap()
        .send("l", "t", "hi")
        .unwrap_err();
    assert_eq!(err.kind, SendErrorKind::TimedOut);
    assert!(err.detail.contains("did not finish"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(10));
    // Gone: a live child writes its marker 1.5 s after it started.
    std::thread::sleep(ALIVE_AFTER + Duration::from_secs(1));
    assert!(
        !alive_marker(&out.display().to_string()),
        "the child was not killed"
    );
}

fn the_whole_process_tree_is_killed_not_just_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("tree");
    let err = sender("tree", &out)
        .with_timeout(Duration::from_millis(800))
        .unwrap()
        .send("l", "t", "hi")
        .unwrap_err();
    assert_eq!(err.kind, SendErrorKind::TimedOut);
    // The grandchild existed when the kill came (else the check below proves nothing).
    assert!(Path::new(&format!("{}.spawned", out.display())).exists());
    std::thread::sleep(ALIVE_AFTER + Duration::from_secs(1));
    assert!(
        !alive_marker(&format!("{}.grandchild", out.display())),
        "the grandchild outlived the kill"
    );
}

fn the_time_a_tick_passes_bounds_the_send() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("within");
    let started = Instant::now();
    let err = sender("hang", &out)
        .send_within("l", "t", "hi", Duration::from_millis(250))
        .unwrap_err();
    assert_eq!(err.kind, SendErrorKind::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    let none = sender("hang", &out)
        .send_within("l", "t", "hi", Duration::ZERO)
        .unwrap_err();
    assert_eq!(none.kind, SendErrorKind::TimedOut);
}

fn the_stop_flag_kills_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("stopped");
    let stop = StopFlag::new();
    let flag = stop.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        flag.stop();
    });
    let started = Instant::now();
    let err = sender("tree", &out)
        .with_stop(stop)
        .send("l", "t", "hi")
        .unwrap_err();
    stopper.join().unwrap();
    assert_eq!(err.kind, SendErrorKind::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(5));
    std::thread::sleep(ALIVE_AFTER + Duration::from_secs(1));
    assert!(!alive_marker(&format!("{}.grandchild", out.display())));
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
    let tests: [(&str, fn()); 12] = [
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
            "the_whole_process_tree_is_killed_not_just_the_child",
            the_whole_process_tree_is_killed_not_just_the_child,
        ),
        (
            "the_time_a_tick_passes_bounds_the_send",
            the_time_a_tick_passes_bounds_the_send,
        ),
        (
            "the_stop_flag_kills_the_child",
            the_stop_flag_kills_the_child,
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
    for (name, test) in tests {
        test();
        println!("test {name} ... ok");
    }
    println!("\ntest result: ok. {} passed", tests.len());
}
