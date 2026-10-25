//! `status` and `journal_counts` (REQ-BDC-0603; ADR-BDC-0005; contract §6).
//!
//! The store's status: the `meta` table, journal counts per `phase:state`,
//! each scope's sync state (with `last_ok_at`) and bead count, every
//! escalated entry until it is resolved, the stale rows (a read-back failed;
//! the next sync re-reads them), and the outage while one holds. Starts no
//! bd call.
//!
//! `journal_counts` is the journal by state, in total, per scope and
//! unscoped (`cache status` `journal`): the store's one grouped query
//! ([`CacheStore::journal_counts`]), not a scan of the journal.

use bd_cache_model::{CacheError, CacheStore, DrainLock, JournalCounts, StatusReply};
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

/// `journal_counts()`: see the module docs.
pub(crate) fn journal_counts<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
) -> Result<JournalCounts, CacheError> {
    cache.store.journal_counts()
}
