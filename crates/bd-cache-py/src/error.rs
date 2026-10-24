//! The Python exceptions: [`CacheError`] for typed cache errors, `ValueError` for bad arguments.
//!
//! One exception class carries every `BD_CACHE.*` code (REQ-BDC-0802, 0803).
//! `abi3` wheels cannot subclass `Exception` with a Rust class before Python
//! 3.12, so the class is a plain exception and the typed fields are
//! attributes set on each raised instance.

use bd_cache_model::CacheError as ModelError;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
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

/// Builds the raised exception for a typed error.
pub(crate) fn cache_err(py: Python<'_>, e: &ModelError) -> PyErr {
    match build(py, e) {
        Ok(err) | Err(err) => err,
    }
}

fn build(py: Python<'_>, e: &ModelError) -> PyResult<PyErr> {
    let body = e.to_json();
    let mut payload = serde_json::Map::new();
    if let Value::Object(map) = &body {
        for (k, v) in map {
            if !FIXED_KEYS.contains(&k.as_str()) {
                payload.insert(k.clone(), v.clone());
            }
        }
    }
    let err = CacheError::new_err(format!("{}: {}", e.code(), e.detail()));
    let value = err.value(py);
    value.setattr("code", e.code())?;
    value.setattr("detail", e.detail())?;
    value.setattr("fix", e.fix())?;
    value.setattr("reason", e.reason())?;
    value.setattr("payload", to_py(py, &Value::Object(payload))?)?;
    value.setattr("body", to_py(py, &body)?)?;
    value.setattr("exit_code", e.exit_code())?;
    Ok(err)
}

/// A bad argument (an invalid bead id, phase name, timeout, ...) as `ValueError`.
pub(crate) fn bad_argument(what: &str, e: impl std::fmt::Display) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(format!("{what}: {e}"))
}

/// Registers the exception on the module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("CacheError", py.get_type::<CacheError>())?;
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
