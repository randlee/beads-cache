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
    assert_eq!(r["entry"]["note"], "closed by hand");
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
    assert_eq!(again["entry"]["note"], "closed by hand", "left unchanged");

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
    let failed = refreshed["failed"].as_u64().unwrap();
    let report = &refreshed["results"][0]["report"];
    // An unreachable backend is an outage: reported, not a failed scope.
    assert!(
        failed == 1 || report["outage"] != Value::Null,
        "{refreshed}"
    );
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
    // The core keeps a pruned scope registered (CacheStore rules), so a
    // configured scope lists empty, with no sync, rather than SCOPE_UNKNOWN.
    let gone = ok(
        &mut cache,
        "issues list",
        &json!({"scope": "x", "all": true}),
    );
    assert_eq!(gone["items"], json!([]));
    assert_eq!(gone["x_cache"]["last_ok_at"], Value::Null);
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
