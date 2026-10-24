//! The Python exceptions: [`CacheError`] for typed cache errors, `ValueError` for bad arguments.
//!
//! One exception class carries every `BD_CACHE.*` code (REQ-BDC-0802, 0803).
//! `abi3` wheels cannot subclass `Exception` with a Rust class before Python
//! 3.12, so the class is a plain exception and the typed fields are
//! attributes set on each raised instance.

use bd_cache_model::CacheError as ModelError;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError};
use pyo3::prelude::*;
use serde_json::Value;

use crate::convert::to_py;

create_exception!(
    bd_cache_rs,
    CacheError,
    PyException,
    "A typed bd-cache error (`BD_CACHE.*`).\n\n\
     Attributes: `code` (for example `BD_CACHE.PRECONDITION`), `detail`, `fix`, \
     `reason` (the `/v0` refusal reason of a `PRECONDITION`, else `None`), \
     `payload` (the code's extra fields: `unmet`, `seq`, `known`, `bead`, `field`, \
     `phase`, `cause`, `source`), `body` (the whole BD-CACHE JSON reply as a dict) \
     and `exit_code` (always 3)."
);

/// The keys of [`ModelError::to_json`] that are attributes of their own, not payload.
const FIXED_KEYS: [&str; 5] = ["ok", "code", "detail", "fix", "reason"];

create_exception!(
    bd_cache_rs,
    ClosedError,
    PyRuntimeError,
    "A verb was called on a closed handle (or on one a panicking verb closed).\n\n\
     Open a new `bd_cache_rs.Cache` to keep using the cache. A subclass of `RuntimeError`."
);

/// Builds the raised exception for a typed error.
///
/// Always a [`CacheError`] carrying `code`, `detail` and `fix` in its message. If
/// setting an attribute fails (it cannot on a plain exception instance, but the
/// calls are fallible), the failure is chained as the exception's `__cause__`
/// rather than replacing it.
pub(crate) fn cache_err(py: Python<'_>, e: &ModelError) -> PyErr {
    let err = CacheError::new_err(format!("{}: {}", e.code(), e.detail()));
    if let Err(cause) = decorate(py, &err, e) {
        err.set_cause(py, Some(cause));
    }
    err
}

fn decorate(py: Python<'_>, err: &PyErr, e: &ModelError) -> PyResult<()> {
    let body = e.to_json();
    let payload = payload_of(&body);
    let value = err.value(py);
    value.setattr("code", e.code())?;
    value.setattr("detail", e.detail())?;
    value.setattr("fix", e.fix())?;
    value.setattr("reason", e.reason())?;
    value.setattr("payload", to_py(py, &Value::Object(payload))?)?;
    value.setattr("body", to_py(py, &body)?)?;
    value.setattr("exit_code", e.exit_code())?;
    Ok(())
}

/// The extra fields of an error object: everything but the fixed keys.
fn payload_of(body: &Value) -> serde_json::Map<String, Value> {
    let mut payload = serde_json::Map::new();
    if let Value::Object(map) = body {
        for (k, v) in map {
            if !FIXED_KEYS.contains(&k.as_str()) {
                payload.insert(k.clone(), v.clone());
            }
        }
    }
    payload
}

/// A bad argument (an invalid bead id, phase name, timeout, ...) as `ValueError`.
pub(crate) fn bad_argument(what: &str, e: impl std::fmt::Display) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(format!("{what}: {e}"))
}

/// Registers the exception on the module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("CacheError", py.get_type::<CacheError>())?;
    m.add("ClosedError", py.get_type::<ClosedError>())?;
    Ok(())
}

/// The codes that mean `bd` could not answer at all: a backend outage, never a write attempt.
///
/// The set `bd_cache.py` calls `UNREACHABLE`, for a scheduler's
/// `error.code in UNREACHABLE` tests (`docs/bd-cache-py/architecture.md`, Mapping).
const UNREACHABLE_CODES: [&str; 4] = [
    "BD_CACHE.BD_UNAVAILABLE",
    "BD_CACHE.BD_VERSION_FAILED",
    "BD_CACHE.BD_SCHEMA_FAILED",
    "BD_CACHE.BD_LIST_FAILED",
];

/// [`UNREACHABLE_CODES`] as a Python `frozenset`.
pub(crate) fn unreachable(py: Python<'_>) -> PyResult<Bound<'_, pyo3::types::PyFrozenSet>> {
    pyo3::types::PyFrozenSet::new(py, UNREACHABLE_CODES)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use bd_cache_model::{GuardKey, PreconditionReason, Seq, SyncCause, Unmet};
    use sc_beads::BeadId;

    use super::*;

    fn d() -> String {
        "detail".to_owned()
    }

    /// One error of every variant of the model's `CacheError`.
    fn every_variant() -> Vec<ModelError> {
        let x = bd_cache_model::ScopeId::phase("x").unwrap();
        vec![
            ModelError::BdUnavailable {
                detail: d(),
                source: None,
            },
            ModelError::NoDatabase { detail: d() },
            ModelError::WrongDatabase { detail: d() },
            ModelError::NotFound { detail: d() },
            ModelError::BdVersionFailed {
                detail: d(),
                source: None,
            },
            ModelError::BdSchemaFailed {
                detail: d(),
                source: None,
            },
            ModelError::BdContract {
                bead: Some(BeadId::new("b1").unwrap()),
                field: "priority".to_owned(),
                detail: d(),
                source: None,
            },
            ModelError::BdWriteFailed {
                detail: d(),
                source: None,
            },
            ModelError::BdListFailed {
                detail: d(),
                source: None,
            },
            ModelError::SyncFailed {
                detail: d(),
                scope: Some(x.clone()),
                cause: Some(SyncCause::BdContract),
                source: None,
            },
            ModelError::Precondition {
                detail: d(),
                unmet: BTreeMap::from([(
                    GuardKey::Status,
                    Unmet {
                        expected: "open".to_owned(),
                        actual: "closed".to_owned(),
                    },
                )]),
                reason: PreconditionReason::PreconditionFailed,
                source: None,
            },
            ModelError::IdemConflict {
                detail: d(),
                seq: Seq::new(1),
            },
            ModelError::BadOp { detail: d() },
            ModelError::BadArgs { detail: d() },
            ModelError::BadPrecondition { detail: d() },
            ModelError::SchemaVersion { detail: d() },
            ModelError::PruneBlocked { detail: d() },
            ModelError::NotEscalated { detail: d() },
            ModelError::ScopeUnknown {
                detail: d(),
                known: vec![x],
            },
            ModelError::StoreFailed {
                detail: d(),
                source: None,
            },
        ]
    }

    #[test]
    fn unreachable_codes_are_exactly_the_four_backend_did_not_answer_variants() {
        let in_list: BTreeSet<&str> = UNREACHABLE_CODES.into_iter().collect();
        let mut found = BTreeSet::new();
        for e in every_variant() {
            let is_unreachable = matches!(
                e,
                ModelError::BdUnavailable { .. }
                    | ModelError::BdVersionFailed { .. }
                    | ModelError::BdSchemaFailed { .. }
                    | ModelError::BdListFailed { .. }
            );
            assert_eq!(in_list.contains(e.code()), is_unreachable, "{}", e.code());
            assert_eq!(
                SyncCause::of(&e).is_some() && is_unreachable,
                is_unreachable,
                "{}",
                e.code()
            );
            if is_unreachable {
                found.insert(e.code());
            }
        }
        assert_eq!(found, in_list);
    }

    #[test]
    fn fixed_keys_match_the_model_envelope_for_every_variant() {
        let variants = every_variant();
        assert_eq!(
            variants.len(),
            20,
            "a new CacheError variant needs a case here"
        );
        let fixed: BTreeSet<&str> = FIXED_KEYS.into_iter().collect();
        for e in &variants {
            let body = e.to_json();
            let Value::Object(map) = &body else {
                panic!("{body}")
            };
            let keys: BTreeSet<&str> = map.keys().map(String::as_str).collect();
            for always in ["ok", "code", "detail", "fix"] {
                assert!(keys.contains(always), "{}: no {always}", e.code());
            }
            assert_eq!(
                keys.contains("reason"),
                e.reason().is_some(),
                "{}",
                e.code()
            );
            assert!(fixed.is_superset(&keys.intersection(&fixed).copied().collect()));
            let expected: &[&str] = match e {
                ModelError::BdContract { .. } => &["bead", "field"],
                ModelError::Precondition { .. } => &["unmet"],
                ModelError::IdemConflict { .. } => &["seq"],
                ModelError::SyncFailed { .. } => &["cause", "phase"],
                ModelError::ScopeUnknown { .. } => &["known"],
                _ => &[],
            };
            let extra = payload_of(&body);
            let payload: BTreeSet<&str> = extra.keys().map(String::as_str).collect();
            let want: BTreeSet<&str> = expected.iter().copied().collect();
            assert_eq!(payload, want, "{}", e.code());
        }
    }
}
