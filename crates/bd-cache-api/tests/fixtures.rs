//! The shared fixtures (`tests/fixtures/*.json`), each run against
//! `FakeStore` and `FakeBackend` through `decode` and `execute`.
//!
//! One test per file, so a failure names its scenario; a last test checks
//! that every file has one.
#![allow(
    clippy::panic,
    reason = "test code: a failed fixture is a test failure"
)]

mod common;

use common::{fixtures_dir, run_fixture};

/// The fixture files with a test below.
const FIXTURES: &[&str] = &[
    "decode_refusals",
    "idempotency_key_required",
    "issues_list_filters",
    "reads",
    "writes",
];

fn check(name: &str) {
    let failures = run_fixture(&fixtures_dir().join(format!("{name}.json")));
    assert!(failures.is_empty(), "{name}.json:\n{}", failures.join("\n"));
}

/// REQ-BDC-0807: unknown members, unserved `/v0` names, empty and closing patches.
#[test]
fn decode_refusals() {
    check("decode_refusals");
}

/// REQ-BDC-0004: a write without an idempotency key is refused.
#[test]
fn idempotency_key_required() {
    check("idempotency_key_required");
}

/// Contract §5.3 `issues list`: default exclusions, scope selection, filters.
#[test]
fn issues_list_filters() {
    check("issues_list_filters");
}

/// Contract §5.3 reads: `issues get`, `ready`, `dependencies list`, `context`.
#[test]
fn reads() {
    check("reads");
}

/// REQ-BDC-0809, §5.1: writes, duplicates with the stored `changed`, guards.
#[test]
fn writes() {
    check("writes");
}

#[test]
fn every_fixture_file_has_a_test() {
    let mut files: Vec<String> = std::fs::read_dir(fixtures_dir())
        .unwrap_or_else(|e| panic!("{e}"))
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".json"))
                .map(str::to_owned)
        })
        .collect();
    files.sort();
    assert_eq!(files, FIXTURES, "add a test for each fixture file");
}
