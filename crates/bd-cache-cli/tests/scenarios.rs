//! The named negative scenarios of contract §8 "Testing" (acceptance 5 to 11 of
//! sprint b-3), one test each, run as real `bdc` processes against `fake-bd`.
//!
//! | Scenario | Test |
//! |---|---|
//! | an outage during a write | [`an_outage_during_a_write`] |
//! | an escalation at flush (`not_closable`, a guard miss) | [`an_escalation_at_flush`] |
//! | a partial `refresh scopes` | [`a_partial_refresh_scopes`] |
//! | `fresh_within` running out | [`fresh_within_runs_out`] |
//! | a schema change (rebuild) | [`a_schema_change_rebuilds`] |
//! | `WRONG_DATABASE` | [`a_cache_file_of_another_project_is_wrong_database`] |
//! | a retry after a lost reply | [`a_retry_after_a_lost_reply`] |

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use std::time::{Duration, Instant};

use bd_cache_cli::scenario::{Faults, RefuseWrite};
use serde_json::{Value, json};
use support::{DATABASE, TestRepo, labelled, metadata, two_beads};

/// A repository whose scope `phase-x` has been synced once.
fn synced() -> TestRepo {
    let repo = TestRepo::new(&two_beads());
    ok(&repo, &["cache", "refresh", "scopes", "x"]);
    repo
}

/// Runs `args`, asserts exit 0 and `ok: true`, and returns `data`.
fn ok(repo: &TestRepo, args: &[&str]) -> Value {
    let (exit, v) = repo.json(args);
    assert_eq!(exit, 0, "{args:?}: {v}");
    assert_eq!(v["ok"], true, "{args:?}: {v}");
    v["data"].clone()
}

fn status_of(repo: &TestRepo, id: &str) -> Option<String> {
    repo.scenario()
        .rows
        .iter()
        .find(|b| b.id.as_str() == id)
        .and_then(|b| b.status.clone())
}

/// Acceptance 5: with bd unavailable, `issues update` exits 0 with
/// `x_cache.state: "pending"`; a flush in the outage applies nothing; after
/// recovery `cache flush` applies it and `journal get` shows `applied`.
#[test]
fn an_outage_during_a_write() {
    let repo = synced();
    repo.edit(|s| s.faults.unavailable = true);

    let data = ok(
        &repo,
        &[
            "issues",
            "update",
            "b-1",
            "--actor",
            "dev",
            "--idempotency-key",
            "out-1",
            "--status",
            "in_progress",
        ],
    );
    assert_eq!(data["x_cache"]["state"], "pending", "{data}");
    assert_eq!(data["x_cache"]["duplicate"], false);
    let seq = data["x_cache"]["seq"].as_i64().unwrap();
    assert_eq!(
        data["issue"]["status"], "in_progress",
        "the view has the write"
    );

    let flush = ok(&repo, &["cache", "flush"]);
    assert_eq!(flush["applied"], json!([]), "{flush}");
    assert!(
        !flush["outage"].is_null(),
        "the outage is reported: {flush}"
    );
    let entry = ok(&repo, &["cache", "journal", "get", &seq.to_string()]);
    assert_eq!(entry["entry"]["state"], "pending", "{entry}");
    assert_eq!(
        status_of(&repo, "b-1").as_deref(),
        Some("open"),
        "bd unchanged"
    );

    repo.edit(|s| s.faults.unavailable = false);
    let flush = ok(&repo, &["cache", "flush"]);
    assert_eq!(flush["applied"], json!([seq]), "{flush}");
    let entry = ok(&repo, &["cache", "journal", "get", &seq.to_string()]);
    assert_eq!(entry["entry"]["state"], "applied", "{entry}");
    assert!(!entry["entry"]["applied_at"].is_null());
    assert_eq!(
        status_of(&repo, "b-1").as_deref(),
        Some("in_progress"),
        "bd has it"
    );
}

/// Acceptance 6: a close bd refuses (`not_closable`; over the bd CLI a refused
/// close is exit 13) and a guard that no longer holds each escalate at flush;
/// `cache flush` lists both in `escalated[]`, and `journal get` shows
/// `last_error.code` with no note.
#[test]
fn an_escalation_at_flush() {
    let repo = synced();
    let guarded = ok(
        &repo,
        &[
            "issues",
            "update",
            "b-1",
            "--actor",
            "dev",
            "--idempotency-key",
            "guard-1",
            "--assignee",
            "dev",
            "--expected-status",
            "open",
        ],
    );
    let closing = ok(
        &repo,
        &[
            "issues",
            "close",
            "b-2",
            "--actor",
            "dev",
            "--idempotency-key",
            "close-1",
            "--reason",
            "done",
        ],
    );
    let (g, c) = (
        guarded["x_cache"]["seq"].as_i64().unwrap(),
        closing["x_cache"]["seq"].as_i64().unwrap(),
    );
    // bd moves on: b-1 is started by someone else, and bd refuses every write.
    repo.edit(|s| {
        let b1 = s.rows.iter_mut().find(|b| b.id.as_str() == "b-1").unwrap();
        b1.status = Some("in_progress".to_owned());
        s.faults.refuse_write = Some(RefuseWrite { exit_code: 13 });
    });

    let flush = ok(&repo, &["cache", "flush"]);
    let escalated = flush["escalated"].as_array().unwrap();
    let seqs: Vec<i64> = escalated
        .iter()
        .map(|e| e["seq"].as_i64().unwrap())
        .collect();
    assert_eq!(seqs, [g, c], "{flush}");
    for e in escalated {
        assert_eq!(e["reason"]["code"], "BD_CACHE.PRECONDITION", "{e}");
        assert_eq!(e["actor"], "dev");
        assert_eq!(e["scope"], "phase-x");
    }
    assert_eq!(escalated[1]["op"], "close");
    assert_eq!(flush["applied"], json!([]));

    for seq in [g, c] {
        let entry = ok(&repo, &["cache", "journal", "get", &seq.to_string()])["entry"].clone();
        assert_eq!(entry["state"], "escalated", "{entry}");
        assert_eq!(
            entry["last_error"]["code"], "BD_CACHE.PRECONDITION",
            "{entry}"
        );
        assert!(entry["note"].is_null(), "no note before a resolve: {entry}");
    }
    let listed = ok(&repo, &["cache", "journal", "list"]);
    assert_eq!(
        listed["entries"].as_array().map(Vec::len),
        Some(2),
        "{listed}"
    );
    assert_eq!(
        status_of(&repo, "b-2").as_deref(),
        Some("open"),
        "nothing written"
    );
}

/// Acceptance 7: one scope fails and one passes; exit 0, `data.failed: 1`,
/// `results[]` in request order.
#[test]
fn a_partial_refresh_scopes() {
    let mut scenario = two_beads();
    let mut y = labelled("y-1");
    y.labels = vec!["phase-y".to_owned()];
    scenario.rows.push(y);
    scenario.faults = Faults {
        fail_labels: vec!["phase-y".to_owned()],
        ..Faults::default()
    };
    let repo = TestRepo::new(&scenario);

    for (order, failing) in [(["x", "y"], 1), (["y", "x"], 0)] {
        let mut args = vec!["cache", "refresh", "scopes"];
        args.extend(order);
        let (exit, v) = repo.json(&args);
        assert_eq!(exit, 0, "a partial refresh is not an error exit: {v}");
        let data = &v["data"];
        assert_eq!(data["failed"], 1, "{data}");
        let scopes: Vec<&str> = data["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["scope"].as_str().unwrap())
            .collect();
        let want: Vec<String> = order.iter().map(|s| format!("phase-{s}")).collect();
        assert_eq!(scopes, want, "request order");
        let bad = &data["results"][failing];
        assert!(!bad["error"].is_null() && bad["report"].is_null(), "{data}");
        let good = &data["results"][1 - failing];
        assert!(
            good["error"].is_null() && good["report"].is_object(),
            "{data}"
        );
        if failing == 1 {
            // The first sync of phase-x stores both its rows; the second is incremental.
            assert_eq!(good["report"]["stored"], 2, "{data}");
        }
    }
}

/// Acceptance 8: bd is slow by 2 s; `ready --fresh-within-ms 200` returns
/// within 400 ms with `read_fallback: "timed_out"` and `basis.kind: "cached"`.
#[test]
fn fresh_within_runs_out() {
    let repo = synced();
    // Warm the binaries, so the first execution of a fresh file is not timed.
    ok(&repo, &["ready", "--scope", "x"]);
    repo.edit(|s| s.faults.slow_ms = 2_000);

    let started = Instant::now();
    let (exit, v) = repo.json(&["ready", "--scope", "x", "--fresh-within-ms", "200"]);
    let elapsed = started.elapsed();
    assert_eq!(exit, 0, "{v}");
    let x = &v["data"]["x_cache"];
    assert_eq!(x["read_fallback"], "timed_out", "{v}");
    assert_eq!(x["basis"]["kind"], "cached", "{v}");
    assert_eq!(v["data"]["items"].as_array().map(Vec::len), Some(2));
    // Killing a process tree on Windows (`taskkill /T`) can itself take a few
    // hundred milliseconds, so the bound is asserted where the kill is a signal.
    if cfg!(unix) {
        assert!(elapsed < Duration::from_millis(400), "{elapsed:?}");
    }
}

/// Acceptance 9: a changed bd schema fingerprint makes `cache health --probe`
/// rebuild: `rebuilt: true`, the rows are dropped and the journal is kept.
#[test]
fn a_schema_change_rebuilds() {
    let repo = synced();
    let write = ok(
        &repo,
        &[
            "issues",
            "update",
            "b-1",
            "--actor",
            "dev",
            "--idempotency-key",
            "s-1",
            "--status",
            "in_progress",
        ],
    );
    let seq = write["x_cache"]["seq"].as_i64().unwrap();
    // The control: the same probe on an unchanged schema does not rebuild.
    let same = ok(&repo, &["cache", "health", "--probe"]);
    assert_eq!(same["probe"]["rebuilt"], false, "{same}");
    assert_eq!(
        ok(&repo, &["issues", "get", "b-2", "--miss-timeout-ms", "0"])["id"],
        "b-2"
    );

    repo.edit(|s| s.schema = Some("{\"schema_version\": 99, \"changed\": true}".to_owned()));
    let health = ok(&repo, &["cache", "health", "--probe"]);
    assert_eq!(health["probe"]["rebuilt"], true, "{health}");
    assert_eq!(health["probe"]["model_current"], false, "{health}");

    let (exit, v) = repo.json(&["issues", "get", "b-2", "--miss-timeout-ms", "0"]);
    assert_eq!(exit, 3, "the row was dropped: {v}");
    assert_eq!(v["error"]["code"], "BD_CACHE.NOT_CACHED");
    let status = ok(&repo, &["cache", "status"]);
    assert_eq!(
        status["journal"]["pending"], 1,
        "the journal is kept: {status}"
    );
    let entry = ok(&repo, &["cache", "journal", "get", &seq.to_string()]);
    assert_eq!(entry["entry"]["state"], "pending");
}

/// Acceptance 10: a cache file of another project is refused with exit 3 and
/// `WRONG_DATABASE`.
#[test]
fn a_cache_file_of_another_project_is_wrong_database() {
    let repo = synced();
    let cache = repo
        .root()
        .join(".bd-cache")
        .join(format!("{DATABASE}.bd-cache.sqlite3"));
    let before = std::fs::read(&cache).unwrap();
    std::fs::write(
        repo.root().join(".beads").join("metadata.json"),
        metadata(DATABASE, "another-project"),
    )
    .unwrap();
    for args in [
        &["issues", "get", "b-1"][..],
        &["cache", "status"],
        &["context"],
    ] {
        let (exit, v) = repo.json(args);
        assert_eq!(exit, 3, "{args:?}: {v}");
        assert_eq!(
            v["error"]["code"], "BD_CACHE.WRONG_DATABASE",
            "{args:?}: {v}"
        );
        assert_eq!(v["error"]["kind"], "conflict");
    }
    assert_eq!(
        std::fs::read(&cache).unwrap(),
        before,
        "the file is left as it was"
    );
}

/// Acceptance 11: the same key sent twice; the second reply is
/// `duplicate: true` with the same `seq`, and `journal get --idempotency-key K`
/// returns that entry.
#[test]
fn a_retry_after_a_lost_reply() {
    let repo = synced();
    let args = |actor: &'static str| {
        vec![
            "issues",
            "update",
            "b-1",
            "--actor",
            actor,
            "--idempotency-key",
            "lost-1",
            "--status",
            "in_progress",
        ]
    };
    let first = ok(&repo, &args("dev"));
    assert_eq!(first["x_cache"]["duplicate"], false);
    let seq = first["x_cache"]["seq"].clone();
    // The reply was lost; the retry comes from another actor (actor is not compared).
    let second = ok(&repo, &args("dev-retry"));
    assert_eq!(second["x_cache"]["duplicate"], true, "{second}");
    assert_eq!(second["x_cache"]["seq"], seq, "{second}");
    assert_eq!(second["changed"], first["changed"]);
    let entry = ok(
        &repo,
        &["cache", "journal", "get", "--idempotency-key", "lost-1"],
    );
    assert_eq!(entry["entry"]["seq"], seq, "{entry}");
    assert_eq!(entry["entry"]["idempotency_key"], "lost-1");
    assert_eq!(entry["entry"]["actor"], "dev", "the original entry");
    let pending = ok(&repo, &["cache", "journal", "list", "--state", "pending"]);
    assert_eq!(
        pending["entries"].as_array().map(Vec::len),
        Some(1),
        "{pending}"
    );
}
