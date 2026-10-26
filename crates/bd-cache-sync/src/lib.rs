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
//! The front ends (`bd-cache-cli`, `bd-cache-py`) call one surface:
//!
//! - [`compose`] and its forms ([`compose_with_config`], [`compose_tuned`]):
//!   the real `SqliteStore` + `FlockDrainLock` + `BdCli` cache of a
//!   repository, read from `.beads/bd-cache.toml`, with its schedule table,
//!   over one process-wide lock set;
//! - [`SystemSchedules`] ([`Schedules`] on the system clock): `schedule
//!   set|show|enable|disable|clear`, `sync use-python|use-builtin`, `tick`,
//!   `cron run` and `cron run --once` (REQ-BDC-0720, 0724, 0730, 0731);
//! - [`execute`] with a [`FrontEnd`]: a decoded front-end verb of the
//!   contract run over it, the request mapped to its inputs and every error
//!   to the contract's once (ADR-BDC-0015);
//! - [`execute_api`]: a decoded command `bd-cache-api` runs, with `cache
//!   health` given the sync rows' state [`sync_facts`] reads now
//!   (REQ-BDC-0732, ADR-BDC-0015 amendment 1), so no front end can leave it
//!   out;
//! - [`StopFlag`], the flag a signal sets.
//!
//! Inside: `run_builtin` (the built-in row's run: lease, drain, every scope
//! by its filter, post-sync hook, reply), `TickLease` (one sync at a time
//! across processes, REQ-BDC-0722), `sync_resolver` (sc-cron's per-row
//! target, REQ-CRON-0016, returning the built-in or the Python row's job,
//! REQ-BDC-0725) and `TrustedCacheFile` (the trust check before any row's
//! or hook's code runs, REQ-BDC-0723). They are public only with the
//! `testing` feature, which this crate's own dev-dependency on itself
//! enables, for its integration tests, and hidden from its documentation.
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
mod front_end;
mod health;
mod history;
mod hook;
mod job;
mod lease;
mod mode_lock;
mod payload;
mod python_job;
mod run;
mod schedule;
mod text;
mod trust;

#[cfg(all(test, unix))]
mod test_script {
    use std::path::Path;

    /// Writes an executable test script through a `sh` child, so this
    /// multi-threaded test process never holds a write fd on it that a fork in
    /// a concurrent test could inherit and turn the exec into ETXTBSY.
    #[expect(
        clippy::unwrap_used,
        reason = "test-only helper; a failure fails the test"
    )]
    pub(crate) fn write(path: &Path, contents: &str) {
        let status = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("printf '%s' \"$1\" > \"$2\" && chmod 755 \"$2\"")
            .arg("sh")
            .arg(contents)
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success(), "cannot write {}", path.display());
    }
}

// The front ends' surface.
#[doc(inline)]
pub use compose::{
    ComposeError, ComposeTuning, Composed, RealCache, ScheduleTable, compose, compose_tuned,
    compose_with, compose_with_config, compose_with_config_and_backend,
};
#[doc(inline)]
pub use control::StopFlag;
#[doc(inline)]
pub use cron::{CronOnce, CronStopped, DueRunView};
#[doc(inline)]
pub use front_end::{FrontEnd, execute};
#[doc(inline)]
pub use health::{execute_api, sync_facts};
#[doc(inline)]
pub use history::RowHistory;
#[doc(inline)]
pub use hook::{ScopeError, ScopeOutcome, ScopeResult};
#[doc(inline)]
pub use job::SyncFiles;
#[doc(inline)]
pub use payload::{BUILTIN_ROW, PYTHON_ROW, SyncRow};
#[doc(inline)]
pub use run::{BuiltinReply, HolderView, HookReport, HookStep, RunStatus};
#[doc(inline)]
pub use schedule::{
    HookView, PayloadView, RowView, RowsView, ScheduleClearReply, ScheduleEnableReply, ScheduleSet,
    ScheduleSetReply, ScheduleShow, Schedules, ScopeView, ShowFlag, SyncError, SyncMode,
    SystemSchedules, UseBuiltinReply, UsePython, UsePythonReply,
};

// The parts the integration tests drive directly (module docs): behind the
// `testing` feature, which this crate's own dev-dependency on itself enables,
// so no dependent crate reaches them (PE1-RBQA-F007).
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use control::Budget;
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use hook::{
    DrainSummary, HOOK_BACKOFF_CAP, HOOK_BACKOFF_FROM, HookSuccess, MAX_ESCALATIONS_OFFERED,
    MAX_HOOK_PIPE_BYTES, MAX_HOOK_REQUEST_BYTES, RUN_PROTOCOL, next_attempt,
};
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use job::{BuiltinSyncJob, SyncContext, join_idle, sync_resolver};
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use lease::{LEASE_MARGIN, LeaseTaken, TickLease, new_run_id};
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use mode_lock::MODE_LOCK_WAIT;
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use payload::{
    BuiltinPayload, DEFAULT_PYTHON_TIMEOUT_MS, EnvNames, IMPORT_CHECK_TIMEOUT, MAX_ENV_NAMES,
    MAX_PYTHON_TIMEOUT_MS, MIN_PYTHON_TIMEOUT_MS, PostSyncHook, PythonPayload, SyncPayload,
    TrustedInterpreter,
};
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use python_job::{PythonRun, PythonSyncJob};
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use run::{RunContext, SharedCache, run_builtin};
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use schedule::Rows;
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use text::MAX_KEPT_STDERR_BYTES;
#[cfg(feature = "testing")]
#[doc(hidden)]
pub use trust::{TrustedCacheFile, Untrusted};
