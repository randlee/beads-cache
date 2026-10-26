//! The named negative scenarios of contract §8 "Testing" (acceptance 5 to 11 of
//! sprint b-3), one test each, run as real `bdc` processes against `fake-bd`.
//!
//! | Scenario | Test |
//! |---|---|
//! | an outage during a write | [`an_outage_during_a_write`] |
//! | an escalation at flush (a guard miss; a refused close, `PRECONDITION` over the bd CLI, S69) | [`a_guard_miss_escalates_at_flush`], [`bd_refuses_a_guard_that_misses_at_flush`], [`a_refused_close_escalates_at_flush`] |
//! | a partial `refresh scopes` | [`a_partial_refresh_scopes`] |
//! | a hung bd under `refresh scopes` (QA RCR-001) | [`a_hung_bd_fails_refresh_scopes`] |
//! | `refresh scopes --timeout-ms` against a gated bd (I10) | [`refresh_scopes_timeout_ms_bounds_the_sync`] |
//! | `health --probe` with bd unreachable (QA ARCH-001) | [`a_probe_that_cannot_reach_bd_is_down`] |
//! | health with both sync rows disabled (REQ-BDC-0732) | [`health_reports_the_sync_mode_and_no_sync_row_enabled`] |
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

use bd_cache_cli::scenario::{Faults, RefuseWrite, gate_passed};
use serde_json::{Value, json};
use support::{DATABASE, TestRepo, envelope_json, labelled, metadata, path_str, two_beads};

/// A repository whose scope `x` has been synced once.
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

/// The entry `seq` escalated with `code` and no note, as `cache journal get`
/// reads it.
fn escalated_with(repo: &TestRepo, seq: i64, code: &str) {
    let entry = ok(repo, &["cache", "journal", "get", &seq.to_string()])["entry"].clone();
    assert_eq!(entry["state"], "escalated", "{entry}");
    assert_eq!(entry["last_error"]["code"], code, "{entry}");
    assert!(entry["note"].is_null(), "no note before a resolve: {entry}");
}

/// Acceptance 6, the guard miss (QA RCR-001): an update guarded on
/// `--expected-status open` is flushed after bd moved the bead on. No write
/// fault is injected: the drain's check on its fresh read refuses it before
/// bd is called (bd's own `--if-status` is tested by
/// [`bd_refuses_a_guard_that_misses_at_flush`]). `cache flush` lists it in
/// `escalated[]` with `PRECONDITION`, and `journal get` shows
/// `last_error.code` with no note.
#[test]
fn a_guard_miss_escalates_at_flush() {
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
    let g = guarded["x_cache"]["seq"].as_i64().unwrap();
    // bd moves on: someone else starts b-1.
    repo.edit(|s| {
        let b1 = s.rows.iter_mut().find(|b| b.id.as_str() == "b-1").unwrap();
        b1.status = Some("in_progress".to_owned());
    });

    let flush = ok(&repo, &["cache", "flush"]);
    let escalated = flush["escalated"].as_array().unwrap();
    assert_eq!(escalated.len(), 1, "{flush}");
    let e = &escalated[0];
    assert_eq!(e["seq"], g, "{flush}");
    assert_eq!(e["op"], "update");
    assert_eq!(e["reason"]["code"], "BD_CACHE.PRECONDITION", "{e}");
    // The drain's own check refused it, not bd's --if-status (QA RCR-003).
    assert!(
        e["reason"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("at drain time"),
        "the drain's guard check, not bd, refused it: {e}"
    );
    assert_eq!(e["actor"], "dev");
    assert_eq!(e["scope"], "x");
    assert_eq!(flush["applied"], json!([]));
    escalated_with(&repo, g, "BD_CACHE.PRECONDITION");
    let b1 = repo.scenario();
    let b1 = b1.rows.iter().find(|b| b.id.as_str() == "b-1").unwrap();
    assert_ne!(b1.assignee.as_deref(), Some("dev"), "nothing written");
}

/// Acceptance 6, the guard checked by bd itself (QA RCR-001): the cache's
/// fresh read at flush still shows b-1 open (`faults.read_status`), so the
/// drain's own check passes, but bd's true row is in progress. Only bd's
/// `--if-status open` refuses the write (exit 13), so the escalation proves
/// the guard is passed to bd at flush. No write fault is injected.
#[test]
fn bd_refuses_a_guard_that_misses_at_flush() {
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
            "guard-bd-1",
            "--assignee",
            "dev",
            "--expected-status",
            "open",
        ],
    );
    let g = guarded["x_cache"]["seq"].as_i64().unwrap();
    repo.edit(|s| {
        let b1 = s.rows.iter_mut().find(|b| b.id.as_str() == "b-1").unwrap();
        b1.status = Some("in_progress".to_owned());
        s.faults
            .read_status
            .insert("b-1".to_owned(), "open".to_owned());
    });

    let flush = ok(&repo, &["cache", "flush"]);
    let escalated = flush["escalated"].as_array().unwrap();
    assert_eq!(escalated.len(), 1, "the guard reached bd: {flush}");
    let e = &escalated[0];
    assert_eq!(
        (e["seq"].as_i64(), &e["op"]),
        (Some(g), &json!("update")),
        "{flush}"
    );
    assert_eq!(e["reason"]["code"], "BD_CACHE.PRECONDITION", "{e}");
    let message = e["reason"]["message"].as_str().unwrap_or_default();
    assert!(
        !message.contains("at drain time"),
        "bd refused it, not the drain's own check: {e}"
    );
    assert_eq!(flush["applied"], json!([]));
    escalated_with(&repo, g, "BD_CACHE.PRECONDITION");
    let b1 = repo.scenario();
    let b1 = b1.rows.iter().find(|b| b.id.as_str() == "b-1").unwrap();
    assert_ne!(b1.assignee.as_deref(), Some("dev"), "nothing written");
}

/// Acceptance 6, the refused close: bd refuses the close (`faults.refuse_close`,
/// bd exit 13), while an unguarded update in the same flush applies. Over
/// the bd CLI exit 13 is `precondition_failed`, so the close escalates as
/// `BD_CACHE.PRECONDITION`; only an HTTP backend can report `not_closable`
/// itself (lead ruling S69, contract §5.4).
#[test]
fn a_refused_close_escalates_at_flush() {
    let repo = synced();
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
    let updating = ok(
        &repo,
        &[
            "issues",
            "update",
            "b-1",
            "--actor",
            "dev",
            "--idempotency-key",
            "update-1",
            "--assignee",
            "dev",
        ],
    );
    let (c, u) = (
        closing["x_cache"]["seq"].as_i64().unwrap(),
        updating["x_cache"]["seq"].as_i64().unwrap(),
    );
    repo.edit(|s| s.faults.refuse_close = Some(RefuseWrite { exit_code: 13 }));

    let flush = ok(&repo, &["cache", "flush"]);
    let escalated = flush["escalated"].as_array().unwrap();
    assert_eq!(escalated.len(), 1, "{flush}");
    let e = &escalated[0];
    assert_eq!(
        (e["seq"].as_i64(), &e["op"]),
        (Some(c), &json!("close")),
        "{flush}"
    );
    assert_eq!(e["reason"]["code"], "BD_CACHE.PRECONDITION", "{e}");
    assert_eq!(e["actor"], "dev");
    assert_eq!(e["scope"], "x");
    assert_eq!(
        flush["applied"],
        json!([u]),
        "only the close is refused: {flush}"
    );
    escalated_with(&repo, c, "BD_CACHE.PRECONDITION");
    let listed = ok(&repo, &["cache", "journal", "list"]);
    assert_eq!(
        listed["entries"].as_array().map(Vec::len),
        Some(1),
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
    // `y` is not configured: a refresh registers it on demand as the label
    // scope with that one label (REQ-BDC-0909 as amended by ADR-BDC-0013).
    let mut y = labelled("y-1");
    y.labels = vec!["y".to_owned()];
    scenario.rows.push(y);
    scenario.faults = Faults {
        fail_labels: vec!["y".to_owned()],
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
        let want: Vec<String> = order.iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(scopes, want, "request order");
        let bad = &data["results"][failing];
        assert!(!bad["error"].is_null() && bad["report"].is_null(), "{data}");
        let good = &data["results"][1 - failing];
        assert!(
            good["error"].is_null() && good["report"].is_object(),
            "{data}"
        );
        if failing == 1 {
            // The first sync of scope x stores both its rows; the second is incremental.
            assert_eq!(good["report"]["stored"], 2, "{data}");
        }
    }
}

/// Acceptance 8: bd does not answer; `ready --fresh-within-ms 200` returns
/// with `read_fallback: "timed_out"` and `basis.kind: "cached"`.
///
/// QA FTQ-001: no wall-clock bound on the whole process. bd blocks at a gate
/// the test opens only after `bdc` returned, so `bdc` returning at all, with
/// bd's `passed` marker absent, shows it gave up on bd within its budget
/// rather than waited for bd. (`read_fallback: "timed_out"` is the evidence
/// that bd was called; whether bd's process had started within the 200 ms is
/// not asserted, since that is the host's timing.)
#[test]
fn fresh_within_runs_out() {
    let repo = synced();
    // Warm the binaries, so the first execution of a fresh file does not use
    // up the budget.
    ok(&repo, &["ready", "--scope", "x"]);
    let gate = repo.gate_file();
    repo.edit(|s| s.faults.gate_file = Some(gate.clone()));

    let (exit, v) = repo.json(&["ready", "--scope", "x", "--fresh-within-ms", "200"]);
    assert!(
        !gate_passed(&gate).exists(),
        "bdc returned only after bd passed its gate: {v}"
    );
    repo.release_gate();
    assert_eq!(exit, 0, "{v}");
    let x = &v["data"]["x_cache"];
    assert_eq!(x["read_fallback"], "timed_out", "{v}");
    assert_eq!(x["basis"]["kind"], "cached", "{v}");
    assert_eq!(v["data"]["items"].as_array().map(Vec::len), Some(2));
}

/// Gives the repository a one-second per-call cap (`call_timeout_s`).
fn one_second_call_timeout(repo: &TestRepo) {
    std::fs::write(
        repo.root().join(".beads").join("bd-cache.toml"),
        "scopes = [\"x\"]\ncall_timeout_s = 1\n",
    )
    .unwrap();
}

/// Far past the one-second cap: the parent kills fake-bd long before.
const HUNG_MS: u64 = 60_000;

/// QA RCR-001: a bd that hangs (it times out, not one that fails to start)
/// makes `cache refresh scopes` a failed result: exit 0, `failed: 1`,
/// `BD_UNAVAILABLE` with `details.timed_out`, and the never-synced scope is
/// still not served.
#[test]
fn a_hung_bd_fails_refresh_scopes() {
    let repo = TestRepo::new(&two_beads());
    one_second_call_timeout(&repo);
    repo.edit(|s| s.faults.slow_ms = HUNG_MS);
    let (exit, v) = repo.json(&["cache", "refresh", "scopes", "x"]);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["data"]["failed"], 1, "a hung bd refreshed nothing: {v}");
    let result = &v["data"]["results"][0];
    let e = &result["error"];
    assert_eq!(e["code"], "BD_CACHE.BD_UNAVAILABLE", "{v}");
    assert_eq!(e["kind"], "unavailable", "{v}");
    assert_eq!(e["retryable"], true, "{v}");
    assert_eq!(e["details"]["timed_out"], true, "{v}");
    assert_eq!(result["report"]["deferred"], true, "{v}");
    let (exit, v) = repo.json(&["issues", "list", "--scope", "x"]);
    assert_eq!(exit, 3, "{v}");
    assert_eq!(v["error"]["code"], "BD_CACHE.SCOPE_UNKNOWN", "{v}");

    // The control: the same repository with bd answering refreshes and serves.
    repo.edit(|s| s.faults.slow_ms = 0);
    let data = ok(&repo, &["cache", "refresh", "scopes", "x"]);
    assert_eq!(data["failed"], 0, "{data}");
    let listed = ok(&repo, &["issues", "list", "--scope", "x"]);
    assert_eq!(
        listed["items"].as_array().map(Vec::len),
        Some(2),
        "{listed}"
    );
}

/// I10: `cache refresh scopes --timeout-ms N` bounds each scope's sync. Bd
/// is blocked at its gate (the `gate_file` fault), so the 300 ms deadline is
/// the only thing that can end the command, and it ends it with bd's
/// `passed` marker absent: a failed `BD_UNAVAILABLE` result whose message
/// names the 300 ms. The control, with the gate open and no `--timeout-ms`,
/// refreshes.
#[test]
fn refresh_scopes_timeout_ms_bounds_the_sync() {
    let repo = synced();
    // Warm the binaries, so the first execution of a fresh file does not use
    // up the budget.
    ok(&repo, &["cache", "refresh", "scopes", "x"]);
    let gate = repo.gate_file();
    repo.edit(|s| s.faults.gate_file = Some(gate.clone()));

    let (exit, v) = repo.json(&["cache", "refresh", "scopes", "x", "--timeout-ms", "300"]);
    assert!(
        !gate_passed(&gate).exists(),
        "bdc returned only after bd passed its gate: {v}"
    );
    repo.release_gate();
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["data"]["failed"], 1, "{v}");
    let result = &v["data"]["results"][0];
    assert_eq!(result["report"]["deferred"], true, "{v}");
    let e = &result["error"];
    assert_eq!(e["code"], "BD_CACHE.BD_UNAVAILABLE", "{v}");
    assert_eq!(e["details"]["timed_out"], true, "{v}");
    assert!(e["message"].as_str().unwrap().contains("300 ms"), "{v}");

    // The control: the gate is open, and with no --timeout-ms the sync runs.
    let data = ok(&repo, &["cache", "refresh", "scopes", "x"]);
    assert_eq!(data["failed"], 0, "{data}");
}

/// I7: `cache health` reports the pending age and drain time, and
/// `--max-pending-age-s` reaches the api (a bad value is `BAD_ARGS`, not
/// ignored). The age limit itself is tested on a fake clock in bd-cache-api.
#[test]
fn health_reports_the_pending_age_and_takes_the_age_limit() {
    let repo = synced();
    repo.edit(|s| s.faults.unavailable = true);
    ok(
        &repo,
        &[
            "issues",
            "update",
            "b-1",
            "--actor",
            "dev",
            "--idempotency-key",
            "age-1",
            "--status",
            "in_progress",
        ],
    );
    let health = ok(&repo, &["cache", "health", "--max-pending-age-s", "86400"]);
    // The entry is younger than the limit: the only reason is that no sync
    // row exists (REQ-BDC-0732, lead ruling 2 of sprint b-9).
    assert_eq!(health["state"], "degraded", "{health}");
    assert_eq!(
        health["reasons"],
        json!([{"code": "no_sync_row_enabled", "message": health["reasons"][0]["message"]}]),
        "{health}"
    );
    assert!(health["oldest_pending_age_s"].is_u64(), "{health}");
    assert_eq!(health["last_drain_at"], Value::Null, "{health}");
    assert_eq!(health["unresolved_escalations"], 0, "{health}");
    let (exit, v) = repo.json(&["cache", "health", "--max-pending-age-s", "soon"]);
    assert_ne!(exit, 0, "{v}");
}

/// REQ-BDC-0732: `bdc cache health` reads the sync rows of the cache file:
/// no row at all is mode `none` and `degraded` with `no_sync_row_enabled`
/// (lead ruling 2 of sprint b-9); the built-in row is mode `builtin` with
/// its last run; both rows disabled is `degraded` with `no_sync_row_enabled`
/// (planted negative: enabled again, `ok`).
#[test]
fn health_reports_the_sync_mode_and_no_sync_row_enabled() {
    let repo = synced();
    let health = ok(&repo, &["cache", "health"]);
    assert_eq!(health["state"], "degraded", "{health}");
    assert_eq!(health["sync"]["mode"], "none", "{health}");
    assert_eq!(
        health["reasons"][0]["code"], "no_sync_row_enabled",
        "{health}"
    );

    ok(&repo, &["cache", "schedule", "set", "0 * * * * *"]);
    let health = ok(&repo, &["cache", "health"]);
    assert_eq!(health["state"], "ok", "{health}");
    assert_eq!(health["sync"]["mode"], "builtin", "{health}");
    assert_eq!(health["sync"]["last_run"]["row"], "builtin", "{health}");
    assert_eq!(
        health["sync"]["last_run"]["last_run_at"],
        Value::Null,
        "{health}"
    );

    ok(&repo, &["cache", "schedule", "disable"]);
    let health = ok(&repo, &["cache", "health"]);
    assert_eq!(health["state"], "degraded", "{health}");
    assert_eq!(
        health["reasons"][0]["code"], "no_sync_row_enabled",
        "{health}"
    );
    assert_eq!(health["sync"]["last_run"], Value::Null, "{health}");

    ok(&repo, &["cache", "schedule", "enable"]);
    assert_eq!(ok(&repo, &["cache", "health"])["state"], "ok");
}

/// QA ARCH-001: `cache health --probe` with bd unreachable on its
/// fingerprint call (`bd version` hangs, or bd cannot start) is exit 0 with
/// `state: "down"`, the outage and `probe: null` (contract §3, §6).
#[test]
fn a_probe_that_cannot_reach_bd_is_down() {
    let repo = synced();
    one_second_call_timeout(&repo);
    // A sync row, so only the probe decides (REQ-BDC-0732).
    ok(&repo, &["cache", "schedule", "set", "0 * * * * *"]);
    // The control: bd answers, so the probe runs and the cache is ok.
    let health = ok(&repo, &["cache", "health", "--probe"]);
    assert_eq!(health["state"], "ok", "{health}");
    assert!(health["probe"].is_object(), "{health}");

    repo.edit(|s| s.faults.slow_ms = HUNG_MS);
    let health = ok(&repo, &["cache", "health", "--probe"]);
    assert_eq!(health["state"], "down", "{health}");
    assert_eq!(health["probe"], Value::Null, "{health}");
    assert_eq!(
        health["outage"]["last_error"]["code"], "BD_CACHE.BD_UNAVAILABLE",
        "{health}"
    );
    // The probe recorded the outage: a plain `cache health` (no bd call) sees it.
    let plain = ok(&repo, &["cache", "health"]);
    assert_eq!(plain["state"], "down", "{plain}");

    // A bd that cannot start at all is down the same way.
    let fresh = TestRepo::new(&two_beads());
    let root = fresh.root();
    let missing = root.join("no-such-bd");
    let ran = fresh.bdc_raw(
        &root,
        &[
            "--repo",
            path_str(&root),
            "--bd",
            path_str(&missing),
            "cache",
            "health",
            "--probe",
        ],
        &[],
    );
    let (exit, v) = envelope_json(&ran);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["data"]["state"], "down", "{v}");
    assert_eq!(v["data"]["probe"], Value::Null, "{v}");
    assert_eq!(
        v["data"]["outage"]["last_error"]["code"], "BD_CACHE.BD_UNAVAILABLE",
        "{v}"
    );
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
