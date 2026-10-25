//! The bd-cache sync job: the built-in bd sync on `sc-cron`, consumer Python
//! sync rows and the post-sync hook (REQ-BDC-0701..0706 amended; ADR-BDC-0010).
//!
//! beads-cache syncs bd itself, at the times set in the cron table of its
//! cache file (ANC-002), and sends no messages (ADR-BDC-0010). This crate
//! composes the `bd-cache` core, the SQLite store (`bd-cache-store`), the `bd`
//! CLI backend (`sc-bd-cli`) and the scheduler (`sc-cron`):
//!
//! - [`run_sync`]: one run of the built-in sync within its budget `T` (the
//!   row's timeout): a drain with `DrainBudget { start_gate: T - 5 s, hard:
//!   T - 2 s }`, then each configured scope, a bd filter (REQ-BDC-0702),
//!   synced with deadline `T`, deferring any not started by `T - 2 s`
//!   (REQ-BDC-0703); the [`SyncRun`] carries each scope's result and error;
//! - [`schedule`](crate::register_builtin): the rows of the cron table. At
//!   most one built-in sync row per repository ([`register_builtin`] refuses a
//!   second, ANC-008); other rows are allowed (ANC-005). A consumer disables
//!   the built-in row ([`set_builtin_enabled`], ANC-006) and starts its own
//!   Python row ([`python_spec`]) that feeds the cache through
//!   `ingest_scope` (ANC-003);
//! - [`SyncJob`]: the `sc-cron` [`Job`](sc_cron::Job) that runs both kinds of
//!   row, the sync under `spawn_blocking` with the `Cache` in an
//!   `Arc<Mutex<_>>` (ADR-CORE-0001), then the optional post-sync hook with
//!   the run's results and errors (ANC-004, [`post_sync_request`]);
//! - [`compose`]: the real `SqliteStore` + `FlockDrainLock` + `BdCli`
//!   cache of a repository, read from `.beads/bd-cache.toml`, over one
//!   process-wide `bd` lock set.
//!
//! Every bd call, Python sync run and hook run is killed at its timeout
//! capped by the time left before the run's deadline (NFR-BDC-0004).
//!
//! # Codes
//!
//! Every error this crate reports has a stable lower-case code
//! (`ScheduleError::code`, `ComposeError::code`, `CacheLost::code`), with the
//! `BD_CACHE.*` codes of the core passed through where the cause is a
//! `CacheError`. The code, not the message, is what a program matches on.
//!
//! # One runner per cache file
//!
//! Either `sc-cron` runs this crate's job over the cache file, or the
//! built-in row is run on demand (`bdc cache tick`). Never two runners on
//! one cache file at once.
//!
//! # Example
//!
//! ```
//! use bd_cache_sync::{BUILTIN_NAME, builtin_spec};
//!
//! let spec = builtin_spec(Some("*/30 * * * * *"), None, None).unwrap();
//! assert_eq!(spec.name.as_str(), BUILTIN_NAME);
//! ```

#![deny(missing_docs)]

mod compose;
mod control;
mod hook;
mod job;
mod report;
mod run;
mod schedule;
mod table;
mod text;

#[cfg(all(test, unix))]
mod test_script {
    use std::path::Path;

    /// Writes an executable test script through a `sh` child, so this
    /// multi-threaded test process never holds a write fd on it that a fork in a
    /// concurrent test could inherit and turn the exec into ETXTBSY (os error 26);
    /// a rename would not help, because it keeps the same inode.
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

#[doc(inline)]
pub use compose::{
    ComposeError, Composed, RealCache, compose, compose_with, compose_with_config,
    compose_with_config_and_backend,
};
#[doc(inline)]
pub use control::StopFlag;
#[doc(inline)]
pub use hook::{HookRun, MAX_HOOK_ESCALATIONS, POST_SYNC_KIND, post_sync_request, run_hook};
#[doc(inline)]
pub use job::{BuiltInRun, CacheLost, JobCacheError, JobRun, SyncJob, builtin_outcome};
#[doc(inline)]
pub use report::{ScopeError, ScopeResult, SyncRun};
#[doc(inline)]
pub use run::{DEFAULT_BUDGET, run_sync};
#[doc(inline)]
pub use schedule::{
    BUILTIN_KIND, BUILTIN_NAME, DEFAULT_EXPR, DEFAULT_HOOK_TIMEOUT, HookSpec, PYTHON_KIND,
    RowPayload, ScheduleError, builtin_spec, clear_builtin, cron_db_path, find_builtin,
    python_spec, register_builtin, set_builtin, set_builtin_enabled,
};
#[doc(inline)]
pub use table::{BuiltInRow, CronTable, TickError, TickReport, tick_once};
