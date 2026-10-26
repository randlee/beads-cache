//! The `data` of every command (contract §5.3, §6), and [`Data`], one of them.

mod beads;
mod entry;
mod manage;
mod sync;
mod x_cache;

use serde::Serialize;

/// The sync mode and the sync row of `cache health`'s `sync` member: the
/// one enum of each, `bd-cache-model`'s, which `bd-cache-sync`'s `schedule
/// show` serializes too.
#[doc(inline)]
pub use bd_cache_model::{SyncMode, SyncRow};
#[doc(inline)]
pub use beads::{
    ContextData, DependenciesAddData, DependenciesListData, IssuesCloseData, IssuesGetData,
    IssuesListData, IssuesUpdateData, ReadyData,
};
#[doc(inline)]
pub use entry::{Entry, EntryOp, EntryState};
#[doc(inline)]
pub use manage::{
    BackendData, CacheEvictData, CacheFlushData, CacheHealthData, CacheRefreshIssuesData,
    CacheRefreshScopesData, CacheScopesData, CacheStatusData, EscalatedData, HealthReason,
    HealthReasonCode, HealthState, JournalAnnounceData, JournalCountsData, JournalGetData,
    JournalListData, JournalResolveData, LastRunData, OpenWorkData, ProbeData, ProjectData,
    RefreshError, RefreshScopeResult, ScopeData, StateCountsData, SyncHealthData, SyncReportData,
};
#[doc(inline)]
pub use sync::{ScopesAnnounceData, ScopesChangeData, ScopesIngestData, ScopesRemoveData};
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
    /// `cache scopes add` and `cache scopes set`.
    ScopesChange(ScopesChangeData),
    /// `cache scopes remove`.
    ScopesRemove(ScopesRemoveData),
    /// `cache scopes ingest`.
    ScopesIngest(ScopesIngestData),
    /// `cache scopes announce`.
    ScopesAnnounce(ScopesAnnounceData),
}

impl Data {
    /// The `data` as JSON.
    ///
    /// # Panics
    ///
    /// Never: every member is a JSON-safe value (string-keyed maps, no
    /// non-finite float is ever built), so `serde_json` cannot fail on it; a
    /// failure would be a programming error of this crate (M-PANIC-ON-BUG).
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "invariant: every Data member serializes (string keys, JSON-safe values)"
    )]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("bd-cache-api data always serializes to JSON")
    }
}
