//! `status` (REQ-BDC-0603; ADR-BDC-0005).
//!
//! The store's status: the `meta` table, journal counts per `phase:state`,
//! each scope's sync state (with `last_ok_at`) and bead count, every
//! escalated entry until it is resolved, and the outage while one holds.
//! Starts no bd call.

use bd_cache_model::{CacheError, CacheStore, DrainLock, StatusReply};
use sc_beads::CacheBackend;

use crate::Cache;

/// `status()`: see the module docs.
pub(crate) fn status<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
) -> Result<StatusReply, CacheError> {
    Ok(StatusReply {
        store: cache.store.status()?,
    })
}
