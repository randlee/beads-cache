//! The operations: one function per command over `&mut Cache<S, L, B>` (contract §9).
//!
//! Each returns its command's `data` or an [`ApiError`]; a front end wraps
//! the value without reshaping it (contract §8). The functions are
//! crate-private: the only public entry is [`execute`], which takes a
//! [`Request`] that only [`decode`](crate::decode) can build, so every front
//! end's input passes the decoder's validation (REQ-API-0001).
//!
//! The writes go through `write_returning`, one store transaction that
//! returns the entry and the views before and after (REQ-BDC-0809);
//! `cache journal get` reads by key with `entry_by_key`; `cache status`
//! counts with `journal_counts`. The api never opens a file, so `context`
//! and `cache status` take the file's facts as [`CacheFacts`].

mod beads;
mod manage;
mod scopes;

use bd_cache::Cache;
use bd_cache_model::{CacheStore, DrainLock, ScopeId};
use sc_beads::CacheBackend;

use crate::data::Data;
use crate::error::ApiError;
use crate::request::Request;

pub(crate) use beads::{
    context, dependencies_add, dependencies_list, issues_close, issues_get, issues_list,
    issues_update, ready,
};
pub(crate) use manage::{
    cache_evict, cache_flush, cache_health, cache_refresh_issues, cache_refresh_scopes,
    cache_scopes, cache_status, journal_announce, journal_get, journal_list, journal_resolve,
};
pub(crate) use scopes::{scopes_add, scopes_announce, scopes_ingest, scopes_remove, scopes_set};

/// What a front end knows of the cache file and `metadata.json` (the api
/// opens neither).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CacheFacts {
    /// The cache file.
    pub cache_path: String,
    /// The store's schema version (`user_version`).
    pub store_schema_version: i64,
    /// `dolt_database` from `metadata.json`.
    pub database: String,
    /// `project_id` from `metadata.json`.
    pub project_id: String,
    /// The beads directory, when known.
    pub beads_dir: Option<String>,
    /// The repository root, when known.
    pub repo_root: Option<String>,
}

/// Runs one decoded request.
///
/// The verbs a front end runs over `bd-cache-sync` ([`Command::is_front_end`]:
/// `cache schedule *`, `cron run`, `tick`, `sync *`) are not run here
/// (ADR-BDC-0015).
///
/// # Errors
///
/// The command's errors (contract §5.3, §6), as [`ApiError`]; `INTERNAL`
/// for a front-end verb.
///
/// [`Command::is_front_end`]: crate::Command::is_front_end
pub fn execute<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    request: &Request,
    facts: &CacheFacts,
) -> Result<Data, ApiError> {
    Ok(match request {
        Request::IssuesGet(r) => Data::IssuesGet(Box::new(issues_get(cache, r)?)),
        Request::IssuesList(r) => Data::IssuesList(issues_list(cache, r)?),
        Request::Ready(r) => Data::Ready(ready(cache, r)?),
        Request::DependenciesList(r) => Data::DependenciesList(dependencies_list(cache, r)?),
        Request::IssuesUpdate(r) => Data::IssuesUpdate(Box::new(issues_update(cache, r)?)),
        Request::IssuesClose(r) => Data::IssuesClose(Box::new(issues_close(cache, r)?)),
        Request::DependenciesAdd(r) => Data::DependenciesAdd(dependencies_add(cache, r)?),
        Request::Context(_) => Data::Context(context(cache, facts)?),
        Request::CacheStatus(_) => Data::CacheStatus(cache_status(cache, facts)?),
        Request::CacheHealth(r) => Data::CacheHealth(cache_health(cache, *r)?),
        Request::CacheScopes(_) | Request::ScopesList(_) => Data::CacheScopes(cache_scopes(cache)?),
        Request::CacheRefreshScopes(r) => Data::CacheRefreshScopes(cache_refresh_scopes(cache, r)?),
        Request::CacheRefreshIssues(r) => Data::CacheRefreshIssues(cache_refresh_issues(cache, r)?),
        Request::CacheFlush(r) => Data::CacheFlush(cache_flush(cache, r)?),
        Request::CacheEvict(r) => Data::CacheEvict(cache_evict(cache, r)?),
        Request::JournalGet(r) => Data::JournalGet(journal_get(cache, r)?),
        Request::JournalList(r) => Data::JournalList(journal_list(cache, *r)?),
        Request::JournalResolve(r) => Data::JournalResolve(journal_resolve(cache, r)?),
        Request::JournalAnnounce(r) => Data::JournalAnnounce(journal_announce(cache, *r)?),
        Request::ScopesAdd(r) => Data::ScopesChange(scopes_add(cache, r)?),
        Request::ScopesSet(r) => Data::ScopesChange(scopes_set(cache, r)?),
        Request::ScopesRemove(r) => Data::ScopesRemove(scopes_remove(cache, r)?),
        Request::ScopesIngest(r) => Data::ScopesIngest(scopes_ingest(cache, r)?),
        Request::ScopesAnnounce(r) => Data::ScopesAnnounce(scopes_announce(cache, r)?),
        Request::ScheduleSet(_)
        | Request::ScheduleShow(_)
        | Request::ScheduleEnable(_)
        | Request::ScheduleDisable(_)
        | Request::ScheduleClear(_)
        | Request::CronRun(_)
        | Request::CacheTick(_)
        | Request::SyncUsePython(_)
        | Request::SyncUseBuiltin(_) => {
            return Err(ApiError::internal(format!(
                "{} runs in the front end over bd-cache-sync, not in bd-cache-api",
                request.command().path()
            )));
        }
    })
}

/// Every scope the cache knows, by id.
fn known_scopes<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
) -> Result<Vec<ScopeId>, ApiError> {
    let mut known: Vec<ScopeId> = cache
        .status()?
        .store
        .scopes
        .into_iter()
        .map(|s| s.sync.scope)
        .collect();
    known.sort();
    known.dedup();
    Ok(known)
}
