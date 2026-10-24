//! `get` and `entry` (REQ-BDC-0001, 0202, 0908).
//!
//! `get` answers a cached bead from the store with its pending entries
//! replayed and starts no bd call (REQ-BDC-0001). A miss makes one bounded
//! `bd list --id` (`miss::fill`, under the caller's timeout or the
//! configured `miss_fill_timeout`) and stores the row with no scope
//! membership; a bead bd does not know either is `NOT_FOUND`. The reply
//! carries the row's `stale` mark, its blocked state from the scope's
//! adjusted ready base (ADR-BDC-0002), the scope's `last_ok_at` and the
//! outage (ADR-BDC-0005).

use std::time::Duration;

use bd_cache_model::{CacheError, CacheStore, DrainLock, GetReply, JournalEntry, Seq};
use sc_beads::{BeadId, CacheBackend};

use crate::Cache;
use crate::ready::{blocked_state, row_view_with, scope_for};

fn not_found(id: &BeadId) -> CacheError {
    CacheError::NotFound {
        detail: format!("bead {id} is in neither the cache nor bd"),
    }
}

/// `get(id, miss_timeout)`: see the module docs.
pub(crate) fn get<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
    miss_timeout: Option<Duration>,
) -> Result<GetReply, CacheError> {
    if let Some(found) = crate::view::current_view(cache, id)? {
        return reply(cache, id, found);
    }
    if crate::miss::fill(cache, id, miss_timeout)?.is_none() {
        return Err(not_found(id));
    }
    // The fill stored the row; replay any entry already journaled for it.
    let found = crate::view::current_view(cache, id)?.ok_or_else(|| not_found(id))?;
    reply(cache, id, found)
}

/// `cached(id)`: the reply `get` gives for a cached bead, `None` on a miss; no bd call.
pub(crate) fn cached<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    id: &BeadId,
) -> Result<Option<GetReply>, CacheError> {
    match crate::view::current_view(cache, id)? {
        Some(found) => reply(cache, id, found).map(Some),
        None => Ok(None),
    }
}

/// The reply for a bead whose view is known.
fn reply<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    id: &BeadId,
    (view, freshness): (sc_beads::Bead, bd_cache_model::Freshness),
) -> Result<GetReply, CacheError> {
    let pending = u32::try_from(cache.store.pending_for(id)?.len()).unwrap_or(u32::MAX);
    let (scope_last_ok_at, blocked) = match scope_for(cache, &view)? {
        Some(s) => (
            cache.store.sync_state(&s)?.last_ok_at,
            blocked_state(cache, &s, &view)?,
        ),
        None => (None, None),
    };
    Ok(GetReply {
        row: row_view_with(view, freshness, blocked),
        scope_last_ok_at,
        pending,
        outage: cache.store.outage()?,
    })
}

/// `entry(seq)`: one journal entry in any state (REQ-BDC-0908).
pub(crate) fn entry<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    seq: Seq,
) -> Result<JournalEntry, CacheError> {
    cache
        .store
        .entry_by_seq(seq)?
        .ok_or_else(|| CacheError::NotFound {
            detail: format!("journal entry {seq} is absent"),
        })
}
