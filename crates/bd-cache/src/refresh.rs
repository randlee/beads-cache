//! `refresh`: a fresh read stored with force, overlay applied (REQ-BDC-0907).
//!
//! One `bd list --id` of the requested beads (all statuses) through `call`
//! (`CallKind::Refresh`) under the caller's timeout, past whatever the cache
//! holds. The rows are checked as a whole before any is stored (REQ-BDC-0008),
//! stored forced with their scope membership unchanged, and returned in
//! request order with their pending entries replayed. Ids bd does not know
//! are absent from the reply.

use std::collections::BTreeMap;
use std::time::Duration;

use bd_cache_model::{CacheError, CacheStore, CallKind, DrainLock, RowView, ScopeId};
use sc_beads::{Bead, BeadId, CacheBackend, Deadline};

use crate::Cache;
use crate::ready::{Adjusted, adjusted, row_view, scope_for};

/// `BD_CONTRACT` when bd answered an id list with a bead that was not asked
/// for; checked before any row of the answer is stored.
pub(crate) fn check_requested(ids: &[BeadId], rows: &[Bead]) -> Result<(), CacheError> {
    match rows.iter().find(|r| !ids.contains(&r.id)) {
        Some(extra) => Err(CacheError::BdContract {
            bead: Some(extra.id.clone()),
            field: "id".into(),
            detail: format!(
                "bd list --id returned {}, which was not requested",
                extra.id
            ),
            source: None,
        }),
        None => Ok(()),
    }
}

/// `refresh(ids, timeout)`: see the module docs.
pub(crate) fn refresh<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    ids: &[BeadId],
    timeout: Duration,
) -> Result<Vec<RowView>, CacheError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let hard = Deadline::after(&*cache.clock, timeout);
    let rows = crate::call(cache, CallKind::Refresh, hard, |b, p, d| {
        b.list_by_ids(p, ids, d)
    })
    .map_err(|failure| {
        failure.into_error(format!(
            "bd list --id ({} beads): no answer within {}ms",
            ids.len(),
            timeout.as_millis()
        ))
    })?;
    check_requested(ids, &rows)?;
    cache.store.upsert_rows(None, &rows, true)?;
    let mut bases: BTreeMap<ScopeId, Option<Adjusted>> = BTreeMap::new();
    let mut out: Vec<RowView> = Vec::with_capacity(rows.len());
    for id in ids {
        if out.iter().any(|v| v.bead.id == *id) || !rows.iter().any(|r| r.id == *id) {
            continue;
        }
        let (row, view, freshness) =
            crate::view::current_row(cache, id)?.ok_or_else(|| CacheError::NotFound {
                detail: format!("bead {id} was stored by refresh but the store does not return it"),
            })?;
        let adj = match scope_for(cache, &view)? {
            Some(s) => {
                if !bases.contains_key(&s.id) {
                    let a = adjusted(cache, &s)?;
                    bases.insert(s.id.clone(), a);
                }
                bases.get(&s.id).and_then(Option::as_ref)
            }
            None => None,
        };
        out.push(row_view(Some(&row), view, freshness, adj));
    }
    Ok(out)
}
