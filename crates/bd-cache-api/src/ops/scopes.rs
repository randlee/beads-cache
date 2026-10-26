//! The store-only phase-B verbs: `cache scopes add|set|remove|list|ingest|announce`
//! (ADR-BDC-0013, ADR-BDC-0015, REQ-BDC-0721, 0726, 0728).
//!
//! Each runs over `bd-cache` like the other management verbs. `scopes list`
//! is `cache scopes` (the same reply, with each scope's filter);
//! `scopes remove` is `cache evict` followed by the registry removal, in one
//! store transaction, refused (`PRUNE_BLOCKED`) under `evict`'s rule.

use bd_cache::Cache;
use bd_cache_model::{CacheStore, DrainLock, ScopeFilter};
use sc_beads::CacheBackend;

use crate::data::{ScopesAnnounceData, ScopesChangeData, ScopesIngestData, ScopesRemoveData};
use crate::error::ApiError;
use crate::request::{ScopesAdd, ScopesAnnounce, ScopesIngest, ScopesRemove, ScopesSet};

/// `cache scopes add`.
///
/// # Errors
///
/// `BAD_ARGS` for a registered name or a full registry; `CONFIG_INVALID`
/// for a scope `.beads/bd-cache.toml` defines.
pub(crate) fn scopes_add<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &ScopesAdd,
) -> Result<ScopesChangeData, ApiError> {
    cache.scopes_add(&req.scope)?;
    Ok(change(&req.scope))
}

/// `cache scopes set`.
///
/// # Errors
///
/// `SCOPE_UNKNOWN` for a name that is not registered; `CONFIG_INVALID` for a
/// scope `.beads/bd-cache.toml` defines.
pub(crate) fn scopes_set<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &ScopesSet,
) -> Result<ScopesChangeData, ApiError> {
    cache.scopes_set(&req.scope)?;
    Ok(change(&req.scope))
}

fn change(scope: &bd_cache_model::Scope) -> ScopesChangeData {
    ScopesChangeData {
        scope: scope.id.name().to_owned(),
        filter: scope.filter.as_ref().map(ScopeFilter::parts),
    }
}

/// `cache scopes remove`: `evict` plus the registry removal.
///
/// # Errors
///
/// `SCOPE_UNKNOWN`; `PRUNE_BLOCKED` while the scope has pending or
/// escalated entries; `CONFIG_INVALID` for a configured scope.
pub(crate) fn scopes_remove<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &ScopesRemove,
) -> Result<ScopesRemoveData, ApiError> {
    let r = cache.scopes_remove(&req.scope)?;
    Ok(ScopesRemoveData {
        scope: r.scope.name().to_owned(),
        beads_deleted: r.beads_deleted,
        out_of_scope_swept: r.out_of_scope_swept,
        removed: true,
    })
}

/// `cache scopes ingest` (REQ-BDC-0726).
///
/// # Errors
///
/// `SCOPE_UNKNOWN` for a name that is not registered; `INGEST_INVALID` for
/// an empty snapshot without `allow_empty`; the store's errors. The stored
/// scope is untouched on any error.
pub(crate) fn scopes_ingest<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &ScopesIngest,
) -> Result<ScopesIngestData, ApiError> {
    let r = cache.ingest_scope(&req.scope, &req.snapshot, req.revision.as_deref(), req.allow_empty)?;
    Ok(ScopesIngestData {
        scope: req.scope.name().to_owned(),
        rows: req.snapshot.rows().len(),
        stored: r.stored.len(),
        skipped_newer: r.skipped_newer.len(),
        revision: req.revision.clone(),
    })
}

/// `cache scopes announce` (REQ-BDC-0728).
///
/// # Errors
///
/// `PRECONDITION` when `since` is not the current `failing_since`;
/// `SCOPE_UNKNOWN`.
pub(crate) fn scopes_announce<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &ScopesAnnounce,
) -> Result<ScopesAnnounceData, ApiError> {
    cache.scope_announce(&req.scope, &req.since)?;
    Ok(ScopesAnnounceData {
        scope: req.scope.name().to_owned(),
        since: req.since.as_str().to_owned(),
        notified: true,
    })
}
