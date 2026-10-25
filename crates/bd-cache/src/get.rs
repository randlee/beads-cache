//! `get`, `entry` and `entry_by_key` (REQ-BDC-0001, 0202, 0908).
//!
//! `get` answers a cached bead from the store with its pending entries
//! replayed and starts no bd call (REQ-BDC-0001). A miss makes one bounded
//! `bd list --id` (`miss::fill`, under the caller's timeout or the
//! configured `miss_fill_timeout`) and stores the row with no scope
//! membership; a bead bd does not know either is `NOT_FOUND`. The reply
//! carries the row's `stale` mark, its blocked state from the scope's
//! adjusted ready base (ADR-BDC-0002), the bead's scope (`GetReply.scope`)
//! with its `last_ok_at`, and the outage (ADR-BDC-0005).
//!
//! The scope is the one the blocked state is read in ([`scope_for`]): the
//! view's phase when that scope is known, else the first known scope that
//! holds the view; asking it reads the scope registry, never the scope's
//! members, so a cached `get` stays within NFR-BDC-0001.

use std::time::Duration;

use bd_cache_model::{
    CacheError, CacheStore, DrainLock, Freshness, GetReply, IdemKey, JournalEntry, RowView,
    ScopeId, Seq,
};
use sc_beads::{Bead, BeadId, CacheBackend};

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
    (view, freshness): (Bead, Freshness),
) -> Result<GetReply, CacheError> {
    let pending = u32::try_from(cache.store.pending_for(id)?.len()).unwrap_or(u32::MAX);
    let (row, scope) = row_in_scope(cache, view, freshness)?;
    let scope_last_ok_at = match &scope {
        Some(s) => cache.store.sync_state(s)?.last_ok_at,
        None => None,
    };
    Ok(GetReply {
        row,
        scope_last_ok_at,
        pending,
        outage: cache.store.outage()?,
        scope,
    })
}

/// A view as a read returns it, with the scope its blocked state was read in.
///
/// The row `get` answers for this view: the blocked state comes from the
/// view's scope ([`scope_for`], [`blocked_state`]), `None` outside every scope.
pub(crate) fn row_in_scope<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    view: Bead,
    freshness: Freshness,
) -> Result<(RowView, Option<ScopeId>), CacheError> {
    let scope = scope_for(cache, &view)?;
    let blocked = match &scope {
        Some(s) => blocked_state(cache, s, &view)?,
        None => None,
    };
    Ok((
        row_view_with(view, freshness, blocked),
        scope.map(|s| s.id().clone()),
    ))
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

/// `entry_by_key(key)`: the entry an idempotency key journals, in any state (REQ-BDC-0908).
pub(crate) fn entry_by_key<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    key: &IdemKey,
) -> Result<JournalEntry, CacheError> {
    cache
        .store
        .entry_by_key(key)?
        .ok_or_else(|| CacheError::NotFound {
            detail: format!("no journal entry holds idempotency key {key}"),
        })
}
