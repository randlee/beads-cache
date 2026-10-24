//! `bd_cache_rs.Cache`: the Python handle over the concrete composition.
//!
//! Threading follows ADR-CORE-0001 and ADR-PY-0003: a `Cache` is `Send` but not
//! `Sync`, so the handle keeps it behind a `Mutex` (the "behind their own
//! mutex" option of ADR-CORE-0001). Any Python thread may use the handle; verbs
//! run one at a time, and a thread that waits for the lock or for `bd` does so
//! with the GIL released, so other Python threads keep going. Nothing panics on
//! a cross-thread use. A verb that panics is caught in the slot: the connection is
//! dropped and the handle closed at that moment, and that call and every later
//! one raise `ClosedError` (no `PanicException` reaches Python).

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use bd_cache_model::{CacheError as ModelError, DrainBudget, IdemKey, OPS, ScopeId, Seq};
use pyo3::exceptions::{PyBaseException, PyOSError, PyValueError};
use pyo3::prelude::*;
use sc_bd_cli::argv::ALL_STATUSES;
use sc_beads::{Actor, BeadId, Clock, Deadline};
use serde::Serialize;

use crate::compose::{self, Composition, OpenError, Overrides};
use crate::convert::{json_text, reply, seconds};
use crate::error::{ClosedError, bad_argument, cache_err};
use crate::slot::{Outcome, Slot};

/// The cache of one repository: the SQLite store, the flock drain lock and the `bd` CLI.
///
/// Open it with the repository root. `.beads/bd-cache.toml` supplies the
/// settings; the keyword arguments override the file's keys. Opening starts no
/// `bd` process. One process holds one connection per cache file, so a second
/// open of the same file in the same process raises `CacheError`
/// (`BD_CACHE.STORE_FAILED`) until the first is closed.
///
/// Every verb returns plain Python objects (`dict`, `list`, `str`, `int`,
/// `bool`, `None`) and raises `CacheError` for a typed cache error or
/// `ValueError` for an argument that is not valid (a bead id with whitespace, a
/// negative timeout, a malformed phase name).
#[pyclass(name = "Cache", module = "bd_cache_rs")]
#[derive(Debug)]
pub(crate) struct PyCache {
    // A `Slot` (mutex + closed flag): `Cache` is `!Sync` (ADR-CORE-0001), a
    // pyclass must be `Sync`, and verbs must not interleave on the one store
    // connection. After a panicking verb the slot drops the connection and
    // closes (ADR-PY-0003); it never keeps a half-used connection.
    inner: Slot<Inner>,
    path: PathBuf,
}

struct Inner {
    cache: Composition,
    /// The clock the cache and its backend share; every deadline is minted from it.
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inner").finish_non_exhaustive()
    }
}

/// The `bd_cache_rs` module's version: the wheel's, which versions the reply shapes.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

fn scope(phase: &str) -> PyResult<ScopeId> {
    compose::parse_phase(phase).map_err(|text| PyValueError::new_err(format!("phase: {text}")))
}

/// A positive number of attempts.
fn attempts(n: i64) -> PyResult<u32> {
    u32::try_from(n).ok().filter(|n| *n >= 1).ok_or_else(|| {
        bad_argument(
            "max_attempts",
            format!("{n} is not a whole number of attempts, at least 1"),
        )
    })
}

fn bead_id(bead: &str) -> PyResult<BeadId> {
    BeadId::new(bead).map_err(|e| bad_argument("bead", e))
}

fn parse_actor(actor: &str) -> PyResult<Actor> {
    Actor::new(actor).map_err(|e| bad_argument("actor", e))
}

#[pymethods]
impl PyCache {
    /// Opens the cache of the repository at `repo_root`.
    ///
    /// `bd_binary` is the `bd` program (default `bd`), run in `project_dir`
    /// (default `repo_root`). `scopes`, `cache_path`, `call_timeout_s`,
    /// `miss_fill_timeout_ms` and `max_attempts` override the same keys of
    /// `.beads/bd-cache.toml`.
    #[new]
    #[pyo3(signature = (
        repo_root,
        *,
        bd_binary = "bd",
        project_dir = None,
        scopes = None,
        cache_path = None,
        call_timeout_s = None,
        miss_fill_timeout_ms = None,
        max_attempts = None,
    ))]
    #[expect(
        clippy::too_many_arguments,
        clippy::needless_pass_by_value,
        reason = "the keyword arguments mirror the config keys; PyO3 extracts owned values"
    )]
    fn new(
        py: Python<'_>,
        repo_root: PathBuf,
        bd_binary: &str,
        project_dir: Option<PathBuf>,
        scopes: Option<Vec<String>>,
        cache_path: Option<PathBuf>,
        call_timeout_s: Option<f64>,
        miss_fill_timeout_ms: Option<f64>,
        max_attempts: Option<i64>,
    ) -> PyResult<Self> {
        let scopes = scopes
            .map(|names| {
                names
                    .iter()
                    .map(|n| compose::parse_phase(n))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|text| PyValueError::new_err(format!("scopes: {text}")))
            })
            .transpose()?;
        let overrides = Overrides {
            cache_path,
            scopes,
            call_timeout: call_timeout_s
                .map(|t| seconds("call_timeout_s", t))
                .transpose()?,
            miss_fill_timeout: miss_fill_timeout_ms
                .map(|ms| seconds("miss_fill_timeout_ms", ms / 1000.0))
                .transpose()?,
            max_attempts: max_attempts.map(attempts).transpose()?,
        };
        let opened = py
            .detach(|| compose::open(&repo_root, bd_binary, project_dir.as_deref(), overrides))
            .map_err(|e| open_err(py, e))?;
        Ok(Self {
            path: opened.path,
            inner: Slot::new(Inner {
                cache: opened.cache,
                clock: opened.clock,
            }),
        })
    }

    /// The cache file.
    #[getter]
    fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// True once `close()` has run, or a verb that panicked closed the handle.
    ///
    /// True as soon as `close()` begins, even while a verb on another thread is
    /// still finishing.
    #[getter]
    fn closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Closes the store connection; every later verb raises `ClosedError`.
    ///
    /// The verb in flight on another thread finishes first; the connection is
    /// dropped here with the GIL released.
    fn close(&self, py: Python<'_>) {
        py.detach(|| self.inner.close());
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    #[pyo3(signature = (*_args))]
    fn __exit__(&self, py: Python<'_>, _args: &Bound<'_, pyo3::types::PyTuple>) -> bool {
        self.close(py);
        false
    }

    /// The bead's cached row with pending writes replayed (REQ-BDC-0907).
    ///
    /// On a miss, one bounded `bd list --id` runs: `miss_timeout` seconds, else the
    /// configured miss-fill timeout. Returns `{row: {bead, stale, blocked},
    /// scope_last_ok_at, pending, outage}`.
    #[pyo3(signature = (bead, miss_timeout = None))]
    fn get<'py>(
        &self,
        py: Python<'py>,
        bead: &str,
        miss_timeout: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let id = bead_id(bead)?;
        let timeout = miss_timeout
            .map(|t| seconds("miss_timeout", t))
            .transpose()?;
        self.call(py, move |c| c.cache.get(&id, timeout))
    }

    /// A fresh read of `ids` (one `bd list --id`, all statuses) under `timeout` seconds.
    ///
    /// The rows are stored with force and returned with pending writes replayed, as
    /// a list of `{bead, stale, blocked}` (REQ-BDC-0907).
    #[expect(clippy::needless_pass_by_value, reason = "PyO3 extracts an owned list")]
    fn refresh<'py>(
        &self,
        py: Python<'py>,
        ids: Vec<String>,
        timeout: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let ids = ids
            .iter()
            .map(|i| bead_id(i))
            .collect::<PyResult<Vec<_>>>()?;
        let timeout = seconds("timeout", timeout)?;
        self.call(py, move |c| c.cache.refresh(&ids, timeout))
    }

    /// Journals one write after validating it and checking its precondition.
    ///
    /// `op` is `close`, `update` or `dep_add`. `args` and `precondition` are a
    /// JSON `str` or any object `json.dumps` accepts (`precondition` may be
    /// `None`). Returns `{seq, duplicate}`.
    #[expect(
        clippy::too_many_arguments,
        reason = "the verb's arguments are the BD-CACHE write fields"
    )]
    fn write<'py>(
        &self,
        py: Python<'py>,
        bead: &str,
        op: &str,
        args: &Bound<'py, PyAny>,
        precondition: Option<&Bound<'py, PyAny>>,
        idem_key: &str,
        actor: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let id = bead_id(bead)?;
        if !OPS.contains(&op) {
            return Err(bad_argument(
                "op",
                format!("{op:?} is not an op; use one of {}", OPS.join(", ")),
            ));
        }
        let key = IdemKey::new(idem_key).map_err(|e| bad_argument("idem_key", e))?;
        let who = parse_actor(actor)?;
        let args_json = json_text(py, args)?;
        let pre_json = match precondition {
            Some(p) if !p.is_none() => json_text(py, p)?,
            _ => String::new(),
        };
        let pre = match bd_cache_model::parse_precondition(&pre_json) {
            Ok(g) => g,
            Err(e) => return Err(cache_err(py, &e)),
        };
        let op = op.to_owned();
        self.call(py, move |c| {
            c.cache.write(&id, &op, &args_json, pre, key, who)
        })
    }

    /// The cached rows of a known phase, newest overlay applied.
    ///
    /// Returns `{scope, last_ok_at, rows, outage}`; `status` and `label` filter the rows.
    #[pyo3(signature = (phase, status = None, label = None))]
    fn list<'py>(
        &self,
        py: Python<'py>,
        phase: &str,
        status: Option<String>,
        label: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let s = scope(phase)?;
        if let Some(st) = status
            .as_deref()
            .filter(|st| !ALL_STATUSES.split(',').any(|known| known == *st))
        {
            return Err(bad_argument(
                "status",
                format!("{st:?} is not a bd status; use one of {ALL_STATUSES}"),
            ));
        }
        self.call(py, move |c| {
            c.cache.list(&s, status.as_deref(), label.as_deref())
        })
    }

    /// The phase's ready beads (REQ-BDC-0911, REQ-BDC-0912).
    ///
    /// With `fresh_within` seconds, bd's answer if it arrives in time, else the cached
    /// answer adjusted by journaled writes with `read_fallback` naming why; the call
    /// never takes longer than `fresh_within`. Returns `{scope, last_ok_at, basis,
    /// locally_adjusted, ready, read_fallback, missing_rows}`.
    #[pyo3(signature = (phase, fresh_within = None))]
    fn ready<'py>(
        &self,
        py: Python<'py>,
        phase: &str,
        fresh_within: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let s = scope(phase)?;
        let within = fresh_within
            .map(|t| seconds("fresh_within", t))
            .transpose()?;
        self.call(py, move |c| c.cache.ready(&s, within))
    }

    /// Syncs `phase` (registering it on demand) within `timeout` seconds.
    ///
    /// Returns `{scope, rebuilt, model_current, stored, blocked, deferred, outage}`.
    fn sync<'py>(&self, py: Python<'py>, phase: &str, timeout: f64) -> PyResult<Bound<'py, PyAny>> {
        let s = scope(phase)?;
        let after = seconds("timeout", timeout)?;
        self.call(py, move |c| {
            let d = Deadline::after(c.clock.as_ref(), after);
            c.cache.sync(&s, d)
        })
    }

    /// Applies pending writes in order (REQ-BDC-0402).
    ///
    /// No entry starts after `start_gate_s` seconds from now and no `bd` call runs past
    /// `hard_s` seconds from now. Returns `{busy, applied, retrying, escalated,
    /// deferred, deferred_in_flight, outage}`.
    fn drain<'py>(
        &self,
        py: Python<'py>,
        start_gate_s: f64,
        hard_s: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let gate = seconds("start_gate_s", start_gate_s)?;
        let hard = seconds("hard_s", hard_s)?;
        self.call(py, move |c| {
            let budget = DrainBudget {
                start_gate: Deadline::after(c.clock.as_ref(), gate),
                hard: Deadline::after(c.clock.as_ref(), hard),
            };
            c.cache.drain(&budget)
        })
    }

    /// One journal entry by `seq`, in any state (REQ-BDC-0908).
    fn entry<'py>(&self, py: Python<'py>, seq: i64) -> PyResult<Bound<'py, PyAny>> {
        self.call(py, move |c| c.cache.entry(Seq::new(seq)))
    }

    /// Settles an escalated entry with the lead's `note`; returns `None`.
    fn resolve(&self, py: Python<'_>, seq: i64, note: &str, actor: &str) -> PyResult<()> {
        let who = parse_actor(actor)?;
        let note = note.to_owned();
        self.run(py, move |c| c.cache.resolve(Seq::new(seq), &note, who))
    }

    /// Prunes a phase; returns `{scope, beads_deleted, out_of_scope_swept}`.
    fn prune<'py>(&self, py: Python<'py>, phase: &str) -> PyResult<Bound<'py, PyAny>> {
        let s = scope(phase)?;
        self.call(py, move |c| c.cache.prune(&s))
    }

    /// The store's status: `{meta, journal, phases, escalated, outage, stale}`.
    fn status<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.call(py, |c| c.cache.status())
    }

    /// Escalated entries not yet announced, as journal-entry dicts.
    fn unannounced_escalations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.call(py, |c| c.cache.unannounced_escalations())
    }

    /// Records that the escalation `seq` was announced; returns `None`.
    fn mark_announced(&self, py: Python<'_>, seq: i64) -> PyResult<()> {
        self.run(py, move |c| c.cache.mark_announced(Seq::new(seq)))
    }

    /// The phase's last announced sync-failure cause, or `None`.
    fn sync_alert(&self, py: Python<'_>, phase: &str) -> PyResult<Option<String>> {
        let s = scope(phase)?;
        self.run(py, move |c| c.cache.sync_alert(&s))
    }

    /// Sets the phase's sync-failure cause; `None` clears it. Returns `None`.
    fn set_sync_alert(&self, py: Python<'_>, phase: &str, cause: Option<String>) -> PyResult<()> {
        let s = scope(phase)?;
        self.run(py, move |c| c.cache.set_sync_alert(&s, cause.as_deref()))
    }

    /// Open work for the stop rule: `{pending, escalated_unannounced}`.
    fn open_work<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.call(py, |c| c.cache.open_work())
    }
}

/// Maps an open failure to its Python exception.
///
/// The attributes `kind` and `path` are set from the typed values; if setting one
/// fails, that failure is chained as the exception's `__cause__`.
fn open_err(py: Python<'_>, e: OpenError) -> PyErr {
    let decorate = |err: PyErr, kind: &str, path: Option<&PathBuf>| {
        let set = |value: &Bound<'_, PyBaseException>| -> PyResult<()> {
            value.setattr("kind", kind)?;
            value.setattr("path", path)?;
            Ok(())
        };
        if let Err(cause) = set(err.value(py)) {
            err.set_cause(py, Some(cause));
        }
        err
    };
    match e {
        OpenError::Config { kind, path, text } => decorate(
            PyValueError::new_err(format!("configuration: {text}")),
            kind,
            path.as_ref(),
        ),
        OpenError::Read { path, text } => decorate(PyOSError::new_err(text), "read", Some(&path)),
        OpenError::Cache(e) => cache_err(py, &e),
    }
}

impl PyCache {
    /// Runs `f` with the GIL released and maps a typed error to `CacheError`.
    ///
    /// The lock is taken inside the detached section, so a thread waiting for
    /// another thread's verb does not hold the GIL.
    fn run<T, F>(&self, py: Python<'_>, f: F) -> PyResult<T>
    where
        T: Send,
        F: FnOnce(&mut Inner) -> Result<T, ModelError> + Send,
    {
        match py.detach(|| self.inner.with(f)) {
            Outcome::Done(done) => done.map_err(|e| cache_err(py, &e)),
            Outcome::Closed => Err(ClosedError::new_err(
                "the cache is closed; open a new bd_cache_rs.Cache to keep using it",
            )),
            Outcome::Panicked(text) => Err(ClosedError::new_err(format!(
                "the call failed inside the cache ({text}) and the handle was closed; \
                 open a new bd_cache_rs.Cache to keep using it"
            ))),
            Outcome::Poisoned => Err(ClosedError::new_err(
                "an earlier call failed inside the cache and the handle was closed; \
                 open a new bd_cache_rs.Cache to keep using it",
            )),
        }
    }

    /// As [`run`](Self::run), then converts the reply to Python objects.
    fn call<'py, T, F>(&self, py: Python<'py>, f: F) -> PyResult<Bound<'py, PyAny>>
    where
        T: Send + Serialize,
        F: FnOnce(&mut Inner) -> Result<T, ModelError> + Send,
    {
        let value = self.run(py, f)?;
        reply(py, &value)
    }
}
