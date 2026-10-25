//! REQ-BDC-0808, contract §5.3: both `issues list` orders are total, and a
//! walk with `next_cursor` visits every row exactly once, in the unpaged order.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::collections::BTreeSet;

use common::{World, default_facts, run};
use serde_json::{Map, Value, json};

const ROWS: usize = 250;

/// 250 `phase-x` beads with heavy priority and `created_at` ties, inserted
/// out of id order.
fn world() -> (World, common::TestCache) {
    let w = World::new();
    for n in 0..ROWS {
        // A stride coprime to 250 visits every n once, out of order.
        let i = (n * 37) % ROWS;
        let mut f = Map::new();
        f.insert("priority".into(), json!(i % 3));
        f.insert(
            "created_at".into(),
            json!(format!("2026-10-2{}T09:00:00Z", i % 4)),
        );
        if i.is_multiple_of(5) {
            f.insert("status".into(), json!("closed"));
        }
        w.add(&format!("p-{i:03}"), &f);
    }
    let mut cache = w.cache();
    w.sync(&mut cache, "x");
    (w, cache)
}

fn list(cache: &mut common::TestCache, req: &Value) -> Value {
    let got = run(cache, "issues list", req, &default_facts());
    assert_eq!(got["ok"], true, "{got}");
    got["data"].clone()
}

fn ids(data: &Value) -> Vec<String> {
    data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect()
}

/// The stated order, checked pairwise on the rows themselves.
fn in_order(sort: &str, rows: &[Value]) -> bool {
    rows.windows(2).all(|w| {
        let key = |r: &Value| {
            (
                r["priority"].as_i64().unwrap(),
                r["created_at"].as_str().unwrap().to_owned(),
                r["id"].as_str().unwrap().to_owned(),
            )
        };
        let (a, b) = (key(&w[0]), key(&w[1]));
        // The fixture's timestamps share one format, so text order is time order.
        let created_desc_then_id = a.1 > b.1 || (a.1 == b.1 && a.2 < b.2);
        match sort {
            "created" => created_desc_then_id,
            _ => a.0 < b.0 || (a.0 == b.0 && created_desc_then_id),
        }
    })
}

fn walk(sort: &str) {
    let (_w, mut cache) = world();
    let whole = list(
        &mut cache,
        &json!({"scope": "x", "all": true, "sort": sort}),
    );
    assert_eq!(whole["has_more"], false);
    assert!(whole.get("next_cursor").is_none(), "absent, never null");
    let unpaged = ids(&whole);
    assert_eq!(unpaged.len(), ROWS);
    assert!(
        in_order(sort, whole["items"].as_array().unwrap()),
        "sort={sort}: the unpaged list is not in the stated order"
    );

    let mut walked = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let mut req = json!({"scope": "phase-x", "all": true, "sort": sort, "limit": 7});
        if let Some(c) = &cursor {
            req["cursor"] = json!(c);
        }
        let page = list(&mut cache, &req);
        let got = ids(&page);
        assert!(got.len() <= 7);
        walked.extend(got);
        pages += 1;
        assert!(pages <= ROWS, "the walk does not end");
        if page["has_more"] == false {
            assert!(page.get("next_cursor").is_none());
            break;
        }
        cursor = Some(page["next_cursor"].as_str().unwrap().to_owned());
    }
    let unique: BTreeSet<&String> = walked.iter().collect();
    assert_eq!(
        unique.len(),
        walked.len(),
        "sort={sort}: a row was repeated"
    );
    assert_eq!(
        walked, unpaged,
        "sort={sort}: the pages differ from the unpaged order"
    );
    assert_eq!(pages, ROWS.div_ceil(7));
}

#[test]
fn a_created_walk_concatenates_to_the_unpaged_order() {
    walk("created");
}

#[test]
fn a_priority_walk_concatenates_to_the_unpaged_order() {
    walk("priority");
}

#[test]
fn a_tampered_or_foreign_cursor_is_bad_args_invalid_cursor() {
    let (_w, mut cache) = world();
    let page = list(
        &mut cache,
        &json!({"scope": "x", "all": true, "sort": "priority", "limit": 7}),
    );
    let cursor = page["next_cursor"].as_str().unwrap().to_owned();
    let refused = |cache: &mut common::TestCache, sort: &str, c: &str| {
        let got = run(
            cache,
            "issues list",
            &json!({"scope": "x", "sort": sort, "cursor": c}),
            &default_facts(),
        );
        got["ok"] == false
            && got["error"]["code"] == "BD_CACHE.BAD_ARGS"
            && got["error"]["details"]["v0_code"] == "invalid_cursor"
    };
    // Every one-character edit of the cursor's body is refused.
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    for at in 3..cursor.len() {
        let old = &cursor[at..=at];
        let new = if old == "A" { "B" } else { "A" };
        assert!(alphabet.contains(new));
        let tampered = format!("{}{new}{}", &cursor[..at], &cursor[at + 1..]);
        assert!(refused(&mut cache, "priority", &tampered), "{tampered}");
    }
    for (sort, c) in [
        ("created", cursor.as_str()),
        ("priority", "v2.abc"),
        ("priority", "not a cursor"),
    ] {
        assert!(refused(&mut cache, sort, c), "{sort} {c}");
    }
    assert!(
        !refused(&mut cache, "priority", &cursor),
        "the real cursor is accepted"
    );
}

#[test]
fn inside_a_scope_the_default_is_no_limit_and_zero_is_unlimited() {
    let (_w, mut cache) = world();
    let open = ROWS - ROWS / 5;
    for req in [json!({"scope": "x"}), json!({"scope": "x", "limit": 0})] {
        let data = list(&mut cache, &req);
        assert_eq!(ids(&data).len(), open, "{req}");
        assert_eq!(data["has_more"], false);
    }
}
