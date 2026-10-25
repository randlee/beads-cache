//! `list` of a known scope (REQ-BDC-0203, 0902, 0909).
//!
//! The scope's cached rows, by id, each with its pending entries replayed
//! and filtered on that view (`status`, `label`), so a list agrees with
//! `get`. A scope never configured or synced is `SCOPE_UNKNOWN` naming the
//! known scopes; it is never answered from a partial cache. Starts no bd call.

use bd_cache_model::{CacheError, CacheStore, DrainLock, ListReply, ScopeId, replay};
use sc_beads::CacheBackend;

use crate::Cache;
use crate::ready::{adjusted, require_known, row_view};

/// `list(scope, status, label)`: see the module docs.
pub(crate) fn list<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    s: &ScopeId,
    status: Option<&str>,
    label: Option<&str>,
) -> Result<ListReply, CacheError> {
    require_known(cache, s)?;
    let adj = adjusted(cache, &cache.scope(s))?;
    let mut rows = Vec::new();
    for (row, freshness) in cache.store.rows_in_scope(s)? {
        let view = replay(&row, &cache.store.pending_for(&row.id)?);
        if status.is_some_and(|want| view.status.as_deref() != Some(want)) {
            continue;
        }
        if label.is_some_and(|want| !view.has_label(want)) {
            continue;
        }
        rows.push(row_view(view, freshness, adj.as_ref()));
    }
    Ok(ListReply {
        scope: s.clone(),
        last_ok_at: cache.store.sync_state(s)?.last_ok_at,
        rows,
        outage: cache.store.outage()?,
    })
}
