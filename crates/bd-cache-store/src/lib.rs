//! The bd-cache SQLite store: [`SqliteStore`] ([`CacheStore`]) and [`FlockDrainLock`] ([`DrainLock`]).
//!
//! The store keeps the cache in one SQLite file per dolt database, the file
//! the BD-CACHE v3 Python writer uses, so either implementation reads what the
//! other wrote (REQ-BDC-0804). The v3 tables are kept unchanged; the
//! extensions are additive tables and nullable columns only, at
//! `user_version` 3 (ADR-STORE-0001 in `docs/bd-cache-store/architecture.md`):
//!
//! - scope registry and per-(scope, bead) membership (ADR-BDC-0001), with
//!   `scope.filter` (NULL = a phase-a scope) and `scope.revision`
//!   (ADR-BDC-0013, ADR-STORE-0006);
//! - a per-scope ready base: blocked rows with `blocked_by`, ready ids,
//!   fetch time, stale mark (ADR-BDC-0002);
//! - `journal.applied_at`, for reads that adjust the ready base, and
//!   `journal.first_offered_at`;
//! - nullable `beads.revision` and `sync_state` backend columns (ADR-BDC-0004),
//!   and `sync_state.failing_since` / `notified_since` (REQ-BDC-0728);
//! - the tables `tick_lease`, `hook_state` and `sc_cron_schedule`, and the
//!   `meta` row `sc_cron_schedule_version` (ADR-STORE-0006, ADR-BDC-0014).
//!
//! Locating the file starts no `bd` process: the name and project id come
//! from `.beads/metadata.json` ([`CacheLocation`], REQ-BDC-0101), and
//! [`load_repo_config`] is the one loader of `.beads/bd-cache.toml`.
//!
//! # Example
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use bd_cache_model::{CacheStore, DrainLock};
//! use bd_cache_store::{SqliteStore, load_repo_config};
//!
//! let dir = tempfile::TempDir::new()?;
//! let root = dir.path();
//! std::fs::create_dir_all(root.join(".beads"))?;
//! std::fs::write(
//!     root.join(".beads").join("metadata.json"),
//!     r#"{"dolt_database": "dev", "project_id": "p-1"}"#,
//! )?;
//! let config = load_repo_config(root)?;
//! // `None`: no `$BEADS_DIR`, whatever the environment holds (S36).
//! let store = SqliteStore::open_with(root, &config, None)?;
//! assert!(store.path().ends_with(std::path::Path::new(".bd-cache").join("dev.bd-cache.sqlite3")));
//! let lock = store.drain_lock();
//! let guard = lock.try_acquire()?;
//! assert!(guard.is_some() && lock.try_acquire()?.is_none());
//! # drop((guard, store));
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]

mod codec;
mod error;
mod location;
mod lock;
mod registry;
mod schedule;
mod schema;
mod store;

#[doc(no_inline)]
pub use bd_cache_model::{BdKey, CacheStore, DrainLock};
#[doc(inline)]
pub use location::{
    BEADS_DIR_ENV, BeadsDirError, BeadsDirOverride, CacheLocation, ConfigLoadError,
    DEFAULT_CACHE_DIR, DatabaseIdentity, DoltDatabase, LoadedConfig, MAX_CONFIG_BYTES, ProjectId,
    REPO_CONFIG_FILE, beads_dir, load_repo_config, load_repo_config_with, read_bounded,
};
#[doc(inline)]
pub use lock::FlockDrainLock;
#[doc(inline)]
pub use schedule::CacheScheduleStore;
#[doc(inline)]
pub use schema::{
    APPLIED_SINCE_SQL, APPLIED_SINCE_UNHINTED_SQL, HAS_ENTRIES_SINCE_SQL,
    HAS_ENTRIES_SINCE_UNHINTED_SQL, SC_CRON_SCHEDULE_VERSION_KEY, SCHEMA_VERSION,
};
#[doc(inline)]
pub use store::{BUSY_TIMEOUT, READ_DATING_THRESHOLD, SqliteStore};
