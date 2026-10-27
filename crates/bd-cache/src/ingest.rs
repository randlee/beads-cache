//! `ingest_scope`: a snapshot a caller fetched through bd, stored with the
//! built-in sync's rules (REQ-BDC-0726, REQ-BDC-0007, 0008, 0502; ANC-003).
//!
//! A consumer's Python sync, which runs while the built-in sync row is
//! disabled, fills the cache only through this call, so it replaces the
//! built-in bd sync with no bd process of beads-cache's. The rules:
//!
//! 1. the snapshot is already checked ([`IngestSnapshot`]: the strict row
//!    decoder, at most 50 000 rows and 64 MiB, no bead listed twice), so one
//!    invalid row among valid ones stores no row (REQ-BDC-0008);
//! 2. an empty snapshot is `INGEST_INVALID` (`empty`) unless `allow_empty`;
//!    the scope must be registered (`SCOPE_UNKNOWN` otherwise: only a sync
//!    registers on demand, ADR-BDC-0013), which the store decides inside its
//!    transaction, with the registered filter, so a concurrent `scopes
//!    remove` or `scopes set` is seen;
//! 3. membership is declared by the snapshot: every listed row the scope's
//!    status criterion admits is a member, and nothing else; other scopes'
//!    memberships are kept;
//! 4. the built-in `blocked` rule: the ready base is the snapshot's blocked
//!    rows and the members `bd ready` would list (bd's rule,
//!    [`sc_beads::cache::is_bd_ready`]: open, not blocked, not deferred into
//!    the future), both restricted to the members, as the built-in sync
//!    intersects bd's answer with the members;
//! 5. one store transaction ([`CacheStore::ingest_scope`]): rows unforced
//!    (the newer-row guard, REQ-BDC-0502), membership, base, the sync state
//!    as an ok result (`failing_since` cleared) and the opaque `revision`.
//!
//! The store and the core open no Dolt or MySQL connection and read no Dolt
//! tables (REQ-BDC-0006); no bd call is made.

use std::collections::BTreeSet;

use bd_cache_model::{
    CacheError, CacheStore, DrainLock, IngestReason, IngestSnapshot, ScopeId, UpsertReport,
};
use sc_beads::cache::is_bd_ready;
use sc_beads::{Bead, BeadId, BlockedRow, CacheBackend};

use crate::Cache;

/// `ingest_scope(scope, snapshot, revision, allow_empty)`: see the module docs.
pub(crate) fn ingest<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
    snapshot: &IngestSnapshot,
    revision: Option<&str>,
    allow_empty: bool,
) -> Result<UpsertReport, CacheError> {
    if snapshot.is_empty() && !allow_empty {
        return Err(CacheError::IngestInvalid {
            detail: format!(
                "ingest of scope {s}: the snapshot lists no row; pass allow_empty to store an \
                 empty scope"
            ),
            scope: s.clone(),
            reason: IngestReason::Empty,
            index: None,
        });
    }
    let now = cache.clock.now_utc();
    let ready = ready_ids(snapshot.rows(), snapshot.blocked(), now.unix());
    cache
        .store
        .ingest_scope(s, snapshot, &ready, &now, revision)
}

/// The rows `bd ready` would list: bd's rule ([`is_bd_ready`]) with
/// "blocked" read as listed in `blocked` with a blocker (ADR-BDC-0002). The
/// store keeps those that are members.
fn ready_ids(rows: &[Bead], blocked: &[BlockedRow], now_unix: i64) -> Vec<BeadId> {
    let blocked: BTreeSet<&BeadId> = blocked
        .iter()
        .filter(|b| !b.blocked_by.is_empty())
        .map(|b| &b.id)
        .collect();
    rows.iter()
        .filter(|r| is_bd_ready(r, blocked.contains(&r.id), now_unix))
        .map(|r| r.id.clone())
        .collect()
}
