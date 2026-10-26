//! The outage scenario (ADR-BDC-0005, REQ-BDC-0001, REQ-BDC-0729; sprint b-9
//! deliverable 3, acceptance 5): with bd unavailable, `issues get` answers
//! from the cache and the outage is recorded for the scope. Run as real
//! `bdc` processes against `fake-bd`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use bd_cache_cli::scenario::gate_started;
use serde_json::Value;
use support::{TestRepo, two_beads};

/// Runs `args`, asserts exit 0 and `ok: true`, and returns `data`.
fn ok(repo: &TestRepo, args: &[&str]) -> Value {
    let (exit, v) = repo.json(args);
    assert_eq!(exit, 0, "{args:?}: {v}");
    assert_eq!(v["ok"], true, "{args:?}: {v}");
    v["data"].clone()
}

/// The scope `x` of `data` (`cache scopes`).
fn scope_x(data: &Value) -> Value {
    data["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["scope"] == "x")
        .cloned()
        .unwrap_or_else(|| panic!("no scope x: {data}"))
}

/// bd goes away after a sync: the built-in run records the outage for the
/// scope (its result `outage`, `details.outage: true`, `failing_since`), and
/// `issues get` still answers from the cache with that outage in `x_cache`
/// and starts no bd (a gate no call may pass is never reached). Planted
/// negative: before the outage `x_cache.outage` is null and the scope has no
/// error; after bd is back the next run clears both.
#[test]
fn with_bd_unavailable_issues_get_answers_from_the_cache_and_the_outage_is_recorded() {
    let repo = TestRepo::new(&two_beads());
    ok(&repo, &["cache", "refresh", "scopes", "x"]);
    let before = ok(&repo, &["issues", "get", "b-1"]);
    assert_eq!(before["x_cache"]["outage"], Value::Null, "{before}");
    assert_eq!(
        scope_x(&ok(&repo, &["cache", "scopes"]))["last_error"],
        Value::Null
    );

    repo.edit(|s| s.faults.unavailable = true);
    let run = ok(&repo, &["cache", "tick"]);
    let result = run["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["scope"] == "x")
        .cloned()
        .unwrap_or_else(|| panic!("no result for x: {run}"));
    assert_eq!(result["result"], "outage", "{run}");
    assert_eq!(result["error"]["outage"], true, "{run}");
    assert!(result["failing_since"].is_string(), "{run}");

    let scopes = ok(&repo, &["cache", "scopes"]);
    let x = scope_x(&scopes);
    assert_eq!(
        x["last_error"]["code"], "BD_CACHE.BD_UNAVAILABLE",
        "{scopes}"
    );
    assert!(
        x["last_ok_at"].is_string(),
        "the earlier sync is kept: {scopes}"
    );

    // The read: a gate no bd call may pass shows that `issues get` starts none.
    let gate = repo.gate_file();
    repo.edit(|s| s.faults.gate_file = Some(gate.clone()));
    let got = ok(&repo, &["issues", "get", "b-1"]);
    assert!(
        !gate_started(&gate).exists(),
        "issues get started bd during the outage: {got}"
    );
    assert_eq!(got["id"], "b-1", "{got}");
    assert_eq!(got["x_cache"]["scope"], "x", "{got}");
    assert_eq!(got["x_cache"]["rows"]["b-1"]["stale"], false, "{got}");
    let outage = &got["x_cache"]["outage"];
    assert!(outage["since"].is_string(), "{got}");
    assert_eq!(
        outage["last_error"]["code"], "BD_CACHE.BD_UNAVAILABLE",
        "{got}"
    );
    let health = ok(&repo, &["cache", "health"]);
    assert_eq!(health["state"], "down", "{health}");
    repo.edit(|s| s.faults.gate_file = None);
    repo.release_gate();

    // bd is back: the next run clears the outage and the scope's error.
    repo.edit(|s| s.faults.unavailable = false);
    let run = ok(&repo, &["cache", "tick"]);
    assert_eq!(run["scopes"][0]["result"], "ok", "{run}");
    let after = ok(&repo, &["issues", "get", "b-1"]);
    assert_eq!(after["x_cache"]["outage"], Value::Null, "{after}");
    assert_eq!(
        scope_x(&ok(&repo, &["cache", "scopes"]))["last_error"],
        Value::Null
    );
}
