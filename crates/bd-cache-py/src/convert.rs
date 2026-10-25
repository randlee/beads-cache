//! Conversions between Rust reply values and Python objects.
//!
//! Every reply type of `bd-cache-model` derives `serde::Serialize`, so a
//! reply reaches Python in two steps: `serde_json::to_value`, then [`to_py`],
//! which builds plain `dict`, `list`, `str`, `int`, `float`, `bool` and `None`
//! objects. Replies therefore hold only builtin types and are versioned with
//! the wheel (`docs/bd-cache-py/architecture.md`, Reply shapes).

use std::time::Duration;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde::Serialize;
use serde_json::Value;

/// Builds the Python object for a JSON value.
///
/// Integers stay `int`, other numbers become `float`, objects keep their key
/// order, and `null` is `None`.
pub(crate) fn to_py<'py>(py: Python<'py>, v: &Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match v {
        Value::Null => py.None().into_bound(py),
        Value::Bool(b) => b.into_pyobject(py)?.to_owned().into_any(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py)?.into_any()
            } else if let Some(u) = n.as_u64() {
                u.into_pyobject(py)?.into_any()
            } else {
                // Without serde_json's `arbitrary_precision`, every number is an i64, a
                // u64 or an f64, so this arm always has an f64; refuse rather than invent one.
                let f = n.as_f64().ok_or_else(|| {
                    PyValueError::new_err(format!("bd-cache-py: unrepresentable number {n}"))
                })?;
                f.into_pyobject(py)?.into_any()
            }
        }
        Value::String(s) => s.into_pyobject(py)?.into_any(),
        Value::Array(items) => {
            let converted = items
                .iter()
                .map(|i| to_py(py, i))
                .collect::<PyResult<Vec<_>>>()?;
            PyList::new(py, converted)?.into_any()
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, item) in map {
                dict.set_item(k, to_py(py, item)?)?;
            }
            dict.into_any()
        }
    })
}

/// A Rust reply as its Python form.
pub(crate) fn reply<'py, T: Serialize>(py: Python<'py>, value: &T) -> PyResult<Bound<'py, PyAny>> {
    let json = serde_json::to_value(value).map_err(|e| {
        PyRuntimeError::new_err(format!("bd-cache-py: a reply did not serialize: {e}"))
    })?;
    to_py(py, &json)
}

/// JSON text for a `write` argument or precondition: a `str` is taken as the
/// JSON text itself, any other object is serialized with `json.dumps`.
///
/// # Errors
///
/// `TypeError` / `ValueError` from `json.dumps` for an object it cannot encode.
pub(crate) fn json_text(py: Python<'_>, arg: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(text) = arg.extract::<String>() {
        return Ok(text);
    }
    py.import("json")?.call_method1("dumps", (arg,))?.extract()
}

/// A timeout given in seconds.
///
/// # Errors
///
/// `ValueError` for a negative, NaN or unrepresentable number.
pub(crate) fn seconds(name: &str, secs: f64) -> PyResult<Duration> {
    Duration::try_from_secs_f64(secs).map_err(|e| {
        PyValueError::new_err(format!(
            "{name} must be a non-negative number of seconds, got {secs} ({e})"
        ))
    })
}

/// A configured timeout given in seconds: positive, as the file loader requires
/// of `call_timeout_s` and `miss_fill_timeout_ms` (lead ruling S56).
///
/// A value so small it rounds to zero nanoseconds is zero, and refused.
///
/// # Errors
///
/// `ValueError` for zero and for everything [`seconds`] refuses.
pub(crate) fn positive_seconds(name: &str, secs: f64) -> PyResult<Duration> {
    let d = seconds(name, secs)?;
    if d.is_zero() {
        return Err(PyValueError::new_err(format!(
            "{name} must be a positive number of seconds, got {secs}"
        )));
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_seconds_refuse_zero_and_what_rounds_to_zero() {
        for zero in [0.0, -0.0, 1e-12] {
            assert!(positive_seconds("t", zero).is_err(), "{zero}");
        }
        assert_eq!(
            positive_seconds("t", 0.001).ok(),
            Some(Duration::from_millis(1))
        );
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(positive_seconds("t", bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn seconds_accept_zero_and_fractions() {
        assert_eq!(seconds("t", 0.0).ok(), Some(Duration::ZERO));
        assert_eq!(seconds("t", 0.5).ok(), Some(Duration::from_millis(500)));
    }

    #[test]
    fn seconds_refuse_negative_nan_and_infinity() {
        for bad in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(seconds("t", bad).is_err(), "{bad}");
        }
    }
}
