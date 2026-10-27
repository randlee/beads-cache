//! Python bindings for bd-cache (maturin / `PyO3`): the native module `bd_cache_rs._native`.
//!
//! The package `bd_cache_rs` is maturin's mixed layout (ADR-PY-0006): this
//! crate is its native module `_native`, and `python/bd_cache_rs/` holds
//! `__init__.py` (re-exporting every name, with the same `__all__`,
//! `__version__` and `CONTRACT_VERSION`), `run.py` (`python -m
//! bd_cache_rs.run post_sync <module:function>`, the post-sync hook's entry)
//! and `sync.py` (`bd_cache_rs.sync.main(func)`, a consumer's sync script's
//! harness). `import bd_cache_rs` and every exported name work unchanged.
//!
//! The module exposes the `Cache` class over the concrete composition
//! `Cache<SqliteStore, FlockDrainLock, BdCli>` (REQ-BDC-0910). It is a thin
//! layer: no cache rule lives here. The 0.1 verbs are `get`, `refresh`,
//! `write`, `list`, `ready`, `sync`, `drain`, `entry`, `resolve`, `prune`,
//! `status` and the announcement state `unannounced_escalations`,
//! `mark_announced`, `sync_alert`, `set_sync_alert` and `open_work`.
//!
//! - Replies are the `bd-cache-model` reply types converted to builtin Python
//!   objects (`convert`); their shapes are versioned with the wheel.
//! - A typed error raises `bd_cache_rs.CacheError` carrying `code`, `detail`,
//!   `fix`, `reason` and the payload (`error`); a bad argument raises
//!   `ValueError`.
//! - Since 0.2 `cache.beads.*` and `cache.manage.*` run the contract's commands over
//!   `bd-cache-api` (`api`): the request is the keyword arguments, goes through
//!   `bd_cache_api::decode` and `execute`, and the reply is the command's `data`.
//!   `bd_cache_rs.CONTRACT_VERSION` is the contract version they implement.
//! - The phase-B methods (`tick`, `schedule_*`, `sync_use_*`, `ingest_scope`,
//!   `scope_announce`, `scopes_*`; `phase_b`) run on the handle's own
//!   connection; `tick` and the schedule and switch methods over
//!   `bd-cache-sync` (ADR-PY-0006). `bd_cache_rs.CODES` lists every code.
//! - The handle sits behind a mutex, so any thread may use it, and releases the
//!   GIL while a verb waits on `bd` or SQLite (`handle`; ADR-CORE-0001,
//!   ADR-PY-0003).
//!
//! Documentation: `docs/bd-cache-py/requirements.md` and `architecture.md`.
//!
//! This crate is a `PyO3` extension module: build it with maturin
//! (`crates/bd-cache-py/pyproject.toml`). Its Rust tests need a Python
//! interpreter to link against, so the workspace `ci.yml` leaves the crate out
//! and `.github/workflows/python.yml` is its gate.

#![deny(missing_docs)]

mod api;
mod compose;
mod convert;
mod error;
mod handle;
mod phase_b;
mod slot;

use pyo3::prelude::*;

/// bd-cache in process: the cache of a beads repository (REQ-BDC-0910).
#[pymodule]
#[pyo3(name = "_native")]
fn native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<handle::PyCache>()?;
    error::register(m)?;
    api::register(m)?;
    m.add("__version__", handle::VERSION)?;
    m.add("UNREACHABLE", error::unreachable(m.py())?)?;
    m.add("CODES", error::codes(m.py())?)?;
    m.add_function(wrap_pyfunction!(phase_b::set_in_run, m)?)?;
    Ok(())
}
