//! The op table: parse, canonical args, overlay, achieved, replay (REQ-BDC-0301, 0303, 0304, 0305).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use bd_cache_model::{
    GuardKey, IdemKey, JournalEntry, JournalState, Op, ScopeId, Seq, normalize, parse_precondition,
    precondition_json, replay, unmet,
};
use sc_beads::testing::{bead, edge, id};
use sc_beads::{Bead, DepType, Guards, MetaValue, Patch};

const PY_ARGS: &str = include_str!("fixtures/python-journal-args.json");

fn code(op: &str, args: &str) -> &'static str {
    Op::parse(op, args).map_or_else(|e| e.code(), |_| "ok")
}

#[test]
fn test_bad_ops_are_typed() {
    // The Python suite's cases.
    assert_eq!(code("delete", "{}"), "BD_CACHE.BAD_OP");
    for (op, args) in [
        ("update", "{}"),
        ("dep_add", r#"{"type": "blocks"}"#),
        ("dep_add", r#"{"depends_on": "b2", "type": "likes"}"#),
        ("update", r#"{"set_metadata": {"k": "5"}}"#),
        ("update", r#"{"set_metadata": {"k": "true"}}"#),
        ("update", r#"{"set_metadata": {"k": " x"}}"#),
        ("update", r#"{"set_metadata": {"k": 1.5}}"#),
        ("update", r#"{"set_metadata": {"k": null}}"#),
    ] {
        assert_eq!(code(op, args), "BD_CACHE.BAD_ARGS", "{op} {args}");
    }
}

/// Every class of metadata value bd would coerce is refused; the round-tripping ones pass.
#[test]
fn metadata_coercion_guard() {
    let rejected = [
        r#""5""#,
        r#""-3""#,
        r#""5.0""#,
        r#""1e3""#,
        r#""0""#,
        r#""true""#,
        r#""false""#,
        r#""null""#,
        r#""NaN""#,
        r#""Infinity""#,
        r#""-Infinity""#,
        r#"" 7""#,
        r#""x ""#,
        r#""\tx""#,
        "1.5",
        "null",
        "[1]",
        r#"{"x": 1}"#,
        "1e400",
        "18446744073709551616",
    ];
    for v in rejected {
        let args = format!(r#"{{"set_metadata": {{"k": {v}}}}}"#);
        assert_eq!(
            code("update", &args),
            "BD_CACHE.BAD_ARGS",
            "{v} must be refused"
        );
    }
    let accepted = [
        "5",
        "-3",
        "true",
        "false",
        r#""abc123""#,
        r#""[1]""#,
        r#""{\"x\":1}""#,
        r#""\"5\"""#,
        r#""True""#,
        r#""01""#,
        r#""5x""#,
        r#""""#,
        r#""deadbeef""#,
    ];
    for v in accepted {
        let args = format!(r#"{{"set_metadata": {{"k": {v}}}}}"#);
        assert_eq!(code("update", &args), "ok", "{v} must be accepted");
    }
    let op = Op::parse(
        "update",
        r#"{"set_metadata": {"b": true, "i": 7, "s": "x"}}"#,
    )
    .unwrap();
    let Op::Update(p) = op else { panic!("update") };
    assert_eq!(p.set_metadata["b"], MetaValue::Bool(true));
    assert_eq!(p.set_metadata["i"], MetaValue::Int(7));
    assert_eq!(p.set_metadata["s"], MetaValue::Str("x".into()));
}

#[test]
fn args_shape_errors_are_bad_args() {
    for (op, args) in [
        ("close", "[]"),
        ("close", "not json"),
        ("close", r#"{"reason": 5}"#),
        ("close", r#"{"why": "x"}"#),
        ("update", r#"{"colour": "red"}"#),
        ("update", r#"{"status": 1}"#),
        ("update", r#"{"add_labels": "x"}"#),
        ("update", r#"{"add_labels": [1]}"#),
        ("update", r#"{"metadata": {"set": {"k": "5"}}}"#),
        ("update", r#"{"metadata": {"replace": {}}}"#),
        (
            "update",
            r#"{"set_metadata": {"k": 1}, "metadata": {"set": {"k": 2}}}"#,
        ),
        ("update", r#"{"add_labels": []}"#),
        ("dep_add", r#"{"depends_on": "", "type": "blocks"}"#),
        (
            "dep_add",
            r#"{"depends_on": "b1", "type": "blocks", "x": 1}"#,
        ),
    ] {
        assert_eq!(code(op, args), "BD_CACHE.BAD_ARGS", "{op} {args}");
    }
    for t in DepType::ALL {
        let args = format!(r#"{{"depends_on": "b1", "type": "{}"}}"#, t.as_str());
        assert_eq!(code("dep_add", &args), "ok", "{t}");
    }
}

/// The /v0 metadata form is accepted and written back in the BD-CACHE form (ADR-BDC-0003).
#[test]
fn v0_metadata_form_is_canonicalized() {
    let v0 = Op::parse(
        "update",
        r#"{"metadata": {"set": {"n": 5}, "unset": ["old"]}}"#,
    )
    .unwrap();
    let bd = Op::parse(
        "update",
        r#"{"set_metadata": {"n": 5}, "unset_metadata": ["old"]}"#,
    )
    .unwrap();
    assert_eq!(v0, bd);
    assert_eq!(
        v0.to_args_json(),
        r#"{"set_metadata": {"n": 5}, "unset_metadata": ["old"]}"#
    );
}

#[test]
fn python_written_args_round_trip_byte_for_byte() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(PY_ARGS).unwrap();
    assert_eq!(cases.len(), 10);
    for c in cases {
        let (op, args) = (c["op"].as_str().unwrap(), c["args_json"].as_str().unwrap());
        let parsed = Op::parse(op, args).unwrap_or_else(|e| panic!("{op} {args}: {e}"));
        assert_eq!(parsed.name(), op);
        assert_eq!(parsed.to_args_json(), args, "{op}");
        assert_eq!(
            Op::parse(op, &parsed.to_args_json()).unwrap(),
            parsed,
            "equal ops compare equal"
        );
    }
}

#[test]
fn equal_ops_compare_equal_regardless_of_key_order() {
    let a = Op::parse(
        "update",
        r#"{"status": "open", "set_metadata": {"a": 1, "b": 2}}"#,
    )
    .unwrap();
    let b = Op::parse(
        "update",
        r#"{"set_metadata": {"b": 2, "a": 1}, "status": "open"}"#,
    )
    .unwrap();
    assert_eq!(a, b);
    assert_ne!(a, Op::parse("update", r#"{"status": "closed"}"#).unwrap());
}

fn row() -> Bead {
    let mut b = bead("b1");
    b.status = Some("in_progress".into());
    b.assignee = Some("dev".into());
    b.labels = vec!["phase-x".into(), "r1".into()];
    b.notes = Some("first".into());
    b.metadata.insert("old".into(), serde_json::json!(1));
    b
}

fn update(json: &str) -> Op {
    Op::parse("update", json).unwrap()
}

#[test]
fn close_overlay_and_achieved() {
    let op = Op::parse("close", r#"{"reason": "PASS"}"#).unwrap();
    let r = row();
    assert!(!op.achieved(&r));
    let v = op.overlay(&r);
    assert_eq!(
        (v.status.as_deref(), v.close_reason.as_deref()),
        (Some("closed"), Some("PASS"))
    );
    assert!(op.achieved(&v));
    assert_eq!(r.status.as_deref(), Some("in_progress"), "overlay is pure");
    let plain = Op::parse("close", "{}").unwrap().overlay(&r);
    assert_eq!(plain.close_reason, None);
}

#[test]
fn update_overlay_applies_fields_in_order_and_achieved_checks_each() {
    let op = update(
        r#"{"status": "open", "assignee": "", "add_labels": ["r2", "r1"], "remove_labels": ["r1"],
            "set_metadata": {"n": 5, "ok": true}, "unset_metadata": ["old"], "append_notes": "second"}"#,
    );
    let r = row();
    let v = op.overlay(&r);
    assert_eq!(v.status.as_deref(), Some("open"));
    assert_eq!(v.assignee, None, "unassigned, as bd stores it");
    // remove then add: r1 removed, then re-added by add_labels (Python order).
    assert_eq!(v.labels, ["phase-x", "r2", "r1"]);
    assert_eq!(
        serde_json::Value::Object(v.metadata.clone()),
        serde_json::json!({"n": 5, "ok": true})
    );
    assert_eq!(v.notes.as_deref(), Some("first\nsecond"));
    // achieved needs every field; r1 is both added and removed, so it never holds.
    assert!(!op.achieved(&v));
    let consistent = update(
        r#"{"status": "open", "assignee": "", "add_labels": ["r2"], "remove_labels": ["r1"],
            "set_metadata": {"n": 5, "ok": true}, "unset_metadata": ["old"], "append_notes": "second"}"#,
    );
    let v = consistent.overlay(&r);
    assert!(consistent.achieved(&v));
    assert!(!consistent.achieved(&r));
    for (field, json) in [
        ("status", r#"{"status": "closed"}"#),
        ("assignee", r#"{"assignee": "other"}"#),
        ("add", r#"{"add_labels": ["zz"]}"#),
        ("remove", r#"{"remove_labels": ["r1"]}"#),
        ("set", r#"{"set_metadata": {"old": 2}}"#),
        ("set type", r#"{"set_metadata": {"old": "one"}}"#),
        ("unset", r#"{"unset_metadata": ["old"]}"#),
        ("notes", r#"{"append_notes": "absent"}"#),
    ] {
        assert!(!update(json).achieved(&r), "{field} is not yet achieved");
        assert!(
            update(json).achieved(&update(json).overlay(&r)),
            "{field} achieved after overlay"
        );
    }
    // An unassigned bead matches assignee "".
    let mut free = row();
    free.assignee = None;
    assert!(update(r#"{"assignee": ""}"#).achieved(&free));
    // Notes on an empty bead are the text alone.
    let mut empty = row();
    empty.notes = None;
    assert_eq!(
        update(r#"{"append_notes": "x"}"#)
            .overlay(&empty)
            .notes
            .as_deref(),
        Some("x")
    );
}

#[test]
fn dep_add_overlay_adds_the_edge_once_and_achieved_sees_it() {
    let op = Op::parse("dep_add", r#"{"depends_on": "b0", "type": "blocks"}"#).unwrap();
    let r = row();
    assert!(!op.achieved(&r));
    let v = op.overlay(&r);
    assert_eq!(v.dependencies, [edge("b1", "b0", DepType::Blocks)]);
    assert!(op.achieved(&v));
    assert_eq!(op.overlay(&v).dependencies.len(), 1, "added only if absent");
    let other = Op::parse("dep_add", r#"{"depends_on": "b0", "type": "related"}"#).unwrap();
    assert!(!other.achieved(&v), "kind matters");
}

fn entry(seq: i64, state: JournalState, op: Op) -> JournalEntry {
    JournalEntry {
        seq: Seq::new(seq),
        idem_key: IdemKey::new(format!("k{seq}")).unwrap(),
        scope: Some(ScopeId::phase("x").unwrap()),
        bead: id("b1"),
        op,
        precondition: Guards::default(),
        actor: sc_beads::testing::actor("san"),
        state,
        attempts: 0,
        last_error: None,
        announced_at: None,
        created_at: sc_beads::Timestamp::parse("2026-10-23T10:00:00Z").unwrap(),
        applied_at: None,
    }
}

#[test]
fn replay_applies_pending_entries_in_seq_order_and_skips_the_rest() {
    let r = row();
    let entries = [
        entry(
            3,
            JournalState::Pending,
            update(r#"{"append_notes": "three"}"#),
        ),
        entry(
            1,
            JournalState::Pending,
            update(r#"{"append_notes": "one"}"#),
        ),
        entry(
            2,
            JournalState::Escalated,
            update(r#"{"status": "closed"}"#),
        ),
        entry(4, JournalState::Resolved, update(r#"{"assignee": "x"}"#)),
        entry(
            5,
            JournalState::Applied,
            update(r#"{"add_labels": ["applied"]}"#),
        ),
    ];
    let v = replay(&r, &entries);
    assert_eq!(v.notes.as_deref(), Some("first\none\nthree"));
    assert_eq!(
        v.status.as_deref(),
        Some("in_progress"),
        "escalated entries are not replayed"
    );
    assert_eq!(
        v.assignee.as_deref(),
        Some("dev"),
        "resolved entries are not replayed"
    );
    assert!(!v.labels.contains(&"applied".to_owned()));
    assert_eq!(replay(&r, &[]), r);
}

#[test]
fn normalize_drops_counts_and_sorts_edges() {
    let mut b = row();
    b.dependency_count = Some(2);
    b.dependent_count = Some(1);
    b.comment_count = Some(3);
    b.dependencies = vec![
        edge("b1", "z", DepType::Blocks),
        edge("b1", "a", DepType::Related),
        edge("b1", "a", DepType::Related),
        edge("other", "a", DepType::Blocks),
    ];
    let n = normalize(&b);
    assert_eq!(
        (n.dependency_count, n.dependent_count, n.comment_count),
        (None, None, Some(3))
    );
    assert_eq!(
        n.dependencies,
        [
            edge("b1", "a", DepType::Related),
            edge("b1", "z", DepType::Blocks)
        ]
    );
}

#[test]
fn preconditions_parse_canonicalize_and_report_unmet() {
    let g = parse_precondition(r#"{"status": "in_progress", "assignee": "dev"}"#).unwrap();
    assert_eq!(
        precondition_json(&g),
        r#"{"assignee": "dev", "status": "in_progress"}"#
    );
    assert_eq!(
        parse_precondition(r#"{"expected_status": "open"}"#)
            .unwrap()
            .status
            .as_deref(),
        Some("open")
    );
    assert_eq!(parse_precondition("").unwrap(), Guards::default());
    assert_eq!(precondition_json(&Guards::default()), "{}");
    for bad in [
        r#"{"owner": "x"}"#,
        r#"{"status": 1}"#,
        "[]",
        "nope",
        r#"{"status": "a", "expected_status": "b"}"#,
    ] {
        assert_eq!(
            parse_precondition(bad).unwrap_err().code(),
            "BD_CACHE.BAD_PRECONDITION",
            "{bad}"
        );
    }
    let r = row();
    assert!(unmet(&r, &g).is_empty());
    let miss = unmet(
        &r,
        &parse_precondition(r#"{"status": "open", "assignee": ""}"#).unwrap(),
    );
    assert_eq!(miss[&GuardKey::Status].expected, "open");
    assert_eq!(miss[&GuardKey::Status].actual, "in_progress");
    assert_eq!(miss[&GuardKey::Assignee].actual, "dev");
    let mut free = row();
    free.assignee = None;
    assert!(
        unmet(
            &free,
            &Guards {
                status: None,
                assignee: Some(sc_beads::Assignee::Unassigned)
            }
        )
        .is_empty()
    );
}

#[test]
fn patch_args_value_omits_empty_fields() {
    let op = Op::Update(Patch {
        status: Some("open".into()),
        ..Patch::default()
    });
    assert_eq!(op.to_args_json(), r#"{"status": "open"}"#);
}
