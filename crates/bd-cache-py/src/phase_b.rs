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
//! mutex and with the GIL released (ADR-PY-0003); `schedule_*` and
//! `sync_use_*` use the handle's `CacheScheduleStore` (ADR-BDC-0014), so the
//! process opens no second connection. `tick()` and `sync_use_python()` build
//! a current-thread tokio runtime on a helper thread the call spawns and
//! joins, and drop it before returning: no runtime outlives the call, and a
//! caller on an asyncio loop or inside a tokio runtime of its own never has
//! one nested in its context.
//!
//! Inside a run (`python -m bd_cache_rs.run`, `bd_cache_rs.sync.main`),
//! `drain`, `manage.flush`, `tick` and `schedule_*` raise `CacheError`
//! (`PRECONDITION`, `details.reason: "inside_run"`): the run's own lease and
//! schedule are the caller's (ADR-SYNC-0008).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bd_cache_api::{ApiError, Command, Request, decode};
use bd_cache_model::{CacheError as ModelError, PreconditionReason};
use bd_cache_sync::{RunContext, ScheduleSet, StopFlag, SyncError, SystemSchedules, UsePython};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use serde_json::{Map, Value};

use crate::api::{json_of, request_of};
use crate::convert::to_py;
use crate::error::api_err;
use crate::handle::{Inner, PyCache};

/// Set by `bd_cache_rs.run` and `bd_cache_rs.sync` while a run's code is called.
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
    let e = ApiError::from(&precondition(format!(
        "{verb} cannot be called inside a sync run or a post-sync hook: the run's lease and schedule are the caller's"
    )))
    .with_detail("reason", "inside_run");
    Err(api_err(py, &e))
}

fn precondition(detail: String) -> ModelError {
    ModelError::Precondition {
        detail,
        unmet: std::collections::BTreeMap::new(),
        reason: PreconditionReason::PreconditionFailed,
        source: None,
    }
}

/// One phase-B method: its name, the command it runs and its positional parameters.
struct Spec {
    name: &'static str,
    command: Command,
    positional: &'static [&'static str],
}

const SPECS: &[Spec] = &[
    Spec {
        name: "tick",
        command: Command::CacheTick,
        positional: &[],
    },
    Spec {
        name: "schedule_set",
        command: Command::ScheduleSet,
        positional: &["cron"],
    },
    Spec {
        name: "schedule_show",
        command: Command::ScheduleShow,
        positional: &[],
    },
    Spec {
        name: "schedule_enable",
        command: Command::ScheduleEnable,
        positional: &[],
    },
    Spec {
        name: "schedule_disable",
        command: Command::ScheduleDisable,
        positional: &[],
    },
    Spec {
        name: "schedule_clear",
        command: Command::ScheduleClear,
        positional: &[],
    },
    Spec {
        name: "sync_use_python",
        command: Command::SyncUsePython,
        positional: &[],
    },
    Spec {
        name: "sync_use_builtin",
        command: Command::SyncUseBuiltin,
        positional: &[],
    },
    Spec {
        name: "ingest_scope",
        command: Command::ScopesIngest,
        positional: &["scope", "rows", "blocked", "revision", "allow_empty"],
    },
    Spec {
        name: "scope_announce",
        command: Command::ScopesAnnounce,
        positional: &["scope", "since"],
    },
    Spec {
        name: "scopes_add",
        command: Command::ScopesAdd,
        positional: &["scope"],
    },
    Spec {
        name: "scopes_set",
        command: Command::ScopesSet,
        positional: &["scope"],
    },
    Spec {
        name: "scopes_remove",
        command: Command::ScopesRemove,
        positional: &["scope"],
    },
    Spec {
        name: "scopes_list",
        command: Command::ScopesList,
        positional: &[],
    },
];

/// The bound method `cache.<name>` when `name` is a phase-B method.
pub(crate) fn method(cache: &Bound<'_, PyCache>, name: &str) -> PyResult<Option<Py<PyAny>>> {
    let Some(spec) = SPECS.iter().position(|s| s.name == name) else {
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
    spec: usize,
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
        let spec = SPECS
            .get(self.spec)
            .ok_or_else(|| pyo3::exceptions::PyAttributeError::new_err("no such method"))?;
        let request = request_of_call(spec, args, kwargs)?;
        let cache = self.cache.bind(py).borrow();
        if !spec.command.is_front_end() {
            return cache.api_call(py, spec.command, &request);
        }
        if matches!(
            spec.name,
            "tick"
                | "schedule_set"
                | "schedule_enable"
                | "schedule_disable"
                | "schedule_clear"
                | "schedule_show"
        ) {
            refuse_in_run(py, spec.name)?;
        }
        let req = decode(spec.command, &request).map_err(|e| api_err(py, &e))?;
        let value = cache.run_api(py, move |inner| run_front_end(inner, &req))?;
        to_py(py, &value)
    }

    fn __repr__(&self) -> String {
        let name = SPECS.get(self.spec).map_or("?", |s| s.name);
        format!("<bound method Cache.{name}>")
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

/// Runs a front-end verb on a helper thread with its own current-thread runtime.
fn run_front_end(inner: &mut Inner, req: &Request) -> Result<Value, ApiError> {
    let store = inner.schedules.clone().map_err(|e| {
        ApiError::from(&ModelError::StoreFailed {
            detail: e,
            source: None,
        })
    })?;
    let schedules = SystemSchedules::new(store);
    let cache = Arc::clone(&inner.cache);
    let cache_path = inner.cache_path.clone();
    let beads_dir = inner.beads_dir.clone();
    std::thread::scope(|s| {
        s.spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| ApiError::internal(format!("cannot start a tokio runtime: {e}")))?;
            let out = rt.block_on(front_end(&schedules, &cache, cache_path, beads_dir, req));
            drop(rt);
            out
        })
        .join()
        .unwrap_or_else(|_| Err(ApiError::internal("the run's helper thread panicked")))
    })
}

async fn front_end(
    schedules: &SystemSchedules,
    cache: &bd_cache_sync::SharedCache<
        bd_cache_store::SqliteStore,
        bd_cache_store::FlockDrainLock,
        sc_bd_cli::BdCli,
    >,
    cache_path: PathBuf,
    beads_dir: PathBuf,
    req: &Request,
) -> Result<Value, ApiError> {
    let cache_err = |e: &ModelError| ApiError::from(e);
    match req {
        Request::ScheduleSet(r) => schedules
            .set(&ScheduleSet {
                expr: r.cron().to_owned(),
                budget_ms: r.budget_ms(),
                hook_reserve_ms: r.hook_reserve_ms(),
                post_sync: r.post_sync().map(str::to_owned),
                python: r.python().map(PathBuf::from),
                env: r.env().to_vec(),
            })
            .await
            .map_err(|e| cache_err(&e)),
        Request::ScheduleShow(_) => schedules.show(cache).await.map_err(|e| cache_err(&e)),
        Request::ScheduleEnable(_) => schedules.enable(true).await.map_err(|e| cache_err(&e)),
        Request::ScheduleDisable(_) => schedules.enable(false).await.map_err(|e| cache_err(&e)),
        Request::ScheduleClear(_) => schedules.clear().await.map_err(|e| cache_err(&e)),
        Request::SyncUseBuiltin(_) => schedules.use_builtin().await.map_err(|e| cache_err(&e)),
        Request::SyncUsePython(r) => schedules
            .use_python(
                cache,
                cache_path,
                Some(beads_dir),
                StopFlag::new(),
                &UsePython {
                    python: PathBuf::from(r.python()),
                    module: r.module().to_owned(),
                    cron: r.cron().map(str::to_owned),
                    timeout_ms: r.timeout_ms(),
                    env: r.env().to_vec(),
                },
            )
            .await
            .map_err(|e| cache_err(&e)),
        Request::CacheTick(_) => {
            let stop = StopFlag::new();
            let ctx = RunContext {
                row: bd_cache_sync::BUILTIN_ROW.to_owned(),
                period: None,
                cache_path,
                beads_dir: Some(beads_dir),
                kill: stop.clone(),
                stop,
            };
            let reply = schedules
                .tick(cache, &ctx)
                .await
                .map_err(|e| sync_error(&e))?;
            serde_json::to_value(reply)
                .map_err(|e| ApiError::internal(format!("the reply does not serialize: {e}")))
        }
        other => Err(ApiError::internal(format!(
            "{} is not a front-end verb",
            other.command().path()
        ))),
    }
}

/// A sync error as the contract's error (REQ-BDC-0720, 0729).
fn sync_error(e: &SyncError) -> ApiError {
    match e {
        SyncError::Cache(c) => ApiError::from(c),
        SyncError::PythonSyncEnabled(detail) => ApiError::from(&precondition(detail.clone()))
            .with_detail("reason", "python_sync_enabled")
            .with_suggested_action(
                "cache.sync_use_builtin() makes the built-in row the sync again",
            ),
        _ => ApiError::internal(format!("unexpected sync error: {e}")),
    }
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
}
