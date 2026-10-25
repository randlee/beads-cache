//! REQ-BDC-0803, contract §4: [`ERROR_TABLE`] equals the contract's error
//! table, every `CacheError` code is in it, and `From<CacheError>` maps the
//! core's fields as the contract's mapping table says.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use bd_cache_api::{ApiError, ERROR_TABLE, ErrorKind, spec};
use bd_cache_model::{
    BackendCause, CacheError, GuardKey, PreconditionReason, ScopeId, Seq, StoreCause, SyncCause,
    Unmet,
};
use sc_beads::{BeadId, BeadsCacheError};
use serde_json::json;

/// One parsed row: kind, exit, retryable, `v0_code`.
type Row = (String, Option<i32>, bool, Option<String>);

/// The cells of the `### Error table` section of the contract, row by row.
fn contract_rows() -> Vec<Vec<String>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/bd-cache-cli/contract.md");
    let doc = std::fs::read_to_string(&path).unwrap();
    let section = doc
        .split_once("### Error table")
        .expect("the contract has an error table")
        .1;
    section
        .lines()
        .skip_while(|l| !l.starts_with("| Code"))
        .skip(2) // header and separator
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            l.trim()
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_owned())
                .collect()
        })
        .collect()
}

/// The backticked words of a cell.
fn ticked(cell: &str) -> Vec<String> {
    cell.split('`')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_owned())
        .collect()
}

/// The contract's table as `code -> row`, and its `TICK_*` row's kind cell.
fn parse_contract() -> (BTreeMap<String, Row>, String) {
    let mut table = BTreeMap::new();
    let mut tick = None;
    for cells in contract_rows() {
        let [code, kind, exit, retryable, v0, ..] = cells.as_slice() else {
            panic!("short row {cells:?}");
        };
        let codes = ticked(code);
        if codes.iter().any(|c| c.starts_with("TICK_")) {
            tick = Some(kind.clone());
            continue;
        }
        let kind = kind.replace("**", "");
        let exit = match exit.as_str() {
            "—" => None,
            n => Some(
                n.parse::<i32>()
                    .unwrap_or_else(|e| panic!("{code}: exit {n}: {e}")),
            ),
        };
        let retryable = match retryable.replace("**", "").as_str() {
            "yes" => true,
            "no" => false,
            other => panic!("{code}: retryable {other:?}"),
        };
        let v0 = ticked(v0).into_iter().next();
        for c in codes {
            let prev = table.insert(c.clone(), (kind.clone(), exit, retryable, v0.clone()));
            assert!(prev.is_none(), "{c} is listed twice");
        }
    }
    (table, tick.expect("the contract has a TICK_<REFUSAL> row"))
}

fn ours() -> BTreeMap<String, Row> {
    ERROR_TABLE
        .iter()
        .map(|s| {
            (
                s.code.to_owned(),
                (
                    s.kind.as_str().to_owned(),
                    s.exit,
                    s.retryable,
                    s.v0_code.map(str::to_owned),
                ),
            )
        })
        .collect()
}

#[test]
fn the_error_table_equals_the_contract() {
    let (contract, _) = parse_contract();
    assert!(contract.len() >= 25, "parsed only {} codes", contract.len());
    let ours = ours();
    assert_eq!(ours.len(), ERROR_TABLE.len(), "a code is listed twice");
    let theirs_codes: BTreeSet<_> = contract.keys().collect();
    let our_codes: BTreeSet<_> = ours.keys().collect();
    assert_eq!(our_codes, theirs_codes, "the codes differ");
    for (code, row) in &contract {
        assert_eq!(
            ours.get(code),
            Some(row),
            "{code}: ERROR_TABLE differs from the contract"
        );
    }
}

#[test]
fn tick_refusals_take_the_kinds_the_contract_names() {
    let (_, tick_kinds) = parse_contract();
    let cases = [
        ("missing_team", ErrorKind::InvalidInput),
        ("phase_file_invalid", ErrorKind::InvalidInput),
        ("escalation_undelivered", ErrorKind::Unavailable),
        ("escalation_no_lead", ErrorKind::Unavailable),
        ("busy", ErrorKind::Conflict),
    ];
    for (refusal, kind) in cases {
        assert!(
            tick_kinds.contains(kind.as_str()),
            "the contract names {kind}"
        );
        let e = ApiError::tick_refusal(refusal, "m");
        assert_eq!(e.kind(), kind, "{refusal}");
        assert_eq!(
            e.code(),
            format!("BD_CACHE.TICK_{}", refusal.to_uppercase())
        );
        assert_eq!(e.exit_code(), None);
        assert_eq!(e.retryable(), kind == ErrorKind::Unavailable);
    }
}

fn backend_unavailable(timed_out: bool) -> BackendCause {
    BackendCause::new(BeadsCacheError::Unavailable {
        reason: "bd timed out".into(),
        timed_out,
    })
}

/// One error of every `CacheError` variant.
fn every_cache_error() -> Vec<CacheError> {
    let d = || "d".to_owned();
    vec![
        CacheError::BdUnavailable {
            detail: d(),
            source: None,
        },
        CacheError::NoDatabase {
            detail: d(),
            source: None,
        },
        CacheError::WrongDatabase { detail: d() },
        CacheError::NotFound { detail: d() },
        CacheError::BdVersionFailed {
            detail: d(),
            source: None,
        },
        CacheError::BdSchemaFailed {
            detail: d(),
            source: None,
        },
        CacheError::BdContract {
            bead: None,
            field: String::new(),
            detail: d(),
            source: None,
        },
        CacheError::BdWriteFailed {
            detail: d(),
            source: None,
        },
        CacheError::BdListFailed {
            detail: d(),
            source: None,
        },
        CacheError::SyncFailed {
            detail: d(),
            scope: None,
            cause: None,
            source: None,
        },
        CacheError::Precondition {
            detail: d(),
            unmet: BTreeMap::new(),
            reason: PreconditionReason::PreconditionFailed,
            source: None,
        },
        CacheError::IdemConflict {
            detail: d(),
            seq: Seq::new(1),
        },
        CacheError::BadOp { detail: d() },
        CacheError::BadArgs { detail: d() },
        CacheError::BadPrecondition { detail: d() },
        CacheError::SchemaVersion { detail: d() },
        CacheError::PruneBlocked { detail: d() },
        CacheError::NotEscalated { detail: d() },
        CacheError::ScopeUnknown {
            detail: d(),
            known: Vec::new(),
        },
        CacheError::StoreFailed {
            detail: d(),
            source: None,
        },
        CacheError::StoreInUse { detail: d() },
    ]
}

#[test]
fn every_cache_error_code_is_in_the_table_with_its_kind() {
    let errors = every_cache_error();
    let codes: BTreeSet<&str> = errors.iter().map(CacheError::code).collect();
    assert_eq!(codes.len(), 21, "one error per variant");
    for e in &errors {
        let s = spec(e.code()).unwrap_or_else(|| panic!("{} is not in ERROR_TABLE", e.code()));
        let a = ApiError::from(e);
        assert_eq!(a.code(), e.code(), "the code passes unchanged");
        assert_eq!(a.kind(), s.kind, "{}", e.code());
        assert_eq!(a.message(), e.detail());
        assert_eq!(a.suggested_action(), Some(e.fix()));
        assert_eq!(a.retryable(), s.retryable);
    }
}

#[test]
fn reason_unmet_and_phase_map_to_details() {
    let mut unmet = BTreeMap::new();
    unmet.insert(
        GuardKey::Status,
        Unmet {
            expected: "open".into(),
            actual: "closed".into(),
        },
    );
    let e = ApiError::from(CacheError::Precondition {
        detail: "d".into(),
        unmet,
        reason: PreconditionReason::NotClosable,
        source: None,
    });
    assert_eq!(e.details()["v0_code"], "not_closable");
    assert_eq!(
        e.details()["unmet"],
        json!({"status": {"expected": "open", "actual": "closed"}})
    );

    let e = ApiError::from(CacheError::SyncFailed {
        detail: "d".into(),
        scope: ScopeId::phase("bk").ok(),
        cause: Some(SyncCause::BdListFailed),
        source: None,
    });
    assert_eq!(
        e.details()["scope"],
        "phase-bk",
        "phase becomes scope, in label form"
    );
    assert!(e.details().get("phase").is_none());
    assert_eq!(e.details()["cause"], "BD_CACHE.BD_LIST_FAILED");
    assert_eq!(e.details()["v0_code"], "db_unavailable");

    let e = ApiError::from(CacheError::ScopeUnknown {
        detail: "d".into(),
        known: vec![ScopeId::phase("a").unwrap()],
    });
    assert_eq!(e.details()["known"], json!(["phase-a"]));
}

#[test]
fn the_other_payload_fields_keep_their_names() {
    let e = ApiError::from(CacheError::IdemConflict {
        detail: "d".into(),
        seq: Seq::new(7),
    });
    assert_eq!(e.details()["seq"], 7);

    let e = ApiError::from(CacheError::BdContract {
        bead: BeadId::new("b1").ok(),
        field: "status".into(),
        detail: "d".into(),
        source: None,
    });
    assert_eq!(
        (&e.details()["bead"], &e.details()["field"]),
        (&json!("b1"), &json!("status"))
    );

    let e = ApiError::from(CacheError::BdUnavailable {
        detail: "d".into(),
        source: Some(backend_unavailable(false)),
    });
    assert_eq!(e.details()["timed_out"], false);
    assert_eq!(e.details()["source"]["code"], "SC_BEADS_UNAVAILABLE");
    let cut = ApiError::from(CacheError::BdUnavailable {
        detail: "d".into(),
        source: None,
    });
    assert_eq!(
        cut.details()["timed_out"],
        true,
        "a cut has no backend source"
    );

    let e = ApiError::from(CacheError::StoreFailed {
        detail: "d".into(),
        source: Some(StoreCause::new(std::io::Error::other("disk"))),
    });
    assert_eq!(e.details()["source"], json!({"detail": "disk"}));
    assert_eq!(e.details()["v0_code"], "internal");
}

#[test]
fn the_error_serializes_as_the_envelope_error() {
    let e = ApiError::unsupported(bd_cache_api::Unsupported::Member, "claim");
    let v = serde_json::to_value(&e).unwrap();
    let keys: BTreeSet<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        BTreeSet::from(["kind", "code", "message", "details", "suggested_action"])
    );
    assert_eq!(v, e.to_json());
    assert_eq!(v["kind"], "invalid_input");
}
