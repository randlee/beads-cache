//! The 0.2 namespaces `cache.beads` and `cache.manage` over `bd-cache-api` (contract §10).
//!
//! A method's name is the command path joined with `_`, without the `cache_` prefix
//! of surface (b). It takes the request as keyword arguments, builds the request
//! through `bd_cache_api::decode` and nothing else, runs it with `execute`, and
//! returns the command's `data` as plain Python objects (ADR-PY-0005). No argument is
//! checked here: the decoder's code for a bad member is the CLI's code
//! (REQ-BDC-0807). `cache tick` is not exposed; it belongs to `bd-cache-sync`.
//!
//! Only a Python-level type mistake is a `TypeError`: a positional argument, a
//! non-`str` keyword key or a value with no JSON form.

use bd_cache_api::Command;
use pyo3::exceptions::{PyAttributeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use serde_json::{Map, Number, Value};

use crate::handle::PyCache;

/// The contract version these namespaces implement (REQ-BDC-0811); the api crate's.
pub(crate) const CONTRACT_VERSION: &str = bd_cache_api::CONTRACT_VERSION;

/// Registers `CONTRACT_VERSION`, `Beads` and `Manage` on the module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("CONTRACT_VERSION", CONTRACT_VERSION)?;
    m.add_class::<PyBeads>()?;
    m.add_class::<PyManage>()?;
    Ok(())
}

/// How deep a request value may nest (a `list`, `tuple` or `dict` inside another counts one level).
///
/// `json_of` recurses on the native stack, which a self-referencing container or a very
/// deep one would overflow, killing the interpreter. Past this depth the value is refused
/// with `ValueError`, so a cycle always ends in a normal exception. No contract request
/// nests beyond four levels (a patch's `metadata.set` map).
const MAX_DEPTH: usize = 64;

/// The object for `cache.<name>` when `name` is a namespace; `AttributeError` otherwise.
pub(crate) fn namespace(cache: &Bound<'_, PyCache>, name: &str) -> PyResult<Py<PyAny>> {
    let py = cache.py();
    let handle = cache.clone().unbind();
    match name {
        "beads" => Ok(Py::new(py, PyBeads { cache: handle })?.into_any()),
        "manage" => Ok(Py::new(py, PyManage { cache: handle })?.into_any()),
        other => Err(PyAttributeError::new_err(format!(
            "'Cache' object has no attribute '{other}'"
        ))),
    }
}

/// The request object of keyword arguments, as the JSON the decoder reads.
pub(crate) fn request_of(kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Value> {
    let mut map = Map::new();
    if let Some(kwargs) = kwargs {
        for (key, value) in kwargs.iter() {
            let key = key.extract::<String>()?;
            map.insert(key, json_of(&value, 0)?);
        }
    }
    Ok(Value::Object(map))
}

/// A Python value as JSON: `None`, `bool`, `int`, finite `float`, `str`, `list` / `tuple`
/// and `dict` with `str` keys.
///
/// # Errors
///
/// `TypeError` for any other type or a non-`str` key; `ValueError` for an `int` outside
/// 64 bits, a non-finite `float`, or nesting deeper than [`MAX_DEPTH`] (which includes a
/// container that contains itself).
pub(crate) fn json_of(v: &Bound<'_, PyAny>, depth: usize) -> PyResult<Value> {
    if v.is_none() {
        return Ok(Value::Null);
    }
    // `bool` first: it is a subclass of `int`.
    if v.is_instance_of::<PyBool>() {
        return Ok(Value::Bool(v.extract::<bool>()?));
    }
    if v.is_instance_of::<PyInt>() {
        if let Ok(i) = v.extract::<i64>() {
            return Ok(Value::from(i));
        }
        return v.extract::<u64>().map(Value::from).map_err(|e| {
            PyValueError::new_err(format!("integer {v} does not fit in 64 bits ({e})"))
        });
    }
    if v.is_instance_of::<PyFloat>() {
        let f = v.extract::<f64>()?;
        return Number::from_f64(f)
            .map(Value::Number)
            .ok_or_else(|| PyValueError::new_err(format!("{f} is not a finite number")));
    }
    if v.is_instance_of::<PyString>() {
        return Ok(Value::String(v.extract::<String>()?));
    }
    let container = v.is_instance_of::<PyList>()
        || v.is_instance_of::<PyTuple>()
        || v.is_instance_of::<PyDict>();
    if container && depth >= MAX_DEPTH {
        return Err(PyValueError::new_err(format!(
            "a request nests deeper than {MAX_DEPTH} levels (or contains itself)"
        )));
    }
    if v.is_instance_of::<PyList>() || v.is_instance_of::<PyTuple>() {
        return v
            .try_iter()?
            .map(|item| json_of(&item?, depth + 1))
            .collect::<PyResult<Vec<_>>>()
            .map(Value::Array);
    }
    if let Ok(dict) = v.cast::<PyDict>() {
        let mut map = Map::new();
        for (key, value) in dict.iter() {
            let key = key.extract::<String>().map_err(|e| {
                PyTypeError::new_err(format!("a request object needs str keys, got {key} ({e})"))
            })?;
            map.insert(key, json_of(&value, depth + 1)?);
        }
        return Ok(Value::Object(map));
    }
    let type_name = v.get_type().name()?;
    Err(PyTypeError::new_err(format!(
        "{type_name} has no JSON form; use None, bool, int, float, str, list or dict"
    )))
}

/// Generates a namespace class: one method per command, each `(**kwargs)`.
macro_rules! namespace_class {
    (
        $(#[$meta:meta])*
        $class:ident = $py_name:literal {
            $( $(#[$mdoc:meta])* $method:ident => $command:ident ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[pyclass(name = $py_name, module = "bd_cache_rs", frozen)]
        #[derive(Debug)]
        pub(crate) struct $class {
            cache: Py<PyCache>,
        }

        #[pymethods]
        impl $class {
            $(
                $(#[$mdoc])*
                #[pyo3(signature = (**kwargs))]
                fn $method<'py>(
                    &self,
                    py: Python<'py>,
                    kwargs: Option<&Bound<'py, PyDict>>,
                ) -> PyResult<Bound<'py, PyAny>> {
                    let request = request_of(kwargs)?;
                    self.cache.bind(py).borrow().api_call(py, Command::$command, &request)
                }
            )+
        }

        impl $class {
            /// Every method of the class with the command it runs.
            #[cfg(test)]
            const COMMANDS: &'static [(&'static str, Command)] =
                &[ $( (stringify!($method), Command::$command) ),+ ];
        }
    };
}

namespace_class! {
    /// `cache.beads`: surface (a), the beads operations over the cache (contract §5).
    ///
    /// Each method takes the request as keyword arguments and returns the command's
    /// `data` as a `dict`; a refusal raises `CacheError` (`kind`, `code`, `message`,
    /// `details`, `suggested_action`, `retryable`).
    PyBeads = "Beads" {
        /// `issues get`: `id`, `miss_timeout_ms`.
        issues_get => IssuesGet,
        /// `issues list`: `scope`, `status`, `label`, `label_any`, `exclude_label`, `assignee`, `all`, `sort`, `limit`, `cursor`.
        issues_list => IssuesList,
        /// `ready`: `scope`, `label`, `label_any`, `exclude_label`, `assignee`, `unassigned`, `priority`, `limit`, `sort`, `fresh_within_ms`.
        ready => Ready,
        /// `dependencies list`: `issue_id`, `type`.
        dependencies_list => DependenciesList,
        /// `issues update`: `id`, `actor`, `idempotency_key`, `patch`, `expected_status`, `expected_assignee`.
        issues_update => IssuesUpdate,
        /// `issues close`: `id`, `actor`, `idempotency_key`, `reason`, `expected_status`, `expected_assignee`.
        issues_close => IssuesClose,
        /// `dependencies add`: `actor`, `idempotency_key`, `edges` (exactly one edge: `{issue_id, depends_on_id, type}`).
        dependencies_add => DependenciesAdd,
        /// `context`: the project, scopes and capabilities.
        context => Context,
    }
}

namespace_class! {
    /// `cache.manage`: surface (b), cache management (contract §6).
    ///
    /// Each method takes the request as keyword arguments and returns the command's
    /// `data` as a `dict`; a refusal raises `CacheError`. `cache tick` is not here.
    PyManage = "Manage" {
        /// `cache status`.
        status => CacheStatus,
        /// `cache health`: `probe`.
        health => CacheHealth,
        /// `cache scopes`.
        scopes => CacheScopes,
        /// `cache refresh scopes`: `scopes`.
        refresh_scopes => CacheRefreshScopes,
        /// `cache refresh issues`: `ids`, `timeout_ms`.
        refresh_issues => CacheRefreshIssues,
        /// `cache flush`: `start_gate_ms`, `deadline_ms`.
        flush => CacheFlush,
        /// `cache evict`: `scope`.
        evict => CacheEvict,
        /// `cache journal get`: `seq` or `idempotency_key`.
        journal_get => JournalGet,
        /// `cache journal list`: `state`, `unannounced`.
        journal_list => JournalList,
        /// `cache journal resolve`: `seq`, `note`, `actor`.
        journal_resolve => JournalResolve,
        /// `cache journal announce`: `seq`.
        journal_announce => JournalAnnounce,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A method's name is the command path joined with `_`, without the surface-(b)
    /// `cache_` prefix; together the two classes cover every 0.2 command. The
    /// phase-B verbs are the handle's own methods (`phase_b.rs`), and `cache cron
    /// run` is the CLI's alone.
    #[test]
    fn method_names_follow_the_command_paths_and_cover_every_command() {
        let mut seen = Vec::new();
        for (method, command) in PyBeads::COMMANDS.iter().chain(PyManage::COMMANDS) {
            let path = command.path().replace(' ', "_");
            assert_eq!(path.strip_prefix("cache_").unwrap_or(&path), *method);
            seen.push(*command);
        }
        seen.sort_by_key(|c| c.path());
        let phase_b = |c: &Command| {
            c.is_front_end()
                || (c.path().starts_with("cache scopes ") && *c != Command::CacheScopes)
        };
        let mut all: Vec<Command> = Command::ALL.into_iter().filter(|c| !phase_b(c)).collect();
        all.sort_by_key(|c| c.path());
        assert_eq!(seen, all);
    }
}
