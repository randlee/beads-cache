//! The `data` of the store-only phase-B verbs (ADR-BDC-0015). The verbs a
//! front end executes (`cache schedule *`, `cron run`, `tick`, `sync *`)
//! reply with `bd-cache-sync`'s data, which this crate does not model.

use bd_cache_model::FilterParts;
use serde::Serialize;

/// `cache scopes add` and `cache scopes set`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopesChangeData {
    /// The scope's name.
    pub scope: String,
    /// Its filter, as stored.
    pub filter: Option<FilterParts>,
}

/// `cache scopes remove`: `cache evict`'s reply plus the registry removal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopesRemoveData {
    /// The scope's name.
    pub scope: String,
    /// Beads deleted (held by no other scope).
    pub beads_deleted: u32,
    /// Out-of-scope rows swept.
    pub out_of_scope_swept: u32,
    /// Removed from the registry.
    pub removed: bool,
}

/// `cache scopes ingest` (REQ-BDC-0726).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopesIngestData {
    /// The scope's name.
    pub scope: String,
    /// Rows in the snapshot.
    pub rows: usize,
    /// Rows stored.
    pub stored: usize,
    /// Rows kept because the stored row is newer (REQ-BDC-0502).
    pub skipped_newer: usize,
    /// The revision stored with the snapshot.
    pub revision: Option<String>,
}

/// `cache scopes announce` (REQ-BDC-0728).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopesAnnounceData {
    /// The scope's name.
    pub scope: String,
    /// The `failing_since` token that was marked.
    pub since: String,
    /// The failure is now notified.
    pub notified: bool,
}
