//! Python bindings for bd-cache (maturin / `PyO3`): the `bd_cache_rs` module.
//!
//! The module exposes the `Cache` class over the concrete composition
//! `Cache<SqliteStore, FlockDrainLock, BdCli>` so atm-bd can call the cache in
//! process, without `sc-runtime`, from its own scheduler (REQ-BDC-0910). It is
//! a thin layer: no cache rule lives here. The verbs are `get`, `refresh`,
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
//! - The handle sits behind a mutex, so any thread may use it, and releases the
//!   GIL while a verb waits on `bd` or SQLite (`handle`; ADR-CORE-0001,
//!   ADR-PY-0003).
//!
//! Documentation: `docs/bd-cache-py/requirements.md` and `architecture.md`,
//! including the mapping from each `bd_cache.py` function atm-bd calls today.
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
mod slot;

use pyo3::prelude::*;

/// bd-cache in process: the cache of a beads repository (REQ-BDC-0910).
#[pymodule]
fn bd_cache_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<handle::PyCache>()?;
    error::register(m)?;
    api::register(m)?;
    m.add("__version__", handle::VERSION)?;
    m.add("UNREACHABLE", error::unreachable(m.py())?)?;
    Ok(())
}
