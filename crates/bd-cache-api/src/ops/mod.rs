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
//! and `cache status` take the file's facts as [`CacheFacts`]. `cache
//! health` also judges the sync rows' state, [`SyncFacts`] (REQ-BDC-0732),
//! which only `bd-cache-sync` can read from the schedule table: it runs
//! through [`execute_health`], which takes those facts as a required
//! argument, and [`execute`] refuses it (ADR-BDC-0015 amendment 1), so no
//! front end can report a sync it did not read. The front ends call
//! `bd_cache_sync::execute_api`, which reads the facts and calls it.

mod beads;
mod manage;
mod scopes;

use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{CacheError, CacheStore, DrainLock, ScopeId, SyncMode, SyncRow};
use sc_beads::{CacheBackend, Timestamp};

use crate::data::Data;
use crate::error::ApiError;
use crate::request::{CacheHealth, Request};

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

/// The cache file's sync rows as `cache health` judges them (REQ-BDC-0732),
/// a required argument of [`execute_health`] (ADR-BDC-0015 amendment 1).
///
/// `bd-cache-sync`'s `sync_facts` builds it from the rows `schedule show`
/// reports (REQ-BDC-0731); the mode is derived from [`SyncFacts::rows`], so
/// no value can contradict itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncFacts {
    /// The rows' state.
    pub rows: SyncRows,
    /// The consecutive failures of the post-sync hook the built-in row names;
    /// `None` when it names none.
    pub hook_failures: Option<u32>,
}

/// The state of the two sync rows (REQ-BDC-0720: at most one enabled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncRows {
    /// The schedule table holds no sync row.
    NoRows,
    /// A sync row exists and none is enabled.
    AllDisabled,
    /// The enabled row.
    Enabled(SyncRowFacts),
}

impl SyncRows {
    /// The sync mode these rows give: the enabled row's, else `none`.
    #[must_use]
    pub fn mode(&self) -> SyncMode {
        match self {
            Self::Enabled(r) => r.row.mode(),
            Self::NoRows | Self::AllDisabled => SyncMode::None,
        }
    }

    /// The enabled row, if any.
    #[must_use]
    pub fn enabled(&self) -> Option<&SyncRowFacts> {
        match self {
            Self::Enabled(r) => Some(r),
            Self::NoRows | Self::AllDisabled => None,
        }
    }
}

/// The enabled sync row, as `cache health` judges it (REQ-BDC-0732).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRowFacts {
    /// Which row.
    pub row: SyncRow,
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
/// (ADR-BDC-0015), and neither is `cache health`, which needs the sync rows'
/// state: [`execute_health`] runs it with that state as a required argument
/// (ADR-BDC-0015 amendment 1).
///
/// # Errors
///
/// The command's errors (contract §5.3, §6), as [`ApiError`]; `INTERNAL`
/// for a front-end verb and for `cache health`.
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
        Request::CacheHealth(_) => {
            return Err(ApiError::internal(
                "cache health judges the sync rows, which bd-cache-api cannot read: run it \
                 through bd_cache_sync::execute_api (bd_cache_api::execute_health with the \
                 SyncFacts), ADR-BDC-0015 amendment 1",
            ));
        }
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

/// Runs a decoded `cache health` with the sync rows' state a front end read
/// through `bd-cache-sync` (REQ-BDC-0732, ADR-BDC-0015 amendment 1).
///
/// `sync` is required: `Err` is the failure reading the rows, which health
/// reports as `degraded` with reason `sync_state_unreadable` and `sync:
/// null`, keeping its state and journal facts; it never fails health.
///
/// # Errors
///
/// As `cache health` (contract §6): the store's errors, and with `--probe`
/// the backend check's other typed errors.
pub fn execute_health<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    request: CacheHealth,
    sync: Result<SyncFacts, CacheError>,
) -> Result<Data, ApiError> {
    Ok(Data::CacheHealth(cache_health(cache, request, sync)?))
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
