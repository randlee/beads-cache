//! The phase-B verbs in `bd-cache-api` (ADR-BDC-0015): one decoder and
//! model for every front end, the store-only verbs executed here, and the
//! front-end verbs decoded here but run elsewhere. Each rule has a planted
//! negative.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use bd_cache_api::{CacheFacts, Command, decode, execute};
use common::{TestCache, World, default_facts};

/// Decodes and runs one request; its `data`, or a panic naming the error.
fn run(cache: &mut TestCache, command: &str, request: &Value, facts: &CacheFacts) -> Value {
    let reply = common::run(cache, command, request, facts);
    assert_eq!(reply["ok"], true, "{command}: {reply}");
    reply["data"].clone()
}
use serde_json::{Map, Value, json};

fn err(command: Command, req: &Value) -> bd_cache_api::ApiError {
    decode(command, req).unwrap_err()
}

#[test]
fn every_new_verb_is_a_command_and_cache_tick_parses() {
    for path in [
        "cache scopes add", "cache scopes set", "cache scopes remove", "cache scopes list",
        "cache scopes ingest", "cache scopes announce", "cache schedule set", "cache schedule show",
        "cache schedule enable", "cache schedule disable", "cache schedule clear", "cache cron run",
        "cache tick", "cache sync use-python", "cache sync use-builtin",
    ] {
        assert!(Command::parse(path).is_some(), "{path}");
    }
    assert!(Command::CacheTick.is_front_end() && Command::SyncUsePython.is_front_end());
    assert!(!Command::ScopesIngest.is_front_end() && !Command::CacheScopes.is_front_end());
}

#[test]
fn the_decoder_refuses_unknown_members_and_bad_shapes_of_the_new_verbs() {
    let cases = [
        (Command::ScopesAdd, json!({"scope": "s", "labels_all": ["a"], "surprise": 1}), "surprise"),
        (Command::ScopesAdd, json!({"labels_all": ["a"]}), "scope"),
        (Command::ScopesAdd, json!({"scope": "s"}), "filter"),
        (Command::ScopesAdd, json!({"scope": "s", "all": true, "labels_all": ["a"]}), "filter"),
        (Command::ScopesAnnounce, json!({"scope": "s"}), "since"),
        (Command::ScheduleSet, json!({"budget_ms": 1}), "cron"),
        (Command::ScheduleSet, json!({"cron": "* * * * *", "budget_ms": "x"}), "budget_ms"),
        (Command::CronRun, json!({"shutdown_timeout_ms": 600_001}), "shutdown_timeout_ms"),
        (Command::SyncUsePython, json!({"python": "/p"}), "module"),
        (Command::ScheduleShow, json!({"x": 1}), "x"),
    ];
    for (command, req, member) in cases {
        let e = err(command, &req);
        assert_eq!(e.code(), "BD_CACHE.BAD_ARGS", "{req}");
        assert_eq!(e.details()["member"], member, "{req}");
    }
    // Planted negative: the valid forms decode.
    assert!(decode(Command::ScopesAdd, &json!({"scope": "s", "labels_all": ["a"]})).is_ok());
    assert!(decode(Command::CronRun, &json!({"once": true})).is_ok());
    assert!(decode(Command::ScheduleSet, &json!({"cron": "* * * * *", "env": ["TOKEN"]})).is_ok());
}

#[test]
fn a_front_end_verb_is_decoded_here_but_not_executed_here() {
    let w = World::new();
    let mut cache = w.cache();
    let req = decode(Command::CacheTick, &json!({})).unwrap();
    let e = execute(&mut cache, &req, &CacheFacts::default()).unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.INTERNAL");
}

#[test]
fn scopes_add_set_list_remove_round_trip_and_cache_scopes_is_scopes_lists_alias() {
    let w = World::new();
    let mut cache = w.cache();
    let f = default_facts();
    let added = run(&mut cache, "cache scopes add", &json!({"scope": "bugs", "labels_any": ["bug"], "status": ["open"]}), &f);
    assert_eq!(added["scope"], "bugs");
    assert_eq!(added["filter"]["labels_any"], json!(["bug"]));
    let again = decode(Command::ScopesAdd, &json!({"scope": "bugs", "all": true})).unwrap();
    assert_eq!(execute(&mut cache, &again, &f).unwrap_err().code(), "BD_CACHE.BAD_ARGS", "a second add");
    run(&mut cache, "cache scopes set", &json!({"scope": "bugs", "all": true}), &f);
    let list = run(&mut cache, "cache scopes list", &json!({}), &f);
    let alias = run(&mut cache, "cache scopes", &json!({}), &f);
    assert_eq!(list, alias, "the same reply");
    let bugs = list["scopes"].as_array().unwrap().iter().find(|s| s["scope"] == "bugs").unwrap().clone();
    assert_eq!(bugs["filter"]["all"], true, "each scope's filter");
    let removed = run(&mut cache, "cache scopes remove", &json!({"scope": "bugs"}), &f);
    assert_eq!(removed["removed"], true);
    let list = run(&mut cache, "cache scopes list", &json!({}), &f);
    assert!(list["scopes"].as_array().unwrap().iter().all(|s| s["scope"] != "bugs"));
    let set = decode(Command::ScopesSet, &json!({"scope": "bugs", "all": true})).unwrap();
    assert_eq!(execute(&mut cache, &set, &f).unwrap_err().code(), "BD_CACHE.SCOPE_UNKNOWN");
}

#[test]
fn scopes_remove_of_a_scope_with_pending_entries_is_prune_blocked() {
    // `x` is not configured here, so the registry may remove it.
    let mut w = World::new();
    w.scopes.clear();
    w.add("b1", &Map::new());
    let mut cache = w.cache();
    w.sync(&mut cache, "x");
    let facts = default_facts();
    run(&mut cache, "issues close", &json!({"id": "b1", "actor": "a", "idempotency_key": "k1", "reason": "done"}), &facts);
    let req = decode(Command::ScopesRemove, &json!({"scope": "x"})).unwrap();
    assert_eq!(execute(&mut cache, &req, &facts).unwrap_err().code(), "BD_CACHE.PRUNE_BLOCKED");
    // `cache evict` keeps its 0.1 reply and its refusal.
    let evict = decode(Command::CacheEvict, &json!({"scope": "x"})).unwrap();
    assert_eq!(execute(&mut cache, &evict, &facts).unwrap_err().code(), "BD_CACHE.PRUNE_BLOCKED");
}

#[test]
fn scopes_ingest_stores_a_snapshot_and_one_invalid_row_stores_none() {
    let w = World::new();
    let mut cache = w.cache();
    let f = default_facts();
    run(&mut cache, "cache scopes add", &json!({"scope": "mine", "all": true}), &f);
    let row = |id: &str| json!({"id": id, "title": id, "priority": 2, "status": "open",
        "issue_type": "task", "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"});
    let bad = decode(Command::ScopesIngest, &json!({"scope": "mine", "rows": [row("b1"), {"id": "b2", "surprise": 1}]})).unwrap_err();
    assert_eq!(bad.code(), "BD_CACHE.INGEST_INVALID");
    assert_eq!(bad.details()["index"], 1);
    let list = run(&mut cache, "cache scopes list", &json!({}), &f);
    let mine = list["scopes"].as_array().unwrap().iter().find(|s| s["scope"] == "mine").unwrap().clone();
    assert_eq!(mine["beads"], 0, "no row of the snapshot stored");
    let done = run(&mut cache, "cache scopes ingest", &json!({"scope": "mine", "rows": [row("b1"), row("b2")], "revision": "r1"}), &f);
    assert_eq!((done["rows"].clone(), done["stored"].clone(), done["revision"].clone()), (2.into(), 2.into(), "r1".into()));
    let empty = decode(Command::ScopesIngest, &json!({"scope": "mine", "rows": []})).unwrap();
    assert_eq!(execute(&mut cache, &empty, &f).unwrap_err().code(), "BD_CACHE.INGEST_INVALID");
    let unknown = decode(Command::ScopesIngest, &json!({"scope": "nobody", "rows": [row("b1")]})).unwrap();
    assert_eq!(execute(&mut cache, &unknown, &f).unwrap_err().code(), "BD_CACHE.SCOPE_UNKNOWN");
}

#[test]
fn scopes_announce_marks_only_the_current_failure() {
    let w = World::new();
    let mut cache = w.cache();
    let f = default_facts();
    run(&mut cache, "cache scopes add", &json!({"scope": "mine", "all": true}), &f);
    let req = decode(Command::ScopesAnnounce, &json!({"scope": "mine", "since": "2026-10-23T09:00:00Z"})).unwrap();
    assert_eq!(execute(&mut cache, &req, &f).unwrap_err().code(), "BD_CACHE.PRECONDITION", "no current failure");
}
