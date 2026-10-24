//! `current_view`: the stored row with the bead's pending entries replayed (REQ-BDC-0202).

use bd_cache_model::{CacheError, CacheStore, DrainLock, Freshness, replay};
use sc_beads::{Bead, BeadId, CacheBackend};

use crate::Cache;

/// The bead as reads see it: the stored row with its `Pending` entries
/// overlaid in seq order, and the row's freshness. `None` when the bead is
/// not cached (the caller decides whether to fill the miss). Starts no bd call.
pub(crate) fn current_view<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    id: &BeadId,
) -> Result<Option<(Bead, Freshness)>, CacheError> {
    let Some((row, freshness)) = cache.store.get_row(id)? else {
        return Ok(None);
    };
    let pending = cache.store.pending_for(id)?;
    Ok(Some((replay(&row, &pending), freshness)))
}
