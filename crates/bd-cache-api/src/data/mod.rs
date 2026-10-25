//! The `data` of every command (contract §5.3, §6), and [`Data`], one of them.

mod beads;
mod entry;
mod manage;
mod x_cache;

use serde::Serialize;

#[doc(inline)]
pub use beads::{
    ContextData, DependenciesAddData, DependenciesListData, IssuesCloseData, IssuesGetData,
    IssuesListData, IssuesUpdateData, ReadyData,
};
#[doc(inline)]
pub use entry::Entry;
#[doc(inline)]
pub use manage::{
    BackendData, CacheEvictData, CacheFlushData, CacheHealthData, CacheRefreshIssuesData,
    CacheRefreshScopesData, CacheScopesData, CacheStatusData, EscalatedData, HealthReason,
    HealthState, JournalAnnounceData, JournalCountsData, JournalGetData, JournalListData,
    JournalResolveData, OpenWorkData, ProbeData, ProjectData, RefreshError, RefreshScopeResult,
    ScopeData, StateCountsData, SyncReportData,
};
#[doc(inline)]
pub use x_cache::{
    Basis, ContextXCache, DepsXCache, GetXCache, OpError, OutageData, ReadFallback, ReadXCache,
    ReadyXCache, RowFacts, Rows, RowsXCache, WriteXCache, cache_time,
};

pub(crate) use x_cache::{opt_time, outage, rows};

/// The `data` of one command; serializes as that command's `data` alone.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Data {
    /// `issues get`.
    IssuesGet(Box<IssuesGetData>),
    /// `issues list`.
    IssuesList(IssuesListData),
    /// `ready`.
    Ready(ReadyData),
    /// `dependencies list`.
    DependenciesList(DependenciesListData),
    /// `issues update`.
    IssuesUpdate(Box<IssuesUpdateData>),
    /// `issues close`.
    IssuesClose(Box<IssuesCloseData>),
    /// `dependencies add`.
    DependenciesAdd(DependenciesAddData),
    /// `context`.
    Context(ContextData),
    /// `cache status`.
    CacheStatus(CacheStatusData),
    /// `cache health`.
    CacheHealth(CacheHealthData),
    /// `cache scopes`.
    CacheScopes(CacheScopesData),
    /// `cache refresh scopes`.
    CacheRefreshScopes(CacheRefreshScopesData),
    /// `cache refresh issues`.
    CacheRefreshIssues(CacheRefreshIssuesData),
    /// `cache flush`.
    CacheFlush(CacheFlushData),
    /// `cache evict`.
    CacheEvict(CacheEvictData),
    /// `cache journal get`.
    JournalGet(JournalGetData),
    /// `cache journal list`.
    JournalList(JournalListData),
    /// `cache journal resolve`.
    JournalResolve(JournalResolveData),
    /// `cache journal announce`.
    JournalAnnounce(JournalAnnounceData),
}

impl Data {
    /// The `data` as JSON.
    ///
    /// # Errors
    ///
    /// Never in practice: every member is a JSON-safe value. A serializer
    /// error is returned as `INTERNAL`.
    pub fn to_json(&self) -> Result<serde_json::Value, crate::ApiError> {
        serde_json::to_value(self)
            .map_err(|e| crate::ApiError::internal(format!("data does not serialize: {e}")))
    }
}
