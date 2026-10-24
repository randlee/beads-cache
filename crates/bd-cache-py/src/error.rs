//! The Python exceptions: [`CacheError`] for typed cache errors, `ValueError` for bad arguments.
//!
//! One exception class carries every `BD_CACHE.*` code (REQ-BDC-0802, 0803).
//! `abi3` wheels cannot subclass `Exception` with a Rust class before Python
//! 3.12, so the class is a plain exception and the typed fields are
//! attributes set on each raised instance.

use bd_cache_model::{CacheError as ModelError, ENVELOPE_KEYS, UNREACHABLE_CODES};
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
