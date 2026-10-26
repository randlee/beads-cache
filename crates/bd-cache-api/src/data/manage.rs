//! Surface (b) `data`: the `cache` commands (contract §6).
//!
//! Every member is always present, and `null` means unknown (contract §1
//! rule 3). Scopes appear in label form (`phase-<x>`).

use std::collections::BTreeMap;

use sc_beads::{Bead, BeadId};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::data::entry::Entry;
use crate::data::x_cache::{OpError, OutageData, RowsXCache};
use crate::error::{ErrorKind, spec};

/// `cache status` `project`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectData {
    /// The dolt database.
    pub database: String,
    /// The project id.
    pub project_id: String,
}

/// `cache status` `backend`: the recorded fingerprint, `null` before the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackendData {
    /// bd's version.
    pub bd_version: Option<String>,
    /// The `bd schema` sha.
    pub bd_schema_sha256: Option<String>,
    /// The `bd schema` version.
    pub bd_schema_version: Option<String>,
}

/// Entry counts by state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct StateCountsData {
    /// Waiting for the drain.
    pub pending: u64,
    /// Landed in bd.
    pub applied: u64,
    /// Sent to the lead.
    pub escalated: u64,
    /// Settled by hand.
    pub resolved: u64,
}

/// `cache status` `journal`: the totals, then per scope and unscoped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JournalCountsData {
    /// The totals.
    #[serde(flatten)]
    pub total: StateCountsData,
    /// Per scope, by scope id.
    pub by_scope: BTreeMap<String, StateCountsData>,
    /// Entries with no scope.
    pub unscoped: StateCountsData,
}

/// A Scope (`cache scopes`, `cache status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeData {
    /// The scope's name, e.g. `bk` (ADR-BDC-0013).
    pub scope: String,
    /// Listed in the configuration.
    pub configured: bool,
    /// Registered by a sync, not configured.
    pub on_demand: bool,
    /// Start of the latest sync.
    pub last_started_at: Option<String>,
    /// Start of the latest successful sync.
    pub last_ok_at: Option<String>,
    /// The latest sync failure.
    pub last_error: Option<OpError>,
    /// `backend_unavailable`, `sync_failed`, a `BD_CACHE` code, or `null`.
    pub alert: Option<String>,
    /// Beads the scope holds.
    pub beads: u64,
}

/// `cache status`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CacheStatusData {
    /// The cache file.
    pub cache_path: String,
    /// [`CONTRACT_VERSION`](crate::CONTRACT_VERSION).
    pub contract_version: &'static str,
    /// The store's `user_version`.
    pub store_schema_version: i64,
    /// The bd project.
    pub project: ProjectData,
    /// The recorded fingerprint.
    pub backend: BackendData,
    /// Journal counts.
    pub journal: JournalCountsData,
    /// Every scope, by id.
    pub scopes: Vec<ScopeData>,
    /// Every escalated entry until resolved, by seq.
    pub escalated: Vec<Entry>,
    /// The outage, while one holds.
    pub outage: Option<OutageData>,
    /// Stale rows, by id.
    pub stale: Vec<BeadId>,
}

/// `cache health` `open_work`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OpenWorkData {
    /// Pending entries.
    pub pending: u64,
    /// Escalations not yet announced.
    pub escalated_unannounced: u64,
}

/// `cache health --probe` `probe`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProbeData {
    /// bd's version, as now recorded.
    pub bd_version: Option<String>,
    /// The live schema equals the pinned one.
    pub model_current: bool,
    /// The check rebuilt the cache.
    pub rebuilt: bool,
}

/// `cache health` `state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    /// No outage, no stale row, no unannounced escalation.
    Ok,
    /// Stale rows or unannounced escalations, with bd reachable.
    Degraded,
    /// An outage holds.
    Down,
}

/// `cache health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheHealthData {
    /// The summary.
    pub state: HealthState,
    /// The outage, while one holds.
    pub outage: Option<OutageData>,
    /// Open work.
    pub open_work: OpenWorkData,
    /// How many rows are stale.
    pub stale_rows: u64,
    /// The backend check, with `probe`; `null` without.
    pub probe: Option<ProbeData>,
}

/// `cache scopes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheScopesData {
    /// Every scope, by id.
    pub scopes: Vec<ScopeData>,
}

/// One sync's report in `cache refresh scopes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncReportData {
    /// The backend check rebuilt the cache.
    pub rebuilt: bool,
    /// The live schema equals the pinned one.
    pub model_current: bool,
    /// Rows stored.
    pub stored: u32,
    /// Blocked rows in the refreshed base.
    pub blocked: u32,
    /// The sync did not run (deadline).
    pub deferred: bool,
    /// The outage, when bd was unreachable.
    pub outage: Option<OutageData>,
}

/// One scope's result in `cache refresh scopes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefreshScopeResult {
    /// The scope.
    pub scope: String,
    /// The report, when the sync ran.
    pub report: Option<SyncReportData>,
    /// The failure: the sync did not run, or it could not reach bd (lead
    /// ruling S75: a report with an outage is a failed result too).
    pub error: Option<RefreshError>,
}

/// A failed result's error in `cache refresh scopes`: the [`OpError`] with
/// its code's `kind`, `retryable` and `details` (lead ruling S75).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefreshError {
    /// The code and message.
    #[serde(flatten)]
    pub error: OpError,
    /// The code's kind (contract §4).
    pub kind: ErrorKind,
    /// Whether a retry may succeed (contract §4).
    pub retryable: bool,
    /// The code's details, as an envelope error has them (contract §4):
    /// `timed_out` for `BD_UNAVAILABLE`; `{}` when the code has none.
    pub details: Map<String, Value>,
}

impl RefreshError {
    /// `error` with the error table's `kind` and `retryable` for its code
    /// and no details; a code the table does not hold is `internal`, not
    /// retryable.
    #[must_use]
    pub fn of(error: OpError) -> Self {
        let spec = spec(&error.code);
        Self {
            kind: spec.map_or(ErrorKind::Internal, |s| s.kind),
            retryable: spec.is_some_and(|s| s.retryable),
            details: Map::new(),
            error,
        }
    }

    /// Adds one detail.
    #[must_use]
    pub fn with_detail(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.details.insert(key.to_owned(), value.into());
        self
    }
}

/// `cache refresh scopes`: `{failed, results}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheRefreshScopesData {
    /// How many results carry an error.
    pub failed: u64,
    /// One per scope, in request order (by id when none was named).
    pub results: Vec<RefreshScopeResult>,
}

/// `cache refresh issues`: `{issues, x_cache: {rows}}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CacheRefreshIssuesData {
    /// bd's rows with the overlay, in request order.
    pub issues: Vec<Bead>,
    /// The cache facts.
    pub x_cache: RowsXCache,
}

/// One escalation in `cache flush`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EscalatedData {
    /// The entry.
    pub seq: i64,
    /// The bead.
    pub bead: BeadId,
    /// The op.
    pub op: String,
    /// Who wrote it.
    pub actor: String,
    /// The entry's scope.
    pub scope: Option<String>,
    /// Why it escalated.
    pub reason: OpError,
}

/// `cache flush`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheFlushData {
    /// The drain lock was held elsewhere; nothing ran.
    pub busy: bool,
    /// Applied, by seq.
    pub applied: Vec<i64>,
    /// Failed and still pending, by seq.
    pub retrying: Vec<i64>,
    /// Escalated, by seq.
    pub escalated: Vec<EscalatedData>,
    /// Not started by the start gate, by seq.
    pub deferred: Vec<i64>,
    /// Cut by the deadline, by seq.
    pub deferred_in_flight: Vec<i64>,
    /// The outage, when bd was unreachable.
    pub outage: Option<OutageData>,
}

/// `cache evict`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheEvictData {
    /// The scope.
    pub scope: String,
    /// Beads deleted (held by no other scope).
    pub beads_deleted: u32,
    /// Out-of-scope rows swept.
    pub out_of_scope_swept: u32,
}

/// `cache journal get`: `{entry}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalGetData {
    /// The entry.
    pub entry: Entry,
}

/// `cache journal list`: `{entries}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalListData {
    /// The entries, by seq.
    pub entries: Vec<Entry>,
}

/// `cache journal resolve`: `{entry, already_resolved}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalResolveData {
    /// The entry after the call.
    pub entry: Entry,
    /// It was resolved before; nothing changed.
    pub already_resolved: bool,
}

/// `cache journal announce`: `{entry, already_announced}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JournalAnnounceData {
    /// The entry after the call.
    pub entry: Entry,
    /// It was announced before; nothing changed.
    pub already_announced: bool,
}
