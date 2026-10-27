//! The phase-B methods of `Cache` (ADR-PY-0006, REQ-BDC-0720..0731).
//!
//! `Cache.tick()`, `schedule_set/show/enable/disable/clear`,
//! `sync_use_python` / `sync_use_builtin`, `ingest_scope`, `scope_announce`
//! and `scopes_add/set/remove/list` are resolved by `Cache.__getattr__`, not
//! as class attributes, so `dir(Cache)` stays the 0.1 set; each is an
//! ordinary bound callable all the same. Every request goes through
//! `bd_cache_api::decode` (the CLI's validation and codes). New scope
//! arguments take scope names as written (ADR-BDC-0013).
//!
//! Every method runs on the handle's own `Cache` and connection, under its
//! mutex and with the GIL released (ADR-PY-0003). `tick()`, `schedule_*` and
//! `sync_use_*` run through `bd_cache_sync::execute` over the handle's
//! `CacheScheduleStore` (ADR-BDC-0014), so the process opens no second
//! connection and the CLI and this module give the same codes and details
//! (only the suggested action is worded for Python). They build a
//! current-thread tokio runtime on a helper thread the call spawns and
//! joins, and drop it before returning: no runtime outlives the call, and a
//! caller on an asyncio loop or inside a tokio runtime of its own never has
//! one nested in its context. A Ctrl-C during the call stops it and is
//! raised when it returns.
//!
//! Inside a run (`python -m bd_cache_rs.run`, `bd_cache_rs.sync.main`),
//! `drain`, `manage.flush`, `tick`, `schedule_*` and `sync_use_*` raise
//! `CacheError` (`PRECONDITION`, `details.reason: "inside_run"`): the run's
//! own lease and schedule are the caller's (ADR-SYNC-0008).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bd_cache_api::{ApiError, Command, Request, decode};
use bd_cache_model::{CacheError as ModelError, PreconditionReason};
use bd_cache_sync::{FrontEnd, StopFlag, SystemSchedules};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use serde_json::{Map, Value};

use crate::api::{json_of, request_of};
use crate::convert::to_py;
use crate::error::api_err;
use crate::handle::{Inner, PyCache};

/// Set by `bd_cache_rs.run` and `bd_cache_rs.sync` while a run's code is called.
///
/// The module's second process-wide static (ADR-PY-0006's sprint b-8
/// amendment; ADR-PY-0001's `ProjectLocks` set moved to `bd-cache-sync`): a
/// run's code reaches the cache through handles it opens itself, so the mark
/// cannot live on one handle; it is the process's "a sync run or a
/// post-sync hook is calling user code now", set and cleared by the two
/// entry points around that call only.
static IN_RUN: AtomicBool = AtomicBool::new(false);

/// `bd_cache_rs._native._set_in_run(flag)`: marks the process as inside a run.
#[pyfunction]
#[pyo3(name = "_set_in_run")]
pub(crate) fn set_in_run(flag: bool) {
    IN_RUN.store(flag, Ordering::SeqCst);
}

/// `PRECONDITION` (`inside_run`) for `verb` inside a run.
pub(crate) fn refuse_in_run(py: Python<'_>, verb: &str) -> PyResult<()> {
    if !IN_RUN.load(Ordering::SeqCst) {
        return Ok(());
    }
    let e = ModelError::Precondition {
        detail: format!(
            "{verb} cannot be called inside a sync run or a post-sync hook: the run's lease and schedule are the caller's"
        ),
        unmet: std::collections::BTreeMap::new(),
        reason: PreconditionReason::InsideRun,
        source: None,
    };
    Err(api_err(py, &FrontEnd::Python.api_error(&e)))
}

/// How often a front-end verb's caller checks for a Python signal
/// (`KeyboardInterrupt`) while the verb runs with the GIL released.
const SIGNAL_POLL: Duration = Duration::from_millis(100);

/// One phase-B method: its name, the command it runs, its positional
/// parameters, and whether a run's code may call it (`refuse_in_run`).
struct Spec {
    name: &'static str,
    command: Command,
    positional: &'static [&'static str],
    /// Refused inside a run (ADR-SYNC-0008): the run's lease and schedule
    /// are the caller's, so the verbs that take the lease or change a row are.
    refuse_in_run: bool,
}

const SPECS: &[Spec] = &[
    Spec {
        name: "tick",
        command: Command::CacheTick,
        positional: &[],
        refuse_in_run: true,
    },
    Spec {
        name: "schedule_set",
        command: Command::ScheduleSet,
        positional: &["cron"],
        refuse_in_run: true,
    },
    Spec {
        name: "schedule_show",
        command: Command::ScheduleShow,
        positional: &[],
        refuse_in_run: true,
    },
    Spec {
        name: "schedule_enable",
        command: Command::ScheduleEnable,
        positional: &[],
        refuse_in_run: true,
    },
    Spec {
        name: "schedule_disable",
        command: Command::ScheduleDisable,
        positional: &[],
        refuse_in_run: true,
    },
    Spec {
        name: "schedule_clear",
        command: Command::ScheduleClear,
        positional: &[],
        refuse_in_run: true,
    },
    Spec {
        name: "sync_use_python",
        command: Command::SyncUsePython,
        positional: &[],
        refuse_in_run: true,
    },
    Spec {
        name: "sync_use_builtin",
        command: Command::SyncUseBuiltin,
        positional: &[],
        refuse_in_run: true,
    },
    Spec {
        name: "ingest_scope",
        command: Command::ScopesIngest,
        positional: &["scope", "rows", "blocked", "revision", "allow_empty"],
        refuse_in_run: false,
    },
    Spec {
        name: "scope_announce",
        command: Command::ScopesAnnounce,
        positional: &["scope", "since"],
        refuse_in_run: false,
    },
    Spec {
        name: "scopes_add",
        command: Command::ScopesAdd,
        positional: &["scope"],
        refuse_in_run: false,
    },
    Spec {
        name: "scopes_set",
        command: Command::ScopesSet,
        positional: &["scope"],
        refuse_in_run: false,
    },
    Spec {
        name: "scopes_remove",
        command: Command::ScopesRemove,
        positional: &["scope"],
        refuse_in_run: false,
    },
    Spec {
        name: "scopes_list",
        command: Command::ScopesList,
        positional: &[],
        refuse_in_run: false,
    },
];

/// The bound method `cache.<name>` when `name` is a phase-B method.
pub(crate) fn method(cache: &Bound<'_, PyCache>, name: &str) -> PyResult<Option<Py<PyAny>>> {
    let Some(spec) = SPECS.iter().find(|s| s.name == name) else {
        return Ok(None);
    };
    let py = cache.py();
    let m = PyMethod {
        cache: cache.clone().unbind(),
        spec,
    };
    Ok(Some(Py::new(py, m)?.into_any()))
}

/// `cache.<method>`: a phase-B method bound to its handle.
#[pyclass(name = "Method", module = "bd_cache_rs", frozen)]
#[derive(Debug)]
pub(crate) struct PyMethod {
    cache: Py<PyCache>,
    spec: &'static Spec,
}

impl std::fmt::Debug for Spec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

#[pymethods]
impl PyMethod {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let spec = self.spec;
        let request = request_of_call(spec, args, kwargs)?;
        let cache = self.cache.bind(py).borrow();
        if spec.refuse_in_run {
            refuse_in_run(py, spec.name)?;
        }
        if !spec.command.is_front_end() {
            return cache.api_call(py, spec.command, &request);
        }
        let req = decode(spec.command, &request).map_err(|e| api_err(py, &e))?;
        // Written only by the polling loop of `run_front_end`, on the calling
        // thread; read here once `run_api` has returned.
        let mut interrupt = None;
        let value = cache.run_api(py, |inner| run_front_end(inner, &req, &mut interrupt));
        // A Ctrl-C during the call stopped it; it is raised once the call ended.
        if let Some(e) = interrupt {
            return Err(e);
        }
        to_py(py, &value?)
    }

    fn __repr__(&self) -> String {
        format!("<bound method Cache.{}>", self.spec.name)
    }
}

/// The request of one call: the positional values by name, then the keywords.
fn request_of_call(
    spec: &Spec,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Value> {
    if args.len() > spec.positional.len() {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "{}() takes {} positional arguments but {} were given",
            spec.name,
            spec.positional.len(),
            args.len()
        )));
    }
    let mut map: Map<String, Value> = match request_of(kwargs)? {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    for (name, value) in spec.positional.iter().zip(args.iter()) {
        if map.contains_key(*name) {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "{}() got multiple values for argument '{name}'",
                spec.name
            )));
        }
        map.insert((*name).to_owned(), json_of(&value, 0)?);
    }
    Ok(Value::Object(map))
}

/// Runs a front-end verb through `bd_cache_sync::execute` on a helper thread
/// with its own current-thread runtime, dropped before the call returns.
///
/// The calling thread (the GIL released) waits for it, checking for a
/// Python signal every [`SIGNAL_POLL`]: a `KeyboardInterrupt` sets the run's
/// stop flag (a tick stops between steps and kills its hook; `use-python`
/// kills its script and restores) and is kept in `interrupt`, raised once
/// the call has ended. A call is otherwise bounded by its run's budget or
/// timeout (at most 610 s), while the handle's mutex is held (ADR-PY-0003).
fn run_front_end(
    inner: &mut Inner,
    req: &Request,
    interrupt: &mut Option<PyErr>,
) -> Result<Value, ApiError> {
    let store = inner
        .schedules
        .store()
        .map_err(|e| FrontEnd::Python.api_error(&e))?;
    let schedules = SystemSchedules::system(store, Arc::clone(&inner.cache), inner.files.clone());
    let stop = StopFlag::new();
    std::thread::scope(|s| {
        let helper = s.spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| ApiError::internal(format!("cannot start a tokio runtime: {e}")))?;
            let out = rt.block_on(bd_cache_sync::execute(
                &schedules,
                req,
                &stop,
                FrontEnd::Python,
            ));
            drop(rt);
            out
        });
        while !helper.is_finished() {
            std::thread::sleep(SIGNAL_POLL);
            // A closure: the method path is not general over `Python`'s lifetime.
            #[expect(
                clippy::redundant_closure_for_method_calls,
                reason = "the method path is not general over the GIL token's lifetime"
            )]
            let signalled = Python::attach(|py| py.check_signals());
            if let Err(e) = signalled {
                stop.stop();
                *interrupt = Some(e);
            }
        }
        helper
            .join()
            .unwrap_or_else(|_| Err(ApiError::internal("the run's helper thread panicked")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The phase-B methods name each phase-B command once, and no 0.1 verb
    /// or namespace: `dir(Cache)` stays the 0.1 set and nothing is shadowed.
    #[test]
    fn every_phase_b_command_has_one_method_and_none_shadows_a_0_1_name() {
        let zero_one = [
            "get",
            "refresh",
            "write",
            "list",
            "ready",
            "sync",
            "drain",
            "entry",
            "resolve",
            "prune",
            "status",
            "unannounced_escalations",
            "mark_announced",
            "sync_alert",
            "set_sync_alert",
            "open_work",
            "beads",
            "manage",
            "path",
            "closed",
            "close",
        ];
        let mut commands: Vec<&str> = SPECS.iter().map(|s| s.command.path()).collect();
        commands.sort_unstable();
        commands.dedup();
        assert_eq!(commands.len(), SPECS.len(), "one method per command");
        for spec in SPECS {
            assert!(
                !zero_one.contains(&spec.name),
                "{} shadows a 0.1 name",
                spec.name
            );
        }
        let phase_b = Command::ALL
            .iter()
            .filter(|c| {
                c.path().starts_with("cache scopes ")
                    || c.path().starts_with("cache schedule ")
                    || c.path().starts_with("cache sync ")
                    || *c == &Command::CacheTick
            })
            .count();
        assert_eq!(
            phase_b,
            SPECS.len(),
            "every phase-B verb but cron run has a method"
        );
    }

    /// The verbs that take the lease or change a row are refused inside a
    /// run, and exactly those (B8Q1-025): `tick`, every `schedule_*` and
    /// both `sync_use_*`; the store-only verbs a run's code calls are not.
    #[test]
    fn exactly_the_lease_and_row_verbs_are_refused_inside_a_run() {
        for spec in SPECS {
            let changes_the_sync = spec.command.is_front_end();
            assert_eq!(spec.refuse_in_run, changes_the_sync, "{}", spec.name);
        }
        assert!(
            SPECS
                .iter()
                .any(|s| s.name == "sync_use_builtin" && s.refuse_in_run)
        );
        assert!(
            SPECS
                .iter()
                .any(|s| s.name == "ingest_scope" && !s.refuse_in_run)
        );
    }
}
