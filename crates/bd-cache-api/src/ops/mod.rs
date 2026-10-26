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
//! and `cache status` take the file's facts as [`CacheFacts`], and
//! `cache health` the sync rows' state as [`SyncFacts`] (REQ-BDC-0732),
//! which the front end reads from the schedule table through `bd-cache-sync`.

mod beads;
mod manage;
mod scopes;

use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{CacheStore, DrainLock, ScopeId};
use sc_beads::{CacheBackend, Timestamp};

use crate::data::{Data, SyncModeData};
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
    /// The sync rows' state, for `cache health` (REQ-BDC-0732); `None` when
    /// the front end did not read the schedule table (health then reports
    /// `sync: null` and no sync reason).
    pub sync: Option<SyncFacts>,
}

/// What a front end read of the cache file's sync rows for `cache health`
/// (REQ-BDC-0732); `bd-cache-sync`'s `sync_facts` builds it from the rows
/// `schedule show` reports (REQ-BDC-0731).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncFacts {
    /// The mode the rows' flags give.
    pub mode: SyncModeData,
    /// Whether a sync row exists, enabled or not.
    pub has_rows: bool,
    /// The enabled row; `None` in mode `none`.
    pub enabled: Option<SyncRowFacts>,
    /// The consecutive failures of the post-sync hook the built-in row names;
    /// `None` when it names none.
    pub hook_failures: Option<u32>,
}

/// The enabled sync row, as `cache health` judges it (REQ-BDC-0732).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRowFacts {
    /// Which row: `builtin` or `python`.
    pub row: &'static str,
    /// When the row was written or last replaced (sc-cron's `created_at`).
    pub created_at: Timestamp,
    /// One period of the row at evaluation (REQ-BDC-0727): the gap between
    /// its next two scheduled times; `None` when it has no next run, or none
    /// after it.
    pub period: Option<Duration>,
    /// When its last recorded run finished.
    pub last_run_at: Option<Timestamp>,
    /// That run's outcome: `ok`, or the failure's text.
    pub outcome: Option<String>,
    /// The latest run's id.
    pub run_id: Option<String>,
    /// Consecutive failed runs, the latest included.
    pub consecutive_failures: u32,
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
        Request::CacheHealth(r) => Data::CacheHealth(cache_health(cache, *r, facts)?),
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
