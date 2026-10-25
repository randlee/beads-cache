//! Contract §6, surface (b): flush, the journal commands, health, scopes,
//! refresh and evict, with `Entry.last_error` and `note` kept apart.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use common::{World, default_facts, run};
use sc_beads::Refusal;
use sc_beads::testing::{FakeOp, Injection};
use serde_json::{Map, Value, json};

fn world() -> (World, common::TestCache) {
    let w = World::new();
    let mut b1 = Map::new();
    b1.insert("status".into(), json!("in_progress"));
    b1.insert("assignee".into(), json!("dev"));
    w.add("b1", &b1);
    w.add("b2", &Map::new());
    let mut cache = w.cache();
    w.sync(&mut cache, "x");
    (w, cache)
}

fn ok(cache: &mut common::TestCache, command: &str, req: &Value) -> Value {
    let got = run(cache, command, req, &default_facts());
    assert_eq!(got["ok"], true, "{command} {req}: {got}");
    got["data"].clone()
}

fn err(cache: &mut common::TestCache, command: &str, req: &Value) -> Value {
    let got = run(cache, command, req, &default_facts());
    assert_eq!(got["ok"], false, "{command} {req}: {got}");
    got["error"].clone()
}

#[test]
fn an_escalation_is_listed_announced_and_resolved_with_its_note_apart() {
    let (w, mut cache) = world();
    ok(
        &mut cache,
        "issues update",
        &json!({"id": "b2", "actor": "san", "idempotency_key": "k1", "patch": {"status": "in_progress"}}),
    );
    ok(
        &mut cache,
        "issues close",
        &json!({"id": "b1", "actor": "san", "idempotency_key": "k2"}),
    );
    w.bd.inject(FakeOp::Close, Injection::Refused(Refusal::NotClosable));
    let flush = ok(&mut cache, "cache flush", &json!({}));
    assert_eq!(flush["applied"], json!([1]));
    assert_eq!(flush["busy"], false);
    let esc = &flush["escalated"][0];
    assert_eq!(
        (esc["seq"].as_i64(), esc["bead"].as_str()),
        (Some(2), Some("b1"))
    );
    assert_eq!(esc["scope"], "phase-x");
    assert_eq!(esc["reason"]["code"], "BD_CACHE.PRECONDITION");

    let listed = ok(&mut cache, "cache journal list", &json!({}));
    assert_eq!(listed["entries"].as_array().unwrap().len(), 1);
    assert_eq!(listed["entries"][0]["state"], "escalated");
    let unannounced = ok(
        &mut cache,
        "cache journal list",
        &json!({"unannounced": true}),
    );
    assert_eq!(unannounced["entries"][0]["seq"], 2);

    let a = ok(&mut cache, "cache journal announce", &json!({"seq": 2}));
    assert_eq!(a["already_announced"], false);
    assert_eq!(a["entry"]["announced_at"], "2026-10-23T10:00:00.000Z");
    let again = ok(&mut cache, "cache journal announce", &json!({"seq": 2}));
    assert_eq!(again["already_announced"], true);
    let none = ok(
        &mut cache,
        "cache journal list",
        &json!({"unannounced": true}),
    );
    assert_eq!(none["entries"], json!([]));

    let r = ok(
        &mut cache,
        "cache journal resolve",
        &json!({"seq": 2, "note": "closed by hand", "actor": "lead"}),
    );
    assert_eq!(r["already_resolved"], false);
    assert_eq!(r["entry"]["state"], "resolved");
    assert_eq!(
        r["entry"]["note"], "resolved by lead: closed by hand",
        "lead ruling S65"
    );
    assert_eq!(r["entry"]["last_error"]["code"], "BD_CACHE.PRECONDITION");
    assert!(
        !r["entry"]["last_error"]["message"]
            .as_str()
            .unwrap()
            .contains("closed by hand"),
        "the note is not part of last_error"
    );
    let again = ok(
        &mut cache,
        "cache journal resolve",
        &json!({"seq": 2, "note": "other", "actor": "lead"}),
    );
    assert_eq!(again["already_resolved"], true);
    assert_eq!(
        again["entry"]["note"], "resolved by lead: closed by hand",
        "left unchanged"
    );

    let e = err(
        &mut cache,
        "cache journal resolve",
        &json!({"seq": 1, "note": "n", "actor": "lead"}),
    );
    assert_eq!(e["code"], "BD_CACHE.NOT_ESCALATED");

    let status = ok(&mut cache, "cache status", &json!({}));
    assert_eq!(status["escalated"], json!([]));
    assert_eq!(status["journal"]["applied"], 1);
    assert_eq!(status["journal"]["resolved"], 1);
    assert_eq!(status["contract_version"], "1.0.0");
    assert_eq!(
        status["project"],
        json!({"database": "beads", "project_id": "proj-1"})
    );
}

#[test]
fn flush_defaults_and_refuses_a_deadline_before_its_start_gate() {
    let (_w, mut cache) = world();
    let e = err(
        &mut cache,
        "cache flush",
        &json!({"start_gate_ms": 1000, "deadline_ms": 1000}),
    );
    assert_eq!(
        (e["code"].as_str(), e["details"]["member"].as_str()),
        (Some("BD_CACHE.BAD_ARGS"), Some("deadline_ms"))
    );
    let flush = ok(&mut cache, "cache flush", &json!({}));
    assert_eq!(flush["applied"], json!([]));
}

#[test]
fn health_reports_down_during_an_outage_and_probes_on_request() {
    let (w, mut cache) = world();
    let h = ok(&mut cache, "cache health", &json!({}));
    assert_eq!(h["state"], "ok");
    assert_eq!(h["probe"], Value::Null);
    let p = ok(&mut cache, "cache health", &json!({"probe": true}));
    assert_eq!(p["probe"]["model_current"], true);
    assert_eq!(p["probe"]["rebuilt"], false);

    w.bd.inject(
        FakeOp::ListScope,
        Injection::Unavailable { timed_out: false },
    );
    let refreshed = ok(&mut cache, "cache refresh scopes", &json!({}));
    assert_eq!(refreshed["results"][0]["scope"], "phase-x");
    // Lead ruling S75: a sync that could not reach bd is a failed result,
    // BD_UNAVAILABLE (unavailable, retryable), though the core returned Ok
    // with an outage in its report.
    assert_eq!(refreshed["failed"], 1, "{refreshed}");
    let result = &refreshed["results"][0];
    assert_eq!(
        result["error"]["code"], "BD_CACHE.BD_UNAVAILABLE",
        "{result}"
    );
    assert_eq!(result["error"]["kind"], "unavailable", "{result}");
    assert_eq!(result["error"]["retryable"], true, "{result}");
    assert_ne!(result["report"]["outage"], Value::Null, "{result}");
    let h = ok(&mut cache, "cache health", &json!({}));
    assert_eq!(h["state"], "down");
    assert_eq!(h["outage"]["last_error"]["code"], "BD_CACHE.BD_UNAVAILABLE");
}

#[test]
fn refresh_scopes_names_each_scope_and_counts_failures() {
    let (w, mut cache) = world();
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    let r = ok(
        &mut cache,
        "cache refresh scopes",
        &json!({"scopes": ["x", "phase-y"]}),
    );
    let results = r["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["scope"], "phase-x");
    assert_eq!(results[1]["scope"], "phase-y");
    assert_eq!(results[0]["report"], Value::Null);
    assert_eq!(results[0]["error"]["code"], "BD_CACHE.SYNC_FAILED");
    assert_eq!(results[0]["error"]["kind"], "unavailable");
    assert_eq!(results[0]["error"]["retryable"], true);
    assert_eq!(results[1]["error"], Value::Null);
    assert_eq!(r["failed"], 1);
    let scopes = ok(&mut cache, "cache scopes", &json!({}));
    let y = &scopes["scopes"][1];
    assert_eq!(
        (
            y["scope"].as_str(),
            y["configured"].as_bool(),
            y["on_demand"].as_bool()
        ),
        (Some("phase-y"), Some(false), Some(true))
    );
    assert_eq!(
        scopes["scopes"][0]["last_error"]["code"],
        "BD_CACHE.SYNC_FAILED"
    );
}

#[test]
fn refresh_issues_keeps_the_request_order() {
    let (_w, mut cache) = world();
    let r = ok(
        &mut cache,
        "cache refresh issues",
        &json!({"ids": ["b2", "b1"]}),
    );
    let ids: Vec<&str> = r["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["b2", "b1"]);
    assert_eq!(r["x_cache"]["rows"]["b1"]["stale"], false);
    let e = err(&mut cache, "cache refresh issues", &json!({"ids": []}));
    assert_eq!(e["details"]["member"], "ids");
}

#[test]
fn evict_is_blocked_by_pending_writes() {
    let (_w, mut cache) = world();
    ok(
        &mut cache,
        "issues update",
        &json!({"id": "b2", "actor": "san", "idempotency_key": "k1", "patch": {"add_labels": ["l"]}}),
    );
    let e = err(&mut cache, "cache evict", &json!({"scope": "phase-x"}));
    assert_eq!(
        (e["code"].as_str(), e["kind"].as_str()),
        (Some("BD_CACHE.PRUNE_BLOCKED"), Some("conflict"))
    );
    ok(&mut cache, "cache flush", &json!({}));
    let r = ok(&mut cache, "cache evict", &json!({"scope": "x"}));
    assert_eq!(r["scope"], "phase-x");
    assert_eq!(r["beads_deleted"], 2);
    // S67: an evicted scope is not served until its next sync, never an empty list.
    for command in ["issues list", "ready"] {
        let gone = err(&mut cache, command, &json!({"scope": "x"}));
        assert_eq!(gone["code"], "BD_CACHE.SCOPE_UNKNOWN", "{command}: {gone}");
        assert_eq!(
            gone["suggested_action"],
            "bdc cache refresh scopes --scope phase-x"
        );
        assert_eq!(gone["details"]["scope"], "phase-x");
    }
    ok(
        &mut cache,
        "cache refresh scopes",
        &json!({"scopes": ["x"]}),
    );
    let back = ok(&mut cache, "issues list", &json!({"scope": "x"}));
    assert_eq!(
        back["items"].as_array().unwrap().len(),
        2,
        "served again after a sync"
    );
}

#[test]
fn journal_list_refuses_states_it_does_not_serve() {
    let (_w, mut cache) = world();
    let e = err(
        &mut cache,
        "cache journal list",
        &json!({"state": "applied"}),
    );
    assert_eq!(e["code"], "BD_CACHE.UNSUPPORTED");
    assert_eq!(e["details"]["unsupported"], "parameter");
    let e = err(&mut cache, "cache journal list", &json!({"state": "nope"}));
    assert_eq!(e["code"], "BD_CACHE.BAD_ARGS");
    let e = err(
        &mut cache,
        "cache journal get",
        &json!({"seq": 1, "idempotency_key": "k"}),
    );
    assert_eq!(e["code"], "BD_CACHE.BAD_ARGS");
    let e = err(&mut cache, "cache journal get", &json!({"seq": 99}));
    assert_eq!(e["code"], "BD_CACHE.NOT_FOUND");
}

#[test]
fn a_configured_scope_never_synced_is_not_served() {
    let w = World::new();
    w.add("b1", &Map::new());
    let mut cache = w.cache();
    for command in ["issues list", "ready"] {
        let e = err(&mut cache, command, &json!({"scope": "phase-x"}));
        assert_eq!(e["code"], "BD_CACHE.SCOPE_UNKNOWN", "{command}: {e}");
        assert_eq!(e["kind"], "conflict");
        assert_eq!(e["details"]["known"], json!([]), "no scope is served yet");
        assert_eq!(
            e["suggested_action"],
            "bdc cache refresh scopes --scope phase-x"
        );
    }
}

/// Lead ruling S75 for `cache refresh issues`: ids that could not reach bd
/// fail the command with `BD_UNAVAILABLE`, unavailable and retryable (there is
/// no per-id result).
#[test]
fn refresh_issues_with_bd_unreachable_is_bd_unavailable() {
    let (w, mut cache) = world();
    w.bd.inject(
        FakeOp::ListByIds,
        Injection::Unavailable { timed_out: false },
    );
    let e = err(&mut cache, "cache refresh issues", &json!({"ids": ["b1"]}));
    assert_eq!(e["code"], "BD_CACHE.BD_UNAVAILABLE", "{e}");
    assert_eq!(e["kind"], "unavailable", "{e}");
    // The envelope's error has no `retryable`; the error table holds it (§4).
    let spec = bd_cache_api::spec(e["code"].as_str().unwrap()).unwrap();
    assert!(spec.retryable, "{e}");
}

/// Far past any per-call cap: the fake moves its clock to the call's deadline
/// and times out, as a bd that hangs does.
const HANG: std::time::Duration = std::time::Duration::from_hours(24);

/// QA RCR-001: a bd that hangs makes the sync's one call time out exactly at
/// the sync's deadline (the per-call cap), which the core reports as `Ok`
/// with `deferred` and no outage. That is a failed result, `BD_UNAVAILABLE`
/// with `details.timed_out`, and the never-synced scope stays unserved.
#[test]
fn refresh_scopes_with_a_hung_bd_is_a_failed_result() {
    for op in [FakeOp::Fingerprint, FakeOp::ListScope] {
        let w = World::new();
        w.add("b1", &Map::new());
        let mut cache = w.cache();
        w.bd.inject(op, Injection::Delay(HANG));
        let r = ok(
            &mut cache,
            "cache refresh scopes",
            &json!({"scopes": ["x"]}),
        );
        assert_eq!(r["failed"], 1, "{op:?}: {r}");
        let result = &r["results"][0];
        assert_eq!(result["report"]["deferred"], true, "{op:?}: {result}");
        assert_eq!(result["report"]["outage"], Value::Null, "{op:?}: {result}");
        let e = &result["error"];
        assert_eq!(e["code"], "BD_CACHE.BD_UNAVAILABLE", "{op:?}: {e}");
        assert_eq!(e["kind"], "unavailable", "{op:?}: {e}");
        assert_eq!(e["retryable"], true, "{op:?}: {e}");
        assert_eq!(e["details"]["timed_out"], true, "{op:?}: {e}");
        let gone = err(&mut cache, "issues list", &json!({"scope": "x"}));
        assert_eq!(gone["code"], "BD_CACHE.SCOPE_UNKNOWN", "{op:?}: {gone}");
    }
    // The control: the same world without the hang refreshes and serves.
    let w = World::new();
    w.add("b1", &Map::new());
    let mut cache = w.cache();
    let r = ok(
        &mut cache,
        "cache refresh scopes",
        &json!({"scopes": ["x"]}),
    );
    assert_eq!(r["failed"], 0, "{r}");
    assert_eq!(r["results"][0]["error"], Value::Null, "{r}");
    let listed = ok(&mut cache, "issues list", &json!({"scope": "x"}));
    assert_eq!(listed["items"].as_array().unwrap().len(), 1, "{listed}");
}

/// A failed sync's result carries the details its envelope error would have.
#[test]
fn a_failed_sync_result_carries_its_details() {
    let (w, mut cache) = world();
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    let r = ok(
        &mut cache,
        "cache refresh scopes",
        &json!({"scopes": ["x"]}),
    );
    let e = &r["results"][0]["error"];
    assert_eq!(e["code"], "BD_CACHE.SYNC_FAILED", "{e}");
    assert_eq!(e["details"]["scope"], "phase-x", "{e}");
    // A refreshed scope has no error at all.
    let r = ok(
        &mut cache,
        "cache refresh scopes",
        &json!({"scopes": ["x"]}),
    );
    assert_eq!(r["results"][0]["error"], Value::Null, "{r}");
}

/// QA ARCH-001: `cache health --probe` with bd unreachable on the
/// fingerprint call is a result (contract §3, §6): `state: "down"`, the
/// outage, `probe: null`, never a typed error.
#[test]
fn health_probe_with_bd_unreachable_on_the_fingerprint_is_down() {
    // (fault, the code, whether the probe recorded the outage)
    let cases = [
        (
            Injection::Unavailable { timed_out: false },
            "BD_CACHE.BD_UNAVAILABLE",
            true,
        ),
        // A hang: the per-call cap ends it before the probe's deadline, so it
        // is an outage, recorded, and not a silent cut.
        (Injection::Delay(HANG), "BD_CACHE.BD_UNAVAILABLE", true),
        // bd answers `bd version` with a failure: an unreachable code, but
        // not an outage; the outage is the one the probe observed.
        (Injection::Failed(1), "BD_CACHE.BD_VERSION_FAILED", false),
    ];
    for (fault, code, recorded) in cases {
        let (w, mut cache) = world();
        w.bd.inject(FakeOp::Fingerprint, fault.clone());
        let h = ok(&mut cache, "cache health", &json!({"probe": true}));
        assert_eq!(h["state"], "down", "{fault:?}: {h}");
        assert_eq!(h["probe"], Value::Null, "{fault:?}: {h}");
        assert_eq!(h["outage"]["last_error"]["code"], code, "{fault:?}: {h}");
        assert!(h["outage"]["since"].is_string(), "{fault:?}: {h}");
        let after = ok(&mut cache, "cache health", &json!({}));
        assert_eq!(
            after["state"] == "down",
            recorded,
            "{fault:?}: the probe recorded the outage: {after}"
        );
    }
}

/// Journals an update on `id` (a pending entry) at the world clock's time.
fn journal_update(cache: &mut common::TestCache, id: &str, key: &str) {
    ok(
        cache,
        "issues update",
        &json!({"id": id, "actor": "san", "idempotency_key": key, "patch": {"status": "in_progress"}}),
    );
}

/// I7: a pending entry that nothing drains is `degraded` once it is older
/// than the limit (default 300 s, `max_pending_age_s` overrides it), judged
/// on the oldest entry and on the injected clock; the drain clears it.
#[test]
fn health_degrades_when_the_oldest_pending_entry_is_too_old() {
    let (w, mut cache) = world();
    journal_update(&mut cache, "b2", "k1");
    w.clock.advance(std::time::Duration::from_secs(200));
    // A younger entry must not hide the older one.
    journal_update(&mut cache, "b1", "k2");
    w.clock.advance(std::time::Duration::from_secs(100));

    // Exactly at the limit: still ok (the limit is "older than").
    let h = ok(&mut cache, "cache health", &json!({}));
    assert_eq!(h["oldest_pending_age_s"], 300, "{h}");
    assert_eq!(h["state"], "ok", "{h}");
    assert_eq!(h["reasons"], json!([]), "{h}");
    assert_eq!(h["last_drain_at"], Value::Null, "nothing drained yet: {h}");
    assert_eq!(h["unresolved_escalations"], 0, "{h}");

    w.clock.advance(std::time::Duration::from_secs(1));
    let h = ok(&mut cache, "cache health", &json!({}));
    assert_eq!(h["oldest_pending_age_s"], 301, "{h}");
    assert_eq!(h["state"], "degraded", "{h}");
    assert_eq!(h["reasons"][0]["code"], "pending_too_old", "{h}");
    assert!(
        h["reasons"][0]["message"]
            .as_str()
            .unwrap()
            .contains("301 s"),
        "{h}"
    );

    // The limit is configurable.
    let h = ok(
        &mut cache,
        "cache health",
        &json!({"max_pending_age_s": 301}),
    );
    assert_eq!(h["state"], "ok", "{h}");
    let h = ok(
        &mut cache,
        "cache health",
        &json!({"max_pending_age_s": 60}),
    );
    assert_eq!(h["state"], "degraded", "{h}");
    let bad = err(
        &mut cache,
        "cache health",
        &json!({"max_pending_age_s": -1}),
    );
    assert_eq!(bad["code"], "BD_CACHE.BAD_ARGS", "{bad}");

    // The drain applies both entries: nothing pending, and the drain is dated.
    let flush = ok(&mut cache, "cache flush", &json!({}));
    assert_eq!(flush["applied"], json!([1, 2]), "{flush}");
    let h = ok(&mut cache, "cache health", &json!({}));
    assert_eq!(h["state"], "ok", "{h}");
    assert_eq!(h["oldest_pending_age_s"], Value::Null, "{h}");
    assert_eq!(h["last_drain_at"], "2026-10-23T10:05:01.000Z", "{h}");
}

/// I7: an escalation nobody resolved keeps health `degraded` after it was
/// announced, until `journal resolve` settles it.
#[test]
fn health_degrades_on_an_unresolved_escalation_until_it_is_resolved() {
    let (w, mut cache) = world();
    ok(
        &mut cache,
        "issues close",
        &json!({"id": "b1", "actor": "san", "idempotency_key": "k1"}),
    );
    w.bd.inject(FakeOp::Close, Injection::Refused(Refusal::NotClosable));
    let flush = ok(&mut cache, "cache flush", &json!({}));
    assert_eq!(flush["escalated"][0]["seq"], 1, "{flush}");

    let h = ok(&mut cache, "cache health", &json!({}));
    assert_eq!(h["state"], "degraded", "{h}");
    assert_eq!(h["unresolved_escalations"], 1, "{h}");
    assert_eq!(h["reasons"][0]["code"], "unresolved_escalations", "{h}");
    assert!(
        h["reasons"][0]["message"]
            .as_str()
            .unwrap()
            .contains("journal resolve <seq>"),
        "{h}"
    );

    // Announced, but nobody resolved it: still degraded.
    ok(&mut cache, "cache journal announce", &json!({"seq": 1}));
    let h = ok(&mut cache, "cache health", &json!({}));
    assert_eq!(h["open_work"]["escalated_unannounced"], 0, "{h}");
    assert_eq!(
        h["state"], "degraded",
        "an announced escalation is open: {h}"
    );
    assert_eq!(h["unresolved_escalations"], 1, "{h}");

    ok(
        &mut cache,
        "cache journal resolve",
        &json!({"seq": 1, "note": "closed by hand", "actor": "lead"}),
    );
    let h = ok(&mut cache, "cache health", &json!({}));
    assert_eq!(h["state"], "ok", "{h}");
    assert_eq!(h["unresolved_escalations"], 0, "{h}");
    assert_eq!(h["reasons"], json!([]), "{h}");
}

/// I10: `cache refresh scopes` takes `timeout_ms`, the sync's deadline. A bd
/// that answers in 10 s of (injected) time is cut by a 5 s deadline and
/// refreshes by the default one; the failed result names the deadline.
#[test]
fn refresh_scopes_timeout_ms_is_the_sync_deadline() {
    let delay = std::time::Duration::from_secs(10);
    let w = World::new();
    w.add("b1", &Map::new());
    let mut cache = w.cache();
    w.bd.inject(FakeOp::Fingerprint, Injection::Delay(delay));
    let r = ok(
        &mut cache,
        "cache refresh scopes",
        &json!({"scopes": ["x"], "timeout_ms": 5000}),
    );
    assert_eq!(r["failed"], 1, "{r}");
    let result = &r["results"][0];
    assert_eq!(result["report"]["deferred"], true, "{result}");
    assert_eq!(
        result["error"]["code"], "BD_CACHE.BD_UNAVAILABLE",
        "{result}"
    );
    assert_eq!(result["error"]["details"]["timed_out"], true, "{result}");
    assert!(
        result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("5000 ms"),
        "{result}"
    );
    let gone = err(&mut cache, "issues list", &json!({"scope": "x"}));
    assert_eq!(gone["code"], "BD_CACHE.SCOPE_UNKNOWN", "{gone}");

    // The control: the same delay under the default deadline (30 s) refreshes.
    let w = World::new();
    w.add("b1", &Map::new());
    let mut cache = w.cache();
    w.bd.inject(FakeOp::Fingerprint, Injection::Delay(delay));
    let r = ok(
        &mut cache,
        "cache refresh scopes",
        &json!({"scopes": ["x"]}),
    );
    assert_eq!(r["failed"], 0, "{r}");
    let bad = err(
        &mut cache,
        "cache refresh scopes",
        &json!({"timeout_ms": "soon"}),
    );
    assert_eq!(bad["code"], "BD_CACHE.BAD_ARGS", "{bad}");
}
