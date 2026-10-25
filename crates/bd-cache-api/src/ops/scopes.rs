//! `cache scopes ingest` and `cache scopes announce` (ANC-003; REQ-BDC-0007,
//! 0008, 0502, 0705, 0706).

use bd_cache::Cache;
use bd_cache_model::{CacheStore, DrainLock};
use sc_beads::CacheBackend;

use crate::data::{CacheScopesAnnounceData, CacheScopesIngestData, opt_time};
use crate::error::ApiError;
use crate::request::{CacheScopesAnnounce, CacheScopesIngest};

/// `cache scopes ingest`: a consumer's own sync feeds the cache with the
/// built-in sync's rules (`Cache::ingest_scope`); the rows are already strict.
///
/// # Errors
///
/// `BD_CONTRACT` for a row outside the scope's filter or given twice;
/// the store's errors.
pub(crate) fn cache_scopes_ingest<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &CacheScopesIngest,
) -> Result<CacheScopesIngestData, ApiError> {
    let report = cache.ingest_scope(&req.scope, &req.rows, &req.blocked, req.revision())?;
    let ids = |v: Vec<sc_beads::BeadId>| v.into_iter().map(sc_beads::BeadId::into_string).collect();
    Ok(CacheScopesIngestData {
        scope: req.scope.name().to_owned(),
        stored: ids(report.stored),
        skipped_newer: ids(report.skipped_newer),
        status_changed: ids(report.status_changed),
        revision: req.revision.clone(),
    })
}

/// `cache scopes announce <scope> --since TS`: the compare-and-set mark of
/// the scope's current failure as handled; what is not marked is offered to
/// the post-sync hook again.
///
/// # Errors
///
/// The store's errors.
pub(crate) fn cache_scopes_announce<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &CacheScopesAnnounce,
) -> Result<CacheScopesAnnounceData, ApiError> {
    let announced = cache.announce_scope(&req.scope, &req.since)?;
    let state = cache.sync_state(&req.scope)?;
    Ok(CacheScopesAnnounceData {
        scope: req.scope.name().to_owned(),
        announced,
        failing_since: opt_time(state.failing_since.as_ref()),
        notified_since: opt_time(state.notified_since.as_ref()),
    })
}
