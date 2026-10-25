//! `ingest_scope`: a consumer's own sync feeds the cache (REQ-BDC-0007,
//! 0008, 0502; ANC-003).
//!
//! A consumer that disables the built-in sync row and runs its own sync
//! (a registered Python method) hands the rows it read to the cache here.
//! The rules are the built-in sync's:
//!
//! 1. every row is one the scope's filter selects, and no id is given twice,
//!    else `BD_CONTRACT` names the bead and the field and **nothing** is
//!    stored (REQ-BDC-0008); the rows are strict [`Bead`]s already (a front
//!    end decodes JSON with the strict row decoder, REQ-BDC-0007);
//! 2. the rows are stored unforced, so an older row never replaces a newer
//!    stored one (REQ-BDC-0502), with membership from the whole filter;
//! 3. the ready base is `blocked` and the members that `bd ready` would
//!    list: `open`, not blocked, not deferred into the future;
//! 4. rows, base and sync state go in one store transaction
//!    ([`CacheStore::ingest_scope`]); the sync state is a pass, so the
//!    scope's failure state is cleared.
//!
//! No bd call is made.

use std::collections::BTreeSet;

use bd_cache_model::{CacheError, CacheStore, DrainLock, Scope, UpsertReport};
use sc_beads::{Bead, BeadId, BlockedRow, CacheBackend, rfc3339_to_unix};

use crate::Cache;
use crate::sync::check_scope_rows;

/// `ingest_scope(scope, rows, blocked, revision)`: see the module docs.
pub(crate) fn ingest<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    scope: &Scope,
    rows: &[Bead],
    blocked: &[BlockedRow],
    revision: Option<&str>,
) -> Result<UpsertReport, CacheError> {
    let mut seen: BTreeSet<&BeadId> = BTreeSet::new();
    if let Some(twice) = rows.iter().find(|r| !seen.insert(&r.id)) {
        return Err(CacheError::BdContract {
            bead: Some(twice.id.clone()),
            field: "id".into(),
            detail: format!(
                "the ingest of scope {} gives {} twice",
                scope.id(),
                twice.id
            ),
            source: None,
        });
    }
    check_scope_rows(scope, rows)?;
    let now = cache.clock.now_utc();
    let ready = ready_ids(scope, rows, blocked, now.unix());
    cache
        .store
        .ingest_scope(scope, rows, blocked, &ready, &now, revision)
}

/// The members `bd ready` would list: `open`, not in `blocked`, and not
/// deferred past `now_unix` (the fake bd's and bd's rule, ADR-BDC-0002).
fn ready_ids(scope: &Scope, rows: &[Bead], blocked: &[BlockedRow], now_unix: i64) -> Vec<BeadId> {
    let blocked: BTreeSet<&BeadId> = blocked.iter().map(|b| &b.id).collect();
    rows.iter()
        .filter(|r| scope.holds(r) && r.status.as_deref() == Some("open"))
        .filter(|r| !blocked.contains(&r.id))
        .filter(|r| {
            r.defer_until
                .as_deref()
                .and_then(rfc3339_to_unix)
                .is_none_or(|t| t <= now_unix)
        })
        .map(|r| r.id.clone())
        .collect()
}
