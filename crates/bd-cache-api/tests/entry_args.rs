//! Contract §6 `Entry`: `args` is the `/v0` patch, mapped from the stored
//! BD-CACHE form; a `/v0` patch decoded and journaled reads back identically.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use bd_cache_model::IdemKey;
use common::{World, default_facts, run};
use sc_beads::{Actor, BeadId, Guards};
use serde_json::{Map, Value, json};

fn world() -> (World, common::TestCache) {
    let w = World::new();
    let mut meta = Map::new();
    meta.insert("metadata".into(), json!({"old": "x", "keep": 1}));
    meta.insert("labels".into(), json!(["phase-x", "red"]));
    w.add("b1", &meta);
    let mut cache = w.cache();
    w.sync(&mut cache, "x");
    (w, cache)
}

fn entry(cache: &mut common::TestCache, seq: i64) -> Value {
    let got = run(
        cache,
        "cache journal get",
        &json!({"seq": seq}),
        &default_facts(),
    );
    assert_eq!(got["ok"], true, "{got}");
    got["data"]["entry"].clone()
}

#[test]
fn a_stored_set_metadata_entry_reads_back_as_metadata_set() {
    let (_w, mut cache) = world();
    // The binding's old `write` verb journals the BD-CACHE spelling.
    let reply = cache
        .write(
            &BeadId::new("b1").unwrap(),
            "update",
            r#"{"set_metadata": {"k": "v"}, "unset_metadata": ["old"]}"#,
            Guards::default(),
            IdemKey::new("k-old-verb").unwrap(),
            Actor::new("san").unwrap(),
        )
        .unwrap();
    let e = entry(&mut cache, reply.seq.get());
    assert_eq!(e["op"], "update");
    assert_eq!(
        e["args"],
        json!({"metadata": {"set": {"k": "v"}, "unset": ["old"]}})
    );
    assert!(e["args"].get("set_metadata").is_none());
}

#[test]
fn a_v0_patch_decoded_and_journaled_reads_back_identically() {
    let (_w, mut cache) = world();
    let patches = [
        json!({"metadata": {"set": {"k": "v", "n": 3, "b": true}}}),
        json!({"metadata": {"unset": ["old"]}}),
        json!({"metadata": {"set": {"z": "1x"}, "unset": ["keep"]}}),
        json!({"status": "in_progress", "assignee": "san", "add_labels": ["blue"], "remove_labels": ["red"]}),
        json!({"assignee": "", "append_notes": "a note"}),
    ];
    for (i, patch) in patches.iter().enumerate() {
        let req =
            json!({"id": "b1", "actor": "san", "idempotency_key": format!("k{i}"), "patch": patch});
        let got = run(&mut cache, "issues update", &req, &default_facts());
        assert_eq!(got["ok"], true, "{patch}: {got}");
        let seq = got["data"]["x_cache"]["seq"].as_i64().unwrap();
        assert_eq!(
            entry(&mut cache, seq)["args"],
            *patch,
            "patch {i} round trip"
        );
    }
}

#[test]
fn close_and_dep_add_args_are_in_v0_names() {
    let (_w, mut cache) = world();
    let f = default_facts();
    let got = run(
        &mut cache,
        "issues close",
        &json!({"id": "b1", "actor": "san", "idempotency_key": "kc", "reason": "done"}),
        &f,
    );
    let seq = got["data"]["x_cache"]["seq"].as_i64().unwrap();
    assert_eq!(entry(&mut cache, seq)["args"], json!({"reason": "done"}));
    let edge = json!({"issue_id": "b1", "depends_on_id": "b1", "type": "related"});
    let got = run(
        &mut cache,
        "dependencies add",
        &json!({"actor": "san", "idempotency_key": "kd", "edges": [edge]}),
        &f,
    );
    let seq = got["data"]["x_cache"]["seq"].as_i64().unwrap();
    assert_eq!(entry(&mut cache, seq)["args"], json!({"edges": [edge]}));
}
