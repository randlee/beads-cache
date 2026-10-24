//! The bd-cache SQLite store: [`SqliteStore`] ([`CacheStore`]) and [`FlockDrainLock`] ([`DrainLock`]).
//!
//! The store keeps the cache in one SQLite file per dolt database, the file
//! BD-CACHE v3's `bd_cache.py` uses, so either implementation reads what the
//! other wrote (REQ-BDC-0804). The v3 tables are kept unchanged; the
//! extensions are additive tables and nullable columns only, at
//! `user_version` 3 (ADR-STORE-0001 in `docs/bd-cache-store/architecture.md`):
//!
//! - scope registry and per-(scope, bead) membership (ADR-BDC-0001);
//! - a per-scope ready base: blocked rows with `blocked_by`, ready ids,
//!   fetch time, stale mark (ADR-BDC-0002);
//! - `journal.applied_at`, for reads that adjust the ready base;
//! - nullable `beads.revision` and `sync_state` backend columns (ADR-BDC-0004).
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
//! let root = std::env::temp_dir().join(format!("bdc-doc-lib-{}", std::process::id()));
//! std::fs::create_dir_all(root.join(".beads"))?;
//! std::fs::write(
//!     root.join(".beads/metadata.json"),
//!     r#"{"dolt_database": "atm-dev", "project_id": "p-1"}"#,
//! )?;
//! let config = load_repo_config(&root)?;
//! let store = SqliteStore::open(&root, &config)?;
//! assert!(store.path().ends_with(".bd-cache/atm-dev.bd-cache.sqlite3"));
//! let lock = store.drain_lock();
//! let guard = lock.try_acquire()?;
//! assert!(guard.is_some() && lock.try_acquire()?.is_none());
//! # drop((guard, store));
//! # std::fs::remove_dir_all(&root)?;
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]

mod codec;
mod error;
mod location;
mod lock;
mod registry;
mod schema;
mod store;

#[doc(no_inline)]
pub use bd_cache_model::{CacheStore, DrainLock};
#[doc(inline)]
pub use location::{
    BEADS_DIR_ENV, CacheLocation, ConfigLoadError, DEFAULT_CACHE_DIR, DatabaseIdentity,
    DoltDatabase, ProjectId, REPO_CONFIG_FILE, beads_dir, load_repo_config,
};
#[doc(inline)]
pub use lock::FlockDrainLock;
#[doc(inline)]
pub use schema::SCHEMA_VERSION;
#[doc(inline)]
pub use store::{BUSY_TIMEOUT, SqliteStore};
