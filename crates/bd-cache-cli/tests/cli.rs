//! Acceptance 1 and 2 of sprint b-3a: `bdc --version` is one sc-command envelope, and
//! `bd-cache` and `bdc` give identical output for the same argv (REQ-BDC-0805).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use sc_beads::SystemClock;
use sc_command::ErrorKind;
use support::{BD_CACHE, BDC, FAKE_BD, TestRepo, parse_envelope, run_binary, two_beads};

#[test]
fn version_is_one_envelope_with_ok_true_and_a_name() {
    let ran = run_binary(BDC, &["--version"]);
    assert_eq!(ran.exit, 0, "stderr: {}", ran.stderr);
    assert_eq!(ran.stderr, "");
    let envelope = parse_envelope(&ran.stdout).expect("one envelope");
    assert!(envelope.is_ok());
    let data = envelope.data().expect("data on success");
    assert_eq!(data["name"], "bd-cache");
    assert_eq!(data["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(data["contract_version"], "1.0.0");
}

/// The planted negative of acceptance 1: `--version` as plain text is not an envelope.
#[test]
fn plain_text_version_fails_the_envelope_parse() {
    let plain = format!("bd-cache {}\n", env!("CARGO_PKG_VERSION"));
    let err = parse_envelope(&plain).expect_err("plain text is not an envelope");
    assert!(err.contains("not an envelope"), "{err}");
    // Two envelopes on stdout are not "one envelope" either.
    let one = run_binary(BDC, &["--version"]).stdout;
    assert!(parse_envelope(&format!("{one}{one}")).is_err());
    // Nor is an envelope without its newline.
    assert!(parse_envelope(one.trim_end()).is_err());
}

#[test]
fn usage_errors_are_envelopes_with_exit_2() {
    for argv in [
        &["frobnicate"][..],
        &["--bogus"],
        &["--versio"], // an abbreviation is refused (contract §2)
        &["--help"],   // help is a USAGE reply until b-3 (contract §8)
        &[],
    ] {
        let ran = run_binary(BDC, argv);
        assert_eq!(ran.exit, 2, "{argv:?}: {ran:?}");
        let envelope = parse_envelope(&ran.stdout).expect("a usage envelope");
        let error = envelope.error().expect("an error");
        assert_eq!(error.code.as_str(), "BD_CACHE.USAGE", "{argv:?}");
        assert_eq!(error.kind, ErrorKind::InvalidInput);
        assert!(
            error
                .details
                .as_ref()
                .is_some_and(|d| d["valid"].is_array())
        );
    }
}

#[test]
fn the_harness_invocation_shape_reaches_version() {
    // `bdc --repo <repo> --bd <fake-bd> --version`, as every harness test spells it.
    let repo = TestRepo::new(&two_beads());
    let ran = repo.bdc(&["--version"]);
    assert_eq!(ran.exit, 0, "{ran:?}");
    assert!(parse_envelope(&ran.stdout).expect("one envelope").is_ok());
}

/// Both binaries give the same exit code, stdout and stderr for `argv`.
fn identical_output(argv: &[&str]) -> Result<(), String> {
    let bdc = run_binary(BDC, argv);
    let bd_cache = run_binary(BD_CACHE, argv);
    if bdc == bd_cache {
        Ok(())
    } else {
        Err(format!(
            "{argv:?}: bdc gave {bdc:?}, bd-cache gave {bd_cache:?}"
        ))
    }
}

#[test]
fn bd_cache_and_bdc_give_identical_output_for_the_same_argv() {
    for argv in [
        &["--version"][..],
        &["--repo", "r", "--bd", "b", "--version"],
        &["frobnicate"],
        &["--versio"],
        &[],
    ] {
        identical_output(argv).expect("identical output");
    }
    // The identity is not vacuous: the same argv really does succeed.
    assert_eq!(run_binary(BD_CACHE, &["--version"]).exit, 0);
}

/// The planted negative of acceptance 2 at the level of the comparison: two
/// processes that differ are reported as different, so a `bdc` that dropped
/// `--version` could not pass the test above.
#[test]
fn the_identity_check_detects_a_divergent_entry_point() {
    let a = run_binary(BDC, &["--version"]);
    let b = run_binary(BDC, &["frobnicate"]);
    assert_ne!(a, b);
    // A different binary on the same argv is a difference too: fake-bd is not bdc.
    let fake = run_binary(FAKE_BD, &["--version"]);
    assert_ne!(a, fake);
}

#[test]
fn run_in_process_matches_the_real_process() {
    let argv = vec!["--version".to_owned()];
    let (exit, stdout) = bd_cache_cli::run(&argv, &BTreeMap::new(), None, Arc::new(SystemClock));
    let real = run_binary(BDC, &["--version"]);
    assert_eq!((exit, stdout), (real.exit, real.stdout));
}
