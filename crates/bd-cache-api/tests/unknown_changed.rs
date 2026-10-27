//! Lead ruling S68: a duplicate of an entry with no stored `changed` that is
//! no longer pending has an unknown `changed`, so the reply omits it (and
//! `already_closed`, which is `!changed`): absent, never `null`, never guessed.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use bd_cache::testing::store_mut;
use bd_cache_model::{CacheStore, IdemKey, NewEntry, Op, ScopeId};
use common::{START, World, default_facts, run};
use sc_beads::{Actor, BeadId, Guards, Timestamp};
use serde_json::{Map, Value, json};

/// A world whose journal holds one applied entry written before b-1, so with
/// no stored `changed`: `op` with `args` on `b1`, under key `k-old`.
fn world_with_old_applied_entry(op: &str, args: &str) -> common::TestCache {
    let w = World::new();
    w.add("b1", &Map::new());
    let mut cache = w.cache();
    w.sync(&mut cache, "x");
    let at = Timestamp::parse(START).unwrap();
    let store = store_mut(&mut cache);
    let seq = store
        .insert_entry(
            NewEntry {
                idem_key: IdemKey::new("k-old").unwrap(),
                scope: ScopeId::new("x").ok(),
                bead: BeadId::new("b1").unwrap(),
                op: Op::parse(op, args).unwrap(),
                precondition: Guards::default(),
                actor: Actor::new("san").unwrap(),
                changed: None,
            },
            &at,
        )
        .unwrap();
    let row = store
        .get_row(&BeadId::new("b1").unwrap())
        .unwrap()
        .unwrap()
        .0;
    store
        .apply_with_readback(seq, Some(&row), None, &at)
        .unwrap();
    cache
}

fn data(cache: &mut common::TestCache, command: &str, req: &Value) -> Value {
    let got = run(cache, command, req, &default_facts());
    assert_eq!(got["ok"], true, "{got}");
    got["data"].clone()
}

#[test]
fn an_update_duplicate_with_unknown_changed_omits_it() {
    let mut cache = world_with_old_applied_entry("update", r#"{"status": "in_progress"}"#);
    let d = data(
        &mut cache,
        "issues update",
        &json!({"id": "b1", "actor": "san", "idempotency_key": "k-old", "patch": {"status": "in_progress"}}),
    );
    assert_eq!(d["x_cache"]["duplicate"], true);
    assert_eq!(d["x_cache"]["state"], "applied");
    assert!(
        d.get("changed").is_none(),
        "unknown is absent, never null or a guess: {d}"
    );
}

#[test]
fn a_close_duplicate_with_unknown_changed_omits_already_closed() {
    let mut cache = world_with_old_applied_entry("close", r#"{"reason": "done"}"#);
    let d = data(
        &mut cache,
        "issues close",
        &json!({"id": "b1", "actor": "san", "idempotency_key": "k-old", "reason": "done"}),
    );
    assert_eq!(d["x_cache"]["duplicate"], true);
    assert!(d.get("already_closed").is_none(), "{d}");
    assert!(d.get("changed").is_none(), "{d}");
}

#[test]
fn a_known_changed_is_still_present() {
    let mut cache = world_with_old_applied_entry("close", r#"{"reason": "done"}"#);
    let d = data(
        &mut cache,
        "issues update",
        &json!({"id": "b1", "actor": "san", "idempotency_key": "k-new", "patch": {"add_labels": ["l"]}}),
    );
    assert_eq!(d["changed"], true);
}
