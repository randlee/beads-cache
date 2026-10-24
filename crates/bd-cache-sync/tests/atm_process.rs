//! `AtmSender` against a real child process (REQ-BDC-0704..0706).
//!
//! The test binary is its own fake `atm` (no libtest harness): run with
//! `--fake-atm=<mode> <out-prefix> send <to> ...` it behaves as the named
//! mode of `atm`, so the sender spawns and talks to a real process on every
//! platform without a shell script.
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

use bd_cache_model::EscalationSender;
use bd_cache_sync::AtmSender;

const FLAG: &str = "--fake-atm=";

/// The fake `atm`: `<exe> --fake-atm=<mode> <out-prefix> <atm args...>`.
fn fake_atm(mode: &str, out: &str, args: &[String]) -> i32 {
    match mode {
        "record" | "fail" => {
            let mut stdin = Vec::new();
            std::io::stdin().read_to_end(&mut stdin).unwrap();
            std::fs::write(format!("{out}.args"), args.join("\n")).unwrap();
            std::fs::write(format!("{out}.stdin"), stdin).unwrap();
            if mode == "fail" {
                eprintln!("error: mailbox full");
                return 3;
            }
            0
        }
        // Exits at once without reading its input.
        "deaf" => 0,
        // Never finishes.
        "hang" => {
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

fn delivers_the_body_on_stdin_with_the_cron_module_arguments() {
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
            "--as",
            "cron",
            "--stdin"
        ]
    );
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
    assert!(err.detail.contains("exit"), "{err}");
    assert!(err.detail.contains("mailbox full"), "{err}");
}

fn an_atm_that_hangs_is_killed_at_the_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("hung");
    let started = Instant::now();
    let err = sender("hang", &out)
        .with_timeout(Duration::from_millis(300))
        .send("l", "t", "hi")
        .unwrap_err();
    assert!(err.detail.contains("did not finish"), "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "returned after {:?}",
        started.elapsed()
    );
}

fn an_atm_that_exits_without_reading_still_counts_as_delivered_by_its_status() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("deaf");
    // Bigger than a pipe buffer, so the write meets a closed pipe.
    let body = "y".repeat(4 * 1024 * 1024);
    assert!(sender("deaf", &out).send("l", "t", &body).is_ok());
}

fn a_missing_binary_is_a_send_error() {
    let err = AtmSender::with_binary("definitely-not-a-binary-bdc")
        .send("l", "t", "hi")
        .unwrap_err();
    assert!(err.detail.contains("cannot start"), "{err}");
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
        assert!(
            err.detail.contains("not an address"),
            "{lead:?} {team:?}: {err}"
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(mode) = args.first().and_then(|a| a.strip_prefix(FLAG)) {
        let out = args.get(1).cloned().unwrap_or_default();
        let code = fake_atm(mode, &out, &args);
        std::io::stdout().flush().unwrap();
        std::process::exit(code);
    }
    let tests: [(&str, fn()); 7] = [
        (
            "delivers_the_body_on_stdin_with_the_cron_module_arguments",
            delivers_the_body_on_stdin_with_the_cron_module_arguments,
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
            "an_atm_that_hangs_is_killed_at_the_timeout",
            an_atm_that_hangs_is_killed_at_the_timeout,
        ),
        (
            "an_atm_that_exits_without_reading_still_counts_as_delivered_by_its_status",
            an_atm_that_exits_without_reading_still_counts_as_delivered_by_its_status,
        ),
        (
            "a_missing_binary_is_a_send_error",
            a_missing_binary_is_a_send_error,
        ),
        (
            "an_address_that_could_be_read_as_an_option_is_refused_before_any_process_starts",
            an_address_that_could_be_read_as_an_option_is_refused_before_any_process_starts,
        ),
    ];
    for (name, test) in tests {
        test();
        println!("test {name} ... ok");
    }
    println!("\ntest result: ok. {} passed", tests.len());
}
