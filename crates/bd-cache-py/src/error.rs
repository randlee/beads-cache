//! The Python exceptions: [`CacheError`] for typed cache errors, `ValueError` for bad arguments.
//!
//! One exception class carries every `BD_CACHE.*` code (REQ-BDC-0802, 0803).
//! `abi3` wheels cannot subclass `Exception` with a Rust class before Python
//! 3.12, so the class is a plain exception and the typed fields are
//! attributes set on each raised instance.

use bd_cache_api::ApiError;
use bd_cache_model::{CacheError as ModelError, ENVELOPE_KEYS, UNREACHABLE_CODES};
use pyo3::create_exception;
use pyo3::exceptions::{PyBaseException, PyException, PyRuntimeError};
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
     and `exit_code` (always 3).\n\n\
     Since 0.2 every instance also carries the contract's error fields: `kind` \
     (`invalid_input`, `not_found`, `unavailable`, `conflict` or `internal`), \
     `message`, `details` (a dict), `suggested_action` (`str` or `None`) and \
     `retryable` (`bool`, from the contract's error table)."
);

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
    set_contract_fields(py, value, &ApiError::from(e))
}

/// Sets `kind`, `message`, `details`, `suggested_action` and `retryable` (0.2).
fn set_contract_fields(
    py: Python<'_>,
    value: &Bound<'_, PyBaseException>,
    e: &ApiError,
) -> PyResult<()> {
    value.setattr("kind", e.kind().as_str())?;
    value.setattr("message", e.message())?;
    value.setattr("details", to_py(py, &Value::Object(e.details().clone()))?)?;
    value.setattr("suggested_action", e.suggested_action())?;
    value.setattr("retryable", e.retryable())?;
    Ok(())
}

/// Builds the raised exception for an error of `bd-cache-api` (the 0.2 namespaces).
///
/// A [`CacheError`] with the contract's fields and, so a caller can handle both
/// generations alike, the 0.1 ones: `detail` is the message, `fix` the suggested
/// action (`""` when there is none), `reason` is `None`, `payload` is empty, `body` is
/// the error object and `exit_code` the table's exit.
pub(crate) fn api_err(py: Python<'_>, e: &ApiError) -> PyErr {
    let err = CacheError::new_err(format!("{}: {}", e.code(), e.message()));
    if let Err(cause) = decorate_api(py, &err, e) {
        err.set_cause(py, Some(cause));
    }
    err
}

fn decorate_api(py: Python<'_>, err: &PyErr, e: &ApiError) -> PyResult<()> {
    let value = err.value(py);
    value.setattr("code", e.code())?;
    value.setattr("detail", e.message())?;
    value.setattr("fix", e.suggested_action().unwrap_or_default())?;
    value.setattr("reason", py.None())?;
    value.setattr(
        "payload",
        to_py(py, &Value::Object(serde_json::Map::new()))?,
    )?;
    value.setattr("body", to_py(py, &e.to_json())?)?;
    value.setattr("exit_code", e.exit_code().unwrap_or(3))?;
    set_contract_fields(py, value, e)
}

/// The extra fields of an error object: everything but the fixed keys.
fn payload_of(body: &Value) -> serde_json::Map<String, Value> {
    let mut payload = serde_json::Map::new();
    if let Value::Object(map) = body {
        for (k, v) in map {
            if !ENVELOPE_KEYS.contains(&k.as_str()) {
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

/// The model's [`UNREACHABLE_CODES`] as a Python `frozenset`.
pub(crate) fn unreachable(py: Python<'_>) -> PyResult<Bound<'_, pyo3::types::PyFrozenSet>> {
    pyo3::types::PyFrozenSet::new(py, UNREACHABLE_CODES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The classification of every variant is tested in `bd-cache-model`; the binding
    /// takes the envelope keys and the unreachable codes from there.
    #[test]
    fn the_payload_is_everything_but_the_envelope_keys() {
        let body = serde_json::json!({
            "ok": false, "code": "c", "detail": "d", "fix": "f", "reason": "r", "unmet": {}
        });
        let payload = payload_of(&body);
        assert_eq!(payload.keys().collect::<Vec<_>>(), ["unmet"]);
    }
}
