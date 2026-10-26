//! The bd-cache sync: the built-in and Python sync rows on `sc-cron`, the
//! switch between them, and the post-sync hook (ADR-SYNC-0008, ADR-BDC-0011).
//!
//! beads-cache syncs bd on rows of the sc-cron table inside the cache file
//! (ADR-BDC-0014): at most one built-in row ([`BUILTIN_ROW`], a Rust job) and
//! at most one Python row ([`PYTHON_ROW`], a consumer's script), at most one
//! of them enabled (REQ-BDC-0720). The cache sends nothing (ADR-BDC-0010):
//! a post-sync hook the built-in row names gets every result and error and
//! decides what to tell anyone (REQ-BDC-0727).
//!
//! - [`run_builtin`]: the built-in row's run (lease, drain, every scope by its
//!   filter, post-sync hook, reply); `cron run`, `cron run --once`,
//!   `bdc cache tick` and `PyO3` `Cache.tick()` all run it (REQ-BDC-0720);
//! - [`TickLease`]: one sync at a time across processes (REQ-BDC-0722);
//! - [`sync_resolver`]: sc-cron's per-row target (REQ-CRON-0016), returning a
//!   [`BuiltinSyncJob`] or a [`PythonSyncJob`] (REQ-BDC-0725);
//! - [`schedule_set`], [`schedule_show`], [`schedule_enable`],
//!   [`schedule_clear`], [`sync_use_python`], [`sync_use_builtin`] and
//!   [`tick`]: the mode and the switch (REQ-BDC-0720, 0724, 0731);
//! - [`cron_run`] and [`cron_run_once`] (REQ-BDC-0730);
//! - [`TrustedCacheFile`]: the trust check before any row's or hook's code
//!   runs (REQ-BDC-0723);
//! - [`compose`]: the real `SqliteStore` + `FlockDrainLock` + `BdCli` cache of
//!   a repository, read from `.beads/bd-cache.toml`, with its schedule table.
//!
//! The sync reaches the backend only through bd (REQ-BDC-0006); every bd
//! call, hook and script run is bounded by its deadline (NFR-BDC-0004).
//!
//! # Example
//!
//! ```
//! use bd_cache_sync::{BUILTIN_ROW, PYTHON_ROW, SyncPayload};
//!
//! assert_eq!(BUILTIN_ROW, "bd-cache.sync.builtin");
//! assert_eq!(PYTHON_ROW, "bd-cache.sync.python");
//! let payload = serde_json::json!({"target": "builtin", "budget_ms": 25000, "hook_reserve_ms": 5000});
//! assert!(matches!(SyncPayload::decode(&payload), Ok(SyncPayload::Builtin(_))));
//! ```

#![deny(missing_docs)]

mod compose;
mod control;
mod cron;
mod history;
mod hook;
mod job;
mod lease;
mod payload;
mod python_job;
mod run;
mod schedule;
mod text;
mod trust;

#[doc(inline)]
pub use compose::{
    ComposeError, Composed, RealCache, compose, compose_with, compose_with_config,
    compose_with_config_and_backend,
};
#[doc(inline)]
pub use control::{Budget, StopFlag};
#[doc(inline)]
pub use cron::{
    CronStopped, DEFAULT_SHUTDOWN_TIMEOUT, MAX_SHUTDOWN_TIMEOUT, cron_run, cron_run_once,
};
#[doc(inline)]
pub use history::RowHistory;
#[doc(inline)]
pub use hook::{
    DrainSummary, HOOK_BACKOFF_CAP, HOOK_BACKOFF_FROM, HookSuccess, MAX_ESCALATIONS_OFFERED,
    MAX_HOOK_PIPE_BYTES, MAX_HOOK_REQUEST_BYTES, RUN_PROTOCOL, ScopeError, ScopeOutcome,
    ScopeResult, next_attempt,
};
#[doc(inline)]
pub use job::{BuiltinSyncJob, SyncContext, join_idle, sync_resolver};
#[doc(inline)]
pub use lease::{LEASE_MARGIN, LeaseTaken, TickLease, new_run_id};
#[doc(inline)]
pub use payload::{
    BUILTIN_ROW, BuiltinPayload, DEFAULT_PYTHON_TIMEOUT_MS, EnvNames, IMPORT_CHECK_TIMEOUT,
    MAX_ENV_NAMES, MAX_PYTHON_TIMEOUT_MS, MIN_PYTHON_TIMEOUT_MS, PYTHON_ROW, PostSyncHook,
    PythonPayload, SyncPayload, TrustedInterpreter,
};
#[doc(inline)]
pub use python_job::{PythonRun, PythonSyncJob};
#[doc(inline)]
pub use run::{
    BuiltinReply, HookReport, HookStep, RunContext, RunStatus, SharedCache, run_builtin,
};
#[doc(inline)]
pub use schedule::{
    Rows, ScheduleSet, SyncError, SyncMode, UsePython, check_schedule_set, row_period,
    schedule_clear, schedule_enable, schedule_set, schedule_show, sync_use_builtin,
    sync_use_python, tick,
};
#[doc(inline)]
pub use text::MAX_KEPT_STDERR_BYTES;
#[doc(inline)]
pub use trust::{TrustedCacheFile, Untrusted};
