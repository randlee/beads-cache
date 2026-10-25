//! Surface (a) operations (contract §5.2, §5.3).

use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{CacheStore, DrainLock, IdemKey, RowView, ScopeId};
use sc_beads::{Actor, Assignee, BeadId, CacheBackend, Guards};
use serde_json::json;

use crate::b1_stub::{self, WriteOutcome};
use crate::data::{
    Basis, ContextData, ContextXCache, DependenciesAddData, DependenciesListData, DepsXCache,
    GetXCache, IssuesCloseData, IssuesGetData, IssuesListData, IssuesUpdateData, OutageData,
    ReadFallback, ReadXCache, ReadyData, ReadyXCache, WriteXCache,
};
use crate::error::ApiError;
use crate::filter::{list_keeps, ready_keeps, select_scope};
use crate::ops::{CacheFacts, known_scopes};
use crate::page::{ListSort, page, sort_rows};
use crate::request::{
    DependenciesAdd, DependenciesList, IssuesClose, IssuesGet, IssuesList, IssuesUpdate, Ready,
};
use crate::v0::patch_args;
use crate::{CAPABILITIES, CONTRACT_VERSION};

/// The scope a list-like read names, or `SCOPE_UNKNOWN` naming the known ones.
fn required_scope<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    scope: Option<&ScopeId>,
    labels: &[String],
) -> Result<ScopeId, ApiError> {
    match select_scope(scope, labels) {
        Some(s) => Ok(s),
        None => Err(ApiError::scope_required(&known_scopes(cache)?)),
    }
}

/// `issues get` (getIssue): the cached row with its pending entries
/// replayed; a miss waits at most `miss_timeout_ms` for bd, and `0` never
/// asks bd (`NOT_CACHED`).
///
/// # Errors
///
/// `NOT_FOUND`, `NOT_CACHED`, `BD_UNAVAILABLE` (`details.timed_out`), and
/// the store's errors.
pub fn issues_get<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &IssuesGet,
) -> Result<IssuesGetData, ApiError> {
    let reply = match req.miss_timeout_ms {
        Some(0) => cache
            .cached(&req.id)?
            .ok_or_else(|| ApiError::not_cached(req.id.as_str()))?,
        ms => cache.get(&req.id, ms.map(Duration::from_millis))?,
    };
    let scope = b1_stub::get_scope(cache, &reply)?;
    let read = ReadXCache::new(
        scope.as_ref(),
        reply.scope_last_ok_at.as_ref(),
        reply.outage.as_ref(),
        std::slice::from_ref(&reply.row),
    );
    Ok(IssuesGetData {
        issue: reply.row.bead,
        x_cache: GetXCache {
            read,
            pending: reply.pending,
        },
    })
}

/// `issues list` (listIssues): one known scope, filtered, ordered and paged
/// here (REQ-BDC-0808).
///
/// # Errors
///
/// `SCOPE_UNKNOWN` without a scope or for one the cache does not know; the
/// store's errors.
pub fn issues_list<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &IssuesList,
) -> Result<IssuesListData, ApiError> {
    let scope = required_scope(cache, req.scope.as_ref(), &req.label)?;
    let reply = cache.list(&scope, None, None)?;
    let mut rows: Vec<RowView> = reply
        .rows
        .into_iter()
        .filter(|r| list_keeps(req, &r.bead))
        .collect();
    sort_rows(req.sort, &mut rows);
    let p = page(req.sort, rows, req.limit, req.cursor.as_ref());
    let x_cache = ReadXCache::new(
        Some(&scope),
        reply.last_ok_at.as_ref(),
        reply.outage.as_ref(),
        &p.items,
    );
    Ok(IssuesListData {
        items: p.items.into_iter().map(|r| r.bead).collect(),
        has_more: p.has_more,
        next_cursor: p.next_cursor,
        x_cache,
    })
}

/// `ready` (listReadyWork, membership from the bd CLI): the core's answer,
/// filtered and ordered `(priority asc, created_at desc, id asc)`.
///
/// # Errors
///
/// `SCOPE_UNKNOWN`; the store's errors.
pub fn ready<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &Ready,
) -> Result<ReadyData, ApiError> {
    let scope = required_scope(cache, req.scope.as_ref(), &req.label)?;
    let reply = cache.ready(&scope, req.fresh_within_ms.map(Duration::from_millis))?;
    let outage = cache.status()?.store.outage;
    let mut rows: Vec<RowView> = reply
        .ready
        .into_iter()
        .filter(|r| ready_keeps(req, &r.bead))
        .collect();
    sort_rows(ListSort::Priority, &mut rows);
    let p = page(ListSort::Priority, rows, req.limit, None);
    let (read_fallback, read_fallback_error) = match reply.read_fallback.as_deref() {
        Some(text) => {
            let (kind, error) = ReadFallback::of(text);
            (Some(kind), Some(error))
        }
        None => (None, None),
    };
    let x_cache = ReadyXCache {
        read: ReadXCache::new(
            Some(&scope),
            reply.last_ok_at.as_ref(),
            outage.as_ref(),
            &p.items,
        ),
        basis: Basis::of(&reply.basis),
        locally_adjusted: reply.locally_adjusted,
        read_fallback,
        read_fallback_error,
        missing_rows: reply.missing_rows,
    };
    Ok(ReadyData {
        items: p.items.into_iter().map(|r| r.bead).collect(),
        has_more: p.has_more,
        x_cache,
    })
}

/// `dependencies list` (listDependencies): each cached bead's outgoing
/// edges, in request order of `issue_id`, then by `depends_on_id`. Never
/// calls bd: an uncached bead is listed in `missing` and `x_cache.not_cached`.
///
/// # Errors
///
/// The store's errors.
pub fn dependencies_list<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &DependenciesList,
) -> Result<DependenciesListData, ApiError> {
    let mut items = Vec::new();
    let mut missing: Vec<BeadId> = Vec::new();
    for id in &req.issue_id {
        let Some(reply) = cache.cached(id)? else {
            if !missing.contains(id) {
                missing.push(id.clone());
            }
            continue;
        };
        let mut edges: Vec<_> = reply
            .row
            .bead
            .dependencies
            .into_iter()
            .filter(|e| e.issue_id == *id)
            .filter(|e| req.types.is_empty() || req.types.contains(&e.kind))
            .collect();
        edges.sort_by(|a, b| {
            (a.depends_on_id.as_str(), a.kind.as_str())
                .cmp(&(b.depends_on_id.as_str(), b.kind.as_str()))
        });
        items.extend(edges);
    }
    let outage = cache.status()?.store.outage;
    Ok(DependenciesListData {
        items,
        missing: missing.clone(),
        x_cache: DepsXCache {
            outage: outage.as_ref().map(OutageData::of),
            not_cached: missing,
        },
    })
}

fn guards(status: Option<&String>, assignee: Option<&String>) -> Guards {
    Guards {
        status: status.cloned(),
        assignee: assignee.map(|a| Assignee::from_wire(a)),
    }
}

/// One journaled write through `write_returning`.
fn write<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    id: &BeadId,
    op: &str,
    args: &serde_json::Value,
    pre: Guards,
    key: &IdemKey,
    actor: &Actor,
) -> Result<WriteOutcome, ApiError> {
    b1_stub::write_returning(
        cache,
        id,
        op,
        &args.to_string(),
        pre,
        key.clone(),
        actor.clone(),
    )
    .map_err(ApiError::from)
}

fn write_x_cache(o: &WriteOutcome) -> WriteXCache {
    WriteXCache {
        seq: o.entry.seq.get(),
        state: o.entry.state.as_str(),
        duplicate: o.duplicate,
        idempotency_key: o.entry.idem_key.as_str().to_owned(),
    }
}

/// `issues update` (updateIssue), journaled: the view after the write and
/// `changed`, the overlay's prediction (on a duplicate, the stored value).
///
/// # Errors
///
/// `BAD_ARGS`, `BAD_PRECONDITION`, `PRECONDITION` (`details.unmet`),
/// `IDEM_CONFLICT`, `NOT_FOUND`, `BD_UNAVAILABLE`; the store's errors.
pub fn issues_update<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &IssuesUpdate,
) -> Result<IssuesUpdateData, ApiError> {
    let pre = guards(req.expected_status.as_ref(), req.expected_assignee.as_ref());
    let o = write(
        cache,
        &req.id,
        "update",
        &patch_args(&req.patch),
        pre,
        &req.idempotency_key,
        &req.actor,
    )?;
    Ok(IssuesUpdateData {
        x_cache: write_x_cache(&o),
        changed: o.changed,
        issue: o.after.bead,
    })
}

/// `issues close` (closeIssue), journaled: `already_closed` describes the
/// view before the write (on a duplicate, the stored value).
///
/// # Errors
///
/// As [`issues_update`].
pub fn issues_close<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &IssuesClose,
) -> Result<IssuesCloseData, ApiError> {
    let pre = guards(req.expected_status.as_ref(), req.expected_assignee.as_ref());
    let args = match &req.reason {
        Some(r) => json!({"reason": r}),
        None => json!({}),
    };
    let o = write(
        cache,
        &req.id,
        "close",
        &args,
        pre,
        &req.idempotency_key,
        &req.actor,
    )?;
    Ok(IssuesCloseData {
        x_cache: write_x_cache(&o),
        // A close stores `changed = !already_closed` (lead ruling S62).
        // and an unknown `changed` leaves it unknown, so absent (S68).
        already_closed: o.changed.map(|c| !c),
        issue: o.after.bead,
    })
}

/// `dependencies add` (addDependencies), journaled: exactly one edge.
///
/// # Errors
///
/// As [`issues_update`].
pub fn dependencies_add<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    req: &DependenciesAdd,
) -> Result<DependenciesAddData, ApiError> {
    let e = &req.edge;
    let args = json!({"depends_on": e.depends_on_id, "type": e.kind.as_str()});
    let o = write(
        cache,
        &e.issue_id,
        "dep_add",
        &args,
        Guards::default(),
        &req.idempotency_key,
        &req.actor,
    )?;
    Ok(DependenciesAddData {
        added: vec![e.clone()],
        x_cache: write_x_cache(&o),
    })
}

/// `context` (getContext): the project from `facts`, the recorded
/// fingerprint, the capabilities served and [`CONTRACT_VERSION`].
///
/// # Errors
///
/// The store's errors.
pub fn context<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    facts: &CacheFacts,
) -> Result<ContextData, ApiError> {
    let meta = cache.status()?.store.meta;
    let sha = meta.get(META_SHA).cloned();
    Ok(ContextData {
        bd_version: meta.get(META_BD_VERSION).cloned(),
        database: facts.database.clone(),
        project_id: facts.project_id.clone(),
        beads_dir: facts.beads_dir.clone(),
        repo_root: facts.repo_root.clone(),
        capabilities: CAPABILITIES.to_vec(),
        x_cache: ContextXCache {
            cache_path: facts.cache_path.clone(),
            contract_version: CONTRACT_VERSION,
            store_schema_version: facts.store_schema_version,
            model_current: sha.as_deref().map(|s| s == sc_beads::BD_SCHEMA_SHA256),
            bd_schema_sha256: sha,
            bd_schema_version: meta.get(META_SCHEMA_VERSION).cloned(),
        },
    })
}

/// The `meta` keys of the recorded fingerprint (both stores write them).
pub(crate) const META_SHA: &str = "bd_schema_sha256";
/// bd's version in `meta`.
pub(crate) const META_BD_VERSION: &str = "bd_version";
/// `bd schema`'s version in `meta`.
pub(crate) const META_SCHEMA_VERSION: &str = "bd_schema_version";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_take_the_wire_assignee() {
        let g = guards(Some(&"open".to_owned()), Some(&String::new()));
        assert_eq!(g.status.as_deref(), Some("open"));
        assert_eq!(g.assignee, Some(Assignee::Unassigned));
    }
}
