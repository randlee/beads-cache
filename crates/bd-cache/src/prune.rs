//! `prune` of a scope (REQ-BDC-0602, 0902).
//!
//! An unregistered scope is `SCOPE_UNKNOWN`, and nothing is pruned or swept
//! (ADR-BDC-0013). Refused with `PRUNE_BLOCKED` while the scope has a pending or escalated
//! entry; otherwise the store deletes the scope's beads that no other scope
//! holds (with their edges), its journal entries, sync state and ready base,
//! and sweeps out-of-scope rows (a `get` or `write` miss outside every
//! scope) that have no open entry. The scope stays registered. The rules
//! are the store's (`CacheStore::prune_scope`).

use bd_cache_model::{CacheError, CacheStore, DrainLock, PruneReport, ScopeId};
use sc_beads::CacheBackend;

use crate::Cache;
use crate::ready::require_known;

/// `prune(scope)`: see the module docs.
pub(crate) fn prune<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
) -> Result<PruneReport, CacheError> {
    require_known(cache, s)?;
    cache.store.prune_scope(s)
}
