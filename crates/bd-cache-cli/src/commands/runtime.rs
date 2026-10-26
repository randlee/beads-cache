//! What the phase-B verbs share: the tokio runtime, the schedule table, the
//! shared cache and the error mapping (ADR-SYNC-0008, ADR-BDC-0015).

use std::sync::{Arc, Mutex};

use bd_cache_api::ApiError;
use bd_cache_model::{CacheError, PreconditionReason};
use bd_cache_store::CacheScheduleStore;
use bd_cache_sync::{Composed, RunContext, SharedCache, StopFlag, SyncContext, SyncError};
use sc_cron::{Scheduler, SystemClock, SystemJitter};
use serde_json::Value;

/// The scheduler over the cache file's schedule table, on the system clock.
pub(crate) type CliScheduler = Scheduler<CacheScheduleStore, SystemClock, SystemJitter>;

/// The real cache, shared by a run's steps.
pub(crate) type CliCache =
    SharedCache<bd_cache_store::SqliteStore, bd_cache_store::FlockDrainLock, sc_bd_cli::BdCli>;

/// A multi-thread runtime for one command; dropped when the command ends.
pub(crate) fn runtime() -> Result<tokio::runtime::Runtime, ApiError> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| ApiError::internal(format!("cannot start the tokio runtime: {e}")))
}

/// The composed cache split into what the sync verbs use.
pub(crate) struct Parts {
    pub(crate) store: Arc<CacheScheduleStore>,
    pub(crate) cache: CliCache,
    pub(crate) cache_path: std::path::PathBuf,
    pub(crate) beads_dir: std::path::PathBuf,
}

/// Opens the schedule table first, so a cache file whose table cannot be
/// served fails with `STORE_FAILED` before any work (ADR-BDC-0014).
pub(crate) fn parts(composed: Composed) -> Result<Parts, ApiError> {
    let store = composed.schedule_store().map_err(|e| ApiError::from(&e))?;
    Ok(Parts {
        store,
        cache_path: composed.cache_path.clone(),
        beads_dir: composed.beads_dir.clone(),
        cache: Arc::new(Mutex::new(composed.cache)),
    })
}

impl Parts {
    /// The scheduler over the table.
    pub(crate) fn scheduler(&self) -> CliScheduler {
        Scheduler::new(
            Arc::clone(&self.store),
            Arc::new(SystemClock),
            Arc::new(SystemJitter),
        )
    }

    /// The sync context; `kill` kills a running hook or script at once.
    pub(crate) fn sync_context(
        &self,
        stop: &StopFlag,
        kill: StopFlag,
    ) -> SyncContext<
        bd_cache_store::SqliteStore,
        bd_cache_store::FlockDrainLock,
        sc_bd_cli::BdCli,
        SystemClock,
    > {
        SyncContext {
            cache: Arc::clone(&self.cache),
            clock: Arc::new(SystemClock),
            cache_path: self.cache_path.clone(),
            beads_dir: Some(self.beads_dir.clone()),
            stop: stop.clone(),
            kill,
        }
    }

    /// The run context of an on-demand tick: a signal stops it and kills its hook.
    pub(crate) fn run_context(&self, stop: &StopFlag) -> RunContext {
        RunContext {
            row: bd_cache_sync::BUILTIN_ROW.to_owned(),
            period: None,
            cache_path: self.cache_path.clone(),
            beads_dir: Some(self.beads_dir.clone()),
            stop: stop.clone(),
            kill: stop.clone(),
        }
    }
}

/// A sync error as the contract's error (REQ-BDC-0729): `PRECONDITION` with
/// `details.reason: "python_sync_enabled"` for an on-demand tick in mode
/// Python (REQ-BDC-0720).
pub(crate) fn sync_error(e: &SyncError) -> ApiError {
    match e {
        SyncError::Cache(c) => ApiError::from(c),
        SyncError::PythonSyncEnabled(detail) => ApiError::from(&CacheError::Precondition {
            detail: detail.clone(),
            unmet: std::collections::BTreeMap::new(),
            reason: PreconditionReason::PreconditionFailed,
            source: None,
        })
        .with_detail("reason", "python_sync_enabled")
        .with_suggested_action("bdc cache sync use-builtin makes the built-in row the sync again"),
        _ => ApiError::internal(format!("unexpected sync error: {e}")),
    }
}

/// A reply as JSON.
pub(crate) fn json<T: serde::Serialize>(v: &T) -> Result<Value, ApiError> {
    serde_json::to_value(v)
        .map_err(|e| ApiError::internal(format!("the reply does not serialize: {e}")))
}
