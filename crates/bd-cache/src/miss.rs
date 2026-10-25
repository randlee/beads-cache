//! Cache-miss fill: one bounded `bd list --id` (REQ-BDC-0202; ADR-BDC-0005).

use std::time::Duration;

use bd_cache_model::{CacheError, CacheStore, CallKind, DrainLock, normalize};
use sc_beads::{Bead, BeadId, CacheBackend, Deadline};

use crate::Cache;

/// Fetches one uncached bead under `timeout` (else the configured
/// `miss_fill_timeout`) and stores it with no scope membership.
///
/// Returns the stored row (no overlay; the caller replays), or `None` when
/// bd does not know the bead. A fill the timeout cuts, or an unreachable
/// backend, is `BD_UNAVAILABLE`; only the latter records an outage.
pub(crate) fn fill<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
    timeout: Option<Duration>,
) -> Result<Option<Bead>, CacheError> {
    let limit = timeout.unwrap_or(cache.config.miss_fill_timeout);
    let hard = Deadline::after(&*cache.clock, limit);
    let rows = crate::call(cache, CallKind::MissFill, hard, |b, p, d| {
        b.list_by_ids(p, std::slice::from_ref(id), d)
    })
    .map_err(|failure| {
        // A cut becomes BD_UNAVAILABLE; an outage already is one (recorded by `call`).
        failure.into_error(format!(
            "bd list --id {id}: no answer within {}ms",
            limit.as_millis()
        ))
    })?;
    let Some(row) = rows.into_iter().find(|r| r.id == *id) else {
        return Ok(None);
    };
    cache
        .store
        .upsert_rows(None, std::slice::from_ref(&row), false)?;
    Ok(Some(normalize(&row)))
}
