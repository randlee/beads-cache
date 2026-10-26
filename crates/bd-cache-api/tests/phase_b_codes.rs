//! The phase-B codes (REQ-BDC-0729, ADR-BDC-0015; AC8): each `CacheError` maps to the
//! `ERROR_TABLE` row of its code, with REQ-BDC-0729's kind, exit, retryable flag and
//! `details` keys, so the model's and the api's tables agree; and every error that can
//! come from bd carries `details.outage`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

use bd_cache_api::{ApiError, ErrorKind, spec};
use bd_cache_model::{CacheError, CallKind, IngestReason, LeaseHolder, ScopeId, TickRunReason};
use sc_beads::{BeadsCacheError, Timestamp};
use serde_json::json;

fn holder() -> LeaseHolder {
    LeaseHolder {
        run_id: "r0".into(),
        pid: 7,
        host: "h".into(),
        expires_at: Timestamp::parse("2026-10-23T10:00:00Z").unwrap(),
    }
}

#[test]
fn the_four_codes_take_their_rows_and_details() {
    let cases = [
        (
            CacheError::IngestInvalid {
                detail: "d".into(),
                scope: ScopeId::new("lane").unwrap(),
                reason: IngestReason::RowInvalid,
                index: Some(2),
            },
            ErrorKind::InvalidInput,
            false,
            json!({"scope": "lane", "reason": "row_invalid", "index": 2}),
        ),
        (
            CacheError::ScheduleInvalid {
                detail: "d".into(),
                field: "python".into(),
                reason: "not_absolute".into(),
            },
            ErrorKind::InvalidInput,
            false,
            json!({"field": "python", "reason": "not_absolute"}),
        ),
        (
            CacheError::TickRunFailed {
                detail: "d".into(),
                row: "python".into(),
                run_id: "r1".into(),
                reason: TickRunReason::Exit,
                exit_status: Some(1),
                stderr: "boom".into(),
                restored: Some(true),
            },
            ErrorKind::Unavailable,
            false,
            json!({"row": "python", "run_id": "r1", "reason": "exit", "exit_status": 1,
                   "stderr": "boom", "restored": true}),
        ),
        (
            CacheError::TickInFlight {
                detail: "d".into(),
                holder: holder(),
            },
            ErrorKind::Conflict,
            true,
            json!({"holder": {"pid": 7, "host": "h", "expires_at": "2026-10-23T10:00:00Z"}}),
        ),
    ];
    for (e, kind, retryable, details) in cases {
        let row = spec(e.code()).unwrap_or_else(|| panic!("{} has a table row", e.code()));
        let api = ApiError::from(&e);
        assert_eq!(api.code(), e.code());
        assert_eq!((api.kind(), row.kind), (kind, kind), "{}", e.code());
        assert_eq!(api.exit_code(), Some(3), "{}", e.code());
        assert_eq!(api.retryable(), retryable, "{}", e.code());
        for (key, want) in details.as_object().unwrap() {
            assert_eq!(api.details().get(key), Some(want), "{}: {key}", e.code());
        }
        assert!(
            api.details().get("outage").is_none(),
            "{} cannot come from bd",
            e.code()
        );
    }
    let config = ApiError::from(&CacheError::ConfigInvalid {
        detail: "d".into(),
        origin: "config_scope",
    });
    assert_eq!(config.code(), "BD_CACHE.CONFIG_INVALID");
    assert_eq!(config.details()["source"], "config_scope");
}

#[test]
fn errors_that_can_come_from_bd_carry_details_outage() {
    let down = BeadsCacheError::Unavailable {
        reason: "connection refused".into(),
        timed_out: false,
    };
    let answered = BeadsCacheError::Failed {
        code: 1,
        stderr: "no".into(),
    };
    let e = ApiError::from(&CacheError::from_backend(&down, CallKind::Refresh));
    assert_eq!(e.details()["outage"], true);
    let e = ApiError::from(&CacheError::from_backend(&answered, CallKind::Refresh));
    assert_eq!(e.code(), "BD_CACHE.BD_LIST_FAILED");
    assert_eq!(e.details()["outage"], false);
    let e = ApiError::from(&CacheError::BadArgs { detail: "x".into() });
    assert!(e.details().get("outage").is_none());
}
