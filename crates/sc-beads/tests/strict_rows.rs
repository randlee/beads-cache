//! The bd-1.3.1 contract fixtures against the strict model (REQ-BDC-0007).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use sc_beads::{
    parse_bd_row, parse_bd_rows, schema_sha256, BeadsCacheError, DepType, BD_SCHEMA_SHA256,
    BD_SCHEMA_VERSION,
};

const SCHEMA_TEXT: &str = include_str!("fixtures/bd-1.3.1/bd-schema.json");
const LIST_ROWS: &str = include_str!("fixtures/bd-1.3.1/list-rows.json");

#[test]
fn test_pinned_model_matches_recorded_schema() {
    assert_eq!(schema_sha256(SCHEMA_TEXT).as_str(), BD_SCHEMA_SHA256);
    let schema: serde_json::Value = serde_json::from_str(SCHEMA_TEXT).unwrap();
    assert_eq!(schema["schema_version"].to_string(), BD_SCHEMA_VERSION);
}

#[test]
fn test_recorded_bd_rows_validate() {
    let rows = parse_bd_rows(LIST_ROWS).unwrap();
    let mut ids: Vec<_> = rows.iter().map(|r| r.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["ct-1", "ct-2", "ct-3"]);
    for row in &rows {
        assert_eq!(row.is_blocked, None, "bd never emits is_blocked");
    }
    let ct2 = rows.iter().find(|r| r.id.as_str() == "ct-2").unwrap();
    assert_eq!(
        ct2.dependencies,
        vec![sc_beads::testing::edge("ct-2", "ct-1", DepType::Blocks)]
    );
    assert_eq!(ct2.metadata["n"], serde_json::json!(42));
    let ct3 = rows.iter().find(|r| r.id.as_str() == "ct-3").unwrap();
    assert_eq!(ct3.close_reason.as_deref(), Some("PASS at abc123"));
    assert!(ct3.dependencies.is_empty() && ct3.labels == ["phase-ct"]);
}

/// The three BD-CACHE v3 contract violations, plus a missing key and a wrong type.
#[test]
fn a_bad_row_is_a_contract_error_naming_bead_and_field() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(LIST_ROWS).unwrap();
    let cases: [(&str, serde_json::Value, &str); 5] = [
        ("surprise", serde_json::json!(1), "surprise"),
        ("status", serde_json::json!("nope"), "status"),
        ("created_at", serde_json::json!("yesterday"), "created_at"),
        ("priority", serde_json::json!("high"), "priority"),
        ("title", serde_json::Value::Null, "title"),
    ];
    for (key, value, field) in cases {
        let mut row = rows[0].clone();
        row[key] = value;
        match parse_bd_row(&row) {
            Err(BeadsCacheError::Contract { bead, field: f, .. }) => {
                assert_eq!(bead.as_ref().map(sc_beads::BeadId::as_str), Some("ct-2"));
                assert_eq!(f, field);
            }
            other => panic!("{key}: expected Contract, got {other:?}"),
        }
    }
    let mut row = rows[0].clone();
    row.as_object_mut().unwrap().remove("updated_at");
    assert!(matches!(
        parse_bd_row(&row),
        Err(BeadsCacheError::Contract { field, .. }) if field == "updated_at"
    ));
    // One bad row fails the whole list (REQ-BDC-0008).
    let mut all = rows.clone();
    all[2]["surprise"] = serde_json::json!(true);
    assert!(parse_bd_rows(&serde_json::to_string(&all).unwrap()).is_err());
}

/// Stored rows round-trip through serde (the store writes and reads this form).
#[test]
fn stored_form_round_trips() {
    for row in parse_bd_rows(LIST_ROWS).unwrap() {
        let text = serde_json::to_string(&row).unwrap();
        let back: sc_beads::Bead = serde_json::from_str(&text).unwrap();
        assert_eq!(back, row);
    }
}
