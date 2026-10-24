//! Store-side state: freshness, sync state, outage, fingerprint outcome, reports and status.

use std::collections::BTreeMap;

use sc_beads::{BeadId, BlockedRow};
use serde::Serialize;

use crate::journal::JournalEntry;
use crate::scope::ScopeId;

/// Whether a stored row is current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Stored from a successful read.
    Fresh,
    /// A read-back failed (`stale = 1`); the next sync re-reads it.
    Stale,
}

/// One scope's sync cursor and last outcome (`sync_state`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncState {
    /// The scope.
    #[serde(rename = "phase")]
    pub scope: ScopeId,
    /// Start of the latest sync.
    pub last_started_at: Option<String>,
    /// Start of the latest successful sync (the cursor basis).
    pub last_ok_at: Option<String>,
    /// The latest failure, `CODE: detail`.
    pub last_error: Option<String>,
}

impl SyncState {
    /// A scope that never synced.
    #[must_use]
    pub fn new(scope: ScopeId) -> Self {
        Self {
            scope,
            last_started_at: None,
            last_ok_at: None,
            last_error: None,
        }
    }
}

/// The backend outage record (ADR-BDC-0005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outage {
    /// When the outage started; kept across later failures.
    pub since: String,
    /// The latest error, `CODE: detail`.
    pub last_error: String,
}

/// What recording a fingerprint did (REQ-BDC-0105).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintOutcome {
    /// First check: recorded, nothing dropped.
    Recorded,
    /// Same schema sha as recorded.
    Unchanged,
    /// A different sha: cached rows dropped, journal kept, new sha recorded.
    Rebuilt,
}

/// What one `upsert_rows` call stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UpsertReport {
    /// Rows stored.
    pub stored: Vec<BeadId>,
    /// Rows not stored because the stored row is newer (unforced upsert).
    pub skipped_newer: Vec<BeadId>,
    /// Existing rows whose status text changed (forced or not); new rows excluded.
    pub status_changed: Vec<BeadId>,
}

/// A scope's stored ready base (ADR-BDC-0002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadyBase {
    /// `bd blocked -l <label>` rows.
    pub blocked: Vec<BlockedRow>,
    /// `bd ready -l <label> -n 0` ids.
    pub ready: Vec<BeadId>,
    /// The instant taken before the sync's first blocked/ready call.
    pub fetched_at: String,
    /// True when a later refresh failed or was cut.
    pub stale: bool,
}

/// Open work for the stop rule (REQ-BDC-0707).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct OpenWork {
    /// Pending entries.
    pub pending: u64,
    /// Escalated entries not yet announced.
    pub escalated_unannounced: u64,
}

impl OpenWork {
    /// True when any work is open.
    #[must_use]
    pub fn any(&self) -> bool {
        self.pending > 0 || self.escalated_unannounced > 0
    }
}

/// One scope in [`StoreStatus`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeStatus {
    /// Sync state.
    #[serde(flatten)]
    pub sync: SyncState,
    /// Beads held by the scope.
    pub beads: u64,
}

/// What `status` reports from the store (REQ-BDC-0603).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct StoreStatus {
    /// The `meta` table.
    pub meta: BTreeMap<String, String>,
    /// Journal counts keyed `<phase>:<state>`; entries with no phase use `None`,
    /// as the Python implementation prints it.
    pub journal: BTreeMap<String, u64>,
    /// Per-scope sync state and bead counts, by scope.
    #[serde(rename = "phases")]
    pub scopes: Vec<ScopeStatus>,
    /// Every escalated entry until it is resolved, by seq.
    pub escalated: Vec<JournalEntry>,
    /// The outage, while one holds.
    pub outage: Option<Outage>,
}
