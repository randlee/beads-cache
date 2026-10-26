//! A new scope argument takes a scope name as written (ADR-BDC-0013; REQ-BDC-0812 and
//! 0909 as amended; AC4).
//!
//! On a cache holding the phase-a scope stored as `x` (label `phase-x`), `issues list
//! --scope x` and `ready --scope x` read it, and the prefixed spelling is
//! `SCOPE_UNKNOWN`: the legacy prefix is read on the 0.1 paths only (planted negative:
//! apply the prefix rule to a new argument, and `the_prefixed_name_is_scope_unknown`
//! fails). A read of an unknown name is `SCOPE_UNKNOWN` and registers nothing; `cache
//! refresh scopes` registers it on demand as the label scope with that one label.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use bd_cache_model::{CacheStore, LEGACY_SCOPE_PREFIX};
use common::{TestCache, World, default_facts, run};
use serde_json::{Map, Value, json};

fn world() -> (World, TestCache) {
    let w = World::new();
    w.add("b1", &Map::new());
    w.add("b2", &Map::new());
    let mut cache = w.cache();
    w.sync(&mut cache, "x");
    (w, cache)
}

fn ids(data: &Value) -> Vec<String> {
    data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn the_name_as_written_reads_the_phase_a_scope() {
    let (_w, mut cache) = world();
    for command in ["issues list", "ready"] {
        let got = run(
            &mut cache,
            command,
            &json!({"scope": "x"}),
            &default_facts(),
        );
        assert_eq!(got["ok"], true, "{command}: {got}");
        assert_eq!(ids(&got["data"]), ["b1", "b2"], "{command}");
        assert_eq!(got["data"]["x_cache"]["scope"], "x");
    }
}

#[test]
fn the_prefixed_name_is_scope_unknown() {
    let (_w, mut cache) = world();
    let prefixed = format!("{LEGACY_SCOPE_PREFIX}x");
    for command in ["issues list", "ready"] {
        let got = run(
            &mut cache,
            command,
            &json!({"scope": prefixed}),
            &default_facts(),
        );
        assert_eq!(got["ok"], false, "{command}: {got}");
        assert_eq!(
            got["error"]["code"], "BD_CACHE.SCOPE_UNKNOWN",
            "{command}: {got}"
        );
        assert_eq!(got["error"]["details"]["known"], json!(["x"]));
    }
}

#[test]
fn a_read_of_an_unknown_name_registers_nothing_and_a_refresh_registers_it() {
    let (w, mut cache) = world();
    let got = run(
        &mut cache,
        "issues list",
        &json!({"scope": "lane"}),
        &default_facts(),
    );
    assert_eq!(got["error"]["code"], "BD_CACHE.SCOPE_UNKNOWN", "{got}");
    let known: Vec<String> = w
        .store
        .scopes()
        .unwrap()
        .iter()
        .map(|s| s.id.to_string())
        .collect();
    assert_eq!(known, ["x"], "a read registers nothing");
    let mut lane = Map::new();
    lane.insert("labels".into(), json!(["lane"]));
    w.add("l1", &lane);
    let refreshed = run(
        &mut cache,
        "cache refresh scopes",
        &json!({"scopes": ["lane"]}),
        &default_facts(),
    );
    assert_eq!(refreshed["data"]["failed"], 0, "{refreshed}");
    let got = run(
        &mut cache,
        "issues list",
        &json!({"scope": "lane"}),
        &default_facts(),
    );
    assert_eq!(
        ids(&got["data"]),
        ["l1"],
        "the label scope with that one label: {got}"
    );
    let scopes = run(&mut cache, "cache scopes", &json!({}), &default_facts());
    let names: Vec<&str> = scopes["data"]["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["scope"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["lane", "x"]);
}
