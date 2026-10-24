//! Golden JSON for every `CacheError` code, exit codes, and the backend mapping table
//! (REQ-BDC-0802, 0803); reply shapes pinned for wave-2 sprints.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::collections::BTreeMap;

use bd_cache_model::{
    CacheError, CallKind, DrainReport, EscalatedItem, GuardKey, PhaseName, PreconditionReason,
    ReadyBasis, ScopeId, Seq, SyncCause, Unmet,
};
use sc_beads::testing::id;
use sc_beads::{BeadsCacheError, BeadsError, CheckStep, CliClient, Refusal, Timestamp};
use serde_json::json;

fn every_error() -> Vec<CacheError> {
    let d = |s: &str| s.to_owned();
    vec![
        CacheError::BdUnavailable {
            detail: d("bd list: timed out"),
            source: None,
        },
        CacheError::NoDatabase {
            detail: d("no metadata.json"),
        },
        CacheError::WrongDatabase {
            detail: d("project p-2, file p-1"),
        },
        CacheError::NotFound {
            detail: d("nope is not in the cache or in bd"),
        },
        CacheError::BdVersionFailed {
            detail: d("exit 1"),
            source: None,
        },
        CacheError::BdSchemaFailed {
            detail: d("exit 1"),
            source: None,
        },
        CacheError::BdContract {
            bead: Some(id("b2")),
            field: d("status"),
            detail: d("\"nope\" is not one of open"),
            source: None,
        },
        CacheError::BdWriteFailed {
            detail: d("exit 1"),
            source: None,
        },
        CacheError::BdListFailed {
            detail: d("exit 1"),
            source: None,
        },
        CacheError::SyncFailed {
            detail: d("BD_CACHE.BD_CONTRACT: b2: status"),
            scope: Some(ScopeId::phase("x").unwrap()),
            cause: Some(SyncCause::BdContract),
            source: None,
        },
        CacheError::Precondition {
            detail: d("b1 no longer matches the state the write assumed"),
            unmet: BTreeMap::from([(
                GuardKey::Status,
                Unmet {
                    expected: d("open"),
                    actual: d("in_progress"),
                },
            )]),
            reason: PreconditionReason::PreconditionFailed,
            source: None,
        },
        CacheError::IdemConflict {
            detail: d("idem_key k1 already journals a different write"),
            seq: Seq::new(1),
        },
        CacheError::BadOp {
            detail: d("unknown op \"delete\""),
        },
        CacheError::BadArgs {
            detail: d("dep_add takes depends_on and type"),
        },
        CacheError::BadPrecondition {
            detail: d("unknown precondition key"),
        },
        CacheError::SchemaVersion {
            detail: d("cache schema 99, this code is 3"),
        },
        CacheError::PruneBlocked {
            detail: d("phase x has 1 pending or escalated writes"),
        },
        CacheError::NotEscalated {
            detail: d("journal entry 1 is pending"),
        },
        CacheError::ScopeUnknown {
            detail: d("phase zz is not a known scope"),
            known: vec![ScopeId::phase("x").unwrap(), ScopeId::phase("y").unwrap()],
        },
    ]
}

#[test]
fn golden_json_for_every_code() {
    let goldens = [
        json!({"ok": false, "code": "BD_CACHE.BD_UNAVAILABLE", "detail": "bd list: timed out",
               "fix": "transient backend outage: the drain retries without counting an attempt; a timed-out write retries"}),
        json!({"ok": false, "code": "BD_CACHE.NO_DATABASE", "detail": "no metadata.json",
               "fix": "run from the beads repository root (or set BEADS_DIR) after bd init"}),
        json!({"ok": false, "code": "BD_CACHE.WRONG_DATABASE", "detail": "project p-2, file p-1",
               "fix": "move the cache file aside; it was built from a different dolt database"}),
        json!({"ok": false, "code": "BD_CACHE.NOT_FOUND", "detail": "nope is not in the cache or in bd",
               "fix": "check the bead id"}),
        json!({"ok": false, "code": "BD_CACHE.BD_VERSION_FAILED", "detail": "exit 1",
               "fix": "check that bd is installed and on PATH"}),
        json!({"ok": false, "code": "BD_CACHE.BD_SCHEMA_FAILED", "detail": "exit 1",
               "fix": "transient: retried by the next run"}),
        json!({"ok": false, "code": "BD_CACHE.BD_CONTRACT", "detail": "\"nope\" is not one of open",
               "fix": "bd output no longer matches the strict model; regenerate sc-beads' rows from `bd schema`",
               "bead": "b2", "field": "status"}),
        json!({"ok": false, "code": "BD_CACHE.BD_WRITE_FAILED", "detail": "exit 1",
               "fix": "transient: retried by the next drain"}),
        json!({"ok": false, "code": "BD_CACHE.BD_LIST_FAILED", "detail": "exit 1",
               "fix": "transient: retried by the next run"}),
        json!({"ok": false, "code": "BD_CACHE.SYNC_FAILED", "detail": "BD_CACHE.BD_CONTRACT: b2: status",
               "fix": "retried by the next sync; see cause", "phase": "x", "cause": "BD_CACHE.BD_CONTRACT"}),
        json!({"ok": false, "code": "BD_CACHE.PRECONDITION", "detail": "b1 no longer matches the state the write assumed",
               "fix": "re-read the bead and decide again", "reason": "precondition_failed",
               "unmet": {"status": {"expected": "open", "actual": "in_progress"}}}),
        json!({"ok": false, "code": "BD_CACHE.IDEM_CONFLICT", "detail": "idem_key k1 already journals a different write",
               "fix": "use a key derived from the whole operation", "seq": 1}),
        json!({"ok": false, "code": "BD_CACHE.BAD_OP", "detail": "unknown op \"delete\"",
               "fix": "use one of close, update, dep_add"}),
        json!({"ok": false, "code": "BD_CACHE.BAD_ARGS", "detail": "dep_add takes depends_on and type",
               "fix": "fix the args named in detail"}),
        json!({"ok": false, "code": "BD_CACHE.BAD_PRECONDITION", "detail": "unknown precondition key",
               "fix": "precondition keys must be status and assignee"}),
        json!({"ok": false, "code": "BD_CACHE.SCHEMA_VERSION", "detail": "cache schema 99, this code is 3",
               "fix": "upgrade bd-cache, or move the file aside to rebuild it from bd"}),
        json!({"ok": false, "code": "BD_CACHE.PRUNE_BLOCKED", "detail": "phase x has 1 pending or escalated writes",
               "fix": "drain, or resolve the escalated entries with the lead, then prune"}),
        json!({"ok": false, "code": "BD_CACHE.NOT_ESCALATED", "detail": "journal entry 1 is pending",
               "fix": "resolve only entries listed as escalated in status"}),
        json!({"ok": false, "code": "BD_CACHE.SCOPE_UNKNOWN", "detail": "phase zz is not a known scope",
               "fix": "sync the scope first, or use one of the known scopes", "known": ["x", "y"]}),
    ];
    let errors = every_error();
    assert_eq!(errors.len(), goldens.len());
    for (e, golden) in errors.iter().zip(goldens) {
        assert_eq!(e.to_json(), golden, "{}", e.code());
        assert_eq!(e.exit_code(), 3, "{}", e.code());
        assert_eq!(e.to_string(), format!("{}: {}", e.code(), e.detail()));
    }
    let codes: std::collections::BTreeSet<_> = errors.iter().map(CacheError::code).collect();
    assert_eq!(codes.len(), 19, "one variant per code");
    assert!(
        !codes.contains("BD_CACHE.NO_TOML"),
        "NO_TOML is never emitted"
    );
}

#[test]
fn reasons_use_v0_names() {
    for (reason, name) in [
        (
            PreconditionReason::PreconditionFailed,
            "precondition_failed",
        ),
        (PreconditionReason::NotClosable, "not_closable"),
        (PreconditionReason::AlreadyClaimed, "already_claimed"),
    ] {
        let e = CacheError::Precondition {
            detail: String::new(),
            unmet: BTreeMap::new(),
            reason,
            source: None,
        };
        assert_eq!(e.reason(), Some(name));
        assert_eq!(e.to_json()["reason"], name);
    }
    assert_eq!(
        CacheError::BadOp {
            detail: String::new()
        }
        .reason(),
        None
    );
}

const KINDS: [CallKind; 8] = [
    CallKind::Check,
    CallKind::PreRead,
    CallKind::Write,
    CallKind::ReadBack,
    CallKind::Sync,
    CallKind::Refresh,
    CallKind::MissFill,
    CallKind::Ready,
];

fn failed_code(kind: CallKind) -> &'static str {
    match kind {
        CallKind::Write => "BD_CACHE.BD_WRITE_FAILED",
        CallKind::Sync => "BD_CACHE.SYNC_FAILED",
        CallKind::Check => "BD_CACHE.BD_VERSION_FAILED",
        _ => "BD_CACHE.BD_LIST_FAILED",
    }
}

/// The mapping table, for every `CallKind`.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one assertion per row of the mapping table"
)]
fn from_backend_matches_the_table_for_every_call_kind() {
    for kind in KINDS {
        let map = |e: &BeadsCacheError| CacheError::from_backend(e, kind);
        let contract = BeadsCacheError::Contract {
            bead: Some(id("b2")),
            field: "status".into(),
            detail: "bad".into(),
        };
        match map(&contract) {
            CacheError::BdContract {
                bead,
                field,
                detail,
                source,
            } => {
                assert_eq!(
                    (bead, field.as_str(), detail.as_str()),
                    (Some(id("b2")), "status", "bad")
                );
                assert_eq!(
                    source.unwrap().error().code(),
                    "SC_BEADS_CONTRACT",
                    "{kind:?}"
                );
            }
            other => panic!("{kind:?}: {other:?}"),
        }
        assert_eq!(
            map(&BeadsCacheError::Refused(Refusal::Precondition)).code(),
            "BD_CACHE.PRECONDITION"
        );
        assert_eq!(
            map(&BeadsCacheError::Refused(Refusal::Precondition)).reason(),
            Some("precondition_failed")
        );
        assert_eq!(
            map(&BeadsCacheError::Refused(Refusal::NotClosable)).reason(),
            Some("not_closable")
        );
        assert_eq!(
            map(&BeadsCacheError::Refused(Refusal::AlreadyClaimed)).reason(),
            Some("already_claimed")
        );
        for timed_out in [true, false] {
            let u = BeadsCacheError::Unavailable {
                reason: "slow".into(),
                timed_out,
            };
            let e = map(&u);
            assert_eq!(
                (e.code(), e.detail()),
                ("BD_CACHE.BD_UNAVAILABLE", "slow"),
                "{kind:?}"
            );
        }
        let failed = BeadsCacheError::Failed {
            code: 1,
            stderr: "boom".into(),
        };
        assert_eq!(map(&failed).code(), failed_code(kind), "{kind:?}");
        assert_eq!(
            map(&BeadsCacheError::Refused(Refusal::Other("E".into()))).code(),
            failed_code(kind)
        );
        let spawn = BeadsCacheError::Btit(BeadsError::Spawn {
            binary: "bd".into(),
            operation: None,
            source: std::io::Error::other("missing"),
        });
        assert_eq!(map(&spawn).code(), "BD_CACHE.BD_UNAVAILABLE");
        let bad_json = BeadsCacheError::Btit(BeadsError::UnexpectedShape {
            context: "list".into(),
            expected: sc_beads::ExpectedShape::Array,
        });
        assert_eq!(map(&bad_json).code(), "BD_CACHE.BD_CONTRACT");
        let unsupported = BeadsCacheError::Btit(BeadsError::Unsupported {
            operation: "x",
            client: CliClient::Br,
        });
        assert_eq!(map(&unsupported).code(), failed_code(kind));
    }
    let step = |step| BeadsCacheError::CheckFailed {
        step,
        code: 1,
        stderr: "boom".into(),
    };
    assert_eq!(
        CacheError::from_backend(&step(CheckStep::Schema), CallKind::Check).code(),
        "BD_CACHE.BD_SCHEMA_FAILED"
    );
    assert_eq!(
        CacheError::from_backend(&step(CheckStep::Version), CallKind::Check).code(),
        "BD_CACHE.BD_VERSION_FAILED"
    );
    // The step is typed: a Failed whose stderr names `schema` is still BD_VERSION_FAILED.
    let named = BeadsCacheError::Failed {
        code: 1,
        stderr: "schema: exit 1".into(),
    };
    assert_eq!(
        CacheError::from_backend(&named, CallKind::Check).code(),
        "BD_CACHE.BD_VERSION_FAILED"
    );
    let sync = CacheError::from_backend(
        &BeadsCacheError::Failed {
            code: 1,
            stderr: String::new(),
        },
        CallKind::Sync,
    );
    assert_eq!(sync.to_json()["cause"], "BD_CACHE.BD_LIST_FAILED");
}

#[test]
fn call_kind_classes() {
    let lists: Vec<_> = KINDS.iter().filter(|k| k.is_list()).collect();
    assert_eq!(
        lists.len(),
        6,
        "every kind but Check and Write proves bd answers lists"
    );
    assert!(
        !CallKind::Write.unavailable_is_outage() && !CallKind::ReadBack.unavailable_is_outage()
    );
    assert!(CallKind::Check.unavailable_is_outage() && CallKind::Ready.unavailable_is_outage());
}

/// Reply shapes: rows are objects; basis is tagged; reports list seqs.
#[test]
fn reply_goldens() {
    let report = DrainReport {
        applied: vec![Seq::new(1)],
        escalated: vec![EscalatedItem {
            seq: Seq::new(2),
            bead: id("b1"),
            op: "update".into(),
            actor: sc_beads::testing::actor("san"),
            phase: Some(PhaseName::new("x").unwrap()),
            reason: "bd refused: precondition changed (exit 13)".into(),
        }],
        deferred: vec![Seq::new(3)],
        ..DrainReport::default()
    };
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        json!({"busy": false, "applied": [1], "retrying": [], "deferred": [3], "deferred_in_flight": [],
               "outage": null,
               "escalated": [{"seq": 2, "bead": "b1", "op": "update", "actor": "san", "phase": "x",
                              "reason": "bd refused: precondition changed (exit 13)"}]})
    );
    assert_eq!(
        serde_json::to_value(ReadyBasis::Fresh {
            fetched_at: Timestamp::parse("2026-10-23T10:00:05Z").unwrap()
        })
        .unwrap(),
        json!({"kind": "fresh", "fetched_at": "2026-10-23T10:00:05Z"})
    );
    // a-1 amendment 2: ready replies carry the scope's last_ok_at and the basis' fetch time.
    let reply = bd_cache_model::ReadyReply {
        scope: ScopeId::phase("x").unwrap(),
        last_ok_at: Some(Timestamp::parse("2026-10-23T10:00:00Z").unwrap()),
        basis: ReadyBasis::Fresh {
            fetched_at: Timestamp::parse("2026-10-23T10:00:05Z").unwrap(),
        },
        locally_adjusted: true,
        ready: vec![],
        read_fallback: None,
        missing_rows: vec![id("b9")],
    };
    assert_eq!(
        serde_json::to_value(&reply).unwrap(),
        json!({"scope": "x", "last_ok_at": "2026-10-23T10:00:00Z",
               "basis": {"kind": "fresh", "fetched_at": "2026-10-23T10:00:05Z"},
               "locally_adjusted": true, "ready": [], "read_fallback": null, "missing_rows": ["b9"]})
    );
    assert_eq!(
        serde_json::to_value(ReadyBasis::Cached {
            fetched_at: Timestamp::parse("2026-10-23T10:00:00Z").unwrap(),
            stale: true
        })
        .unwrap(),
        json!({"kind": "cached", "fetched_at": "2026-10-23T10:00:00Z", "stale": true})
    );
    assert_eq!(
        serde_json::to_value(ReadyBasis::Unknown).unwrap(),
        json!({"kind": "unknown"})
    );
    let view = bd_cache_model::RowView {
        bead: sc_beads::testing::bead("b1"),
        stale: false,
        blocked: Some(vec![]),
    };
    let v = serde_json::to_value(&view).unwrap();
    assert!(v["bead"].is_object() && v["blocked"] == json!([]) && v["stale"] == json!(false));
}

/// A mapped error keeps the backend failure as its source and in its JSON.
#[test]
fn mapped_errors_keep_their_source_chain() {
    use std::error::Error as _;
    let e = CacheError::from_backend(
        &BeadsCacheError::Failed {
            code: 2,
            stderr: "connection reset".into(),
        },
        CallKind::Write,
    );
    assert_eq!(e.code(), "BD_CACHE.BD_WRITE_FAILED");
    let src = e.source().expect("a mapped error has a source");
    assert_eq!(src.to_string(), "exit 2: connection reset");
    assert_eq!(
        e.to_json()["source"],
        json!({"code": "SC_BEADS_FAILED", "detail": "exit 2: connection reset"})
    );
    let direct = CacheError::BadArgs { detail: "x".into() };
    assert!(direct.source().is_none() && direct.to_json().get("source").is_none());
}
