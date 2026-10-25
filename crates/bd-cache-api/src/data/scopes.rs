//! The `data` of `cache scopes ingest` and `cache scopes announce` (ANC-003,
//! REQ-BDC-0705, 0706).

use serde::Serialize;

/// `cache scopes ingest`: what the ingest stored, by bead id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheScopesIngestData {
    /// The scope fed.
    pub scope: String,
    /// Rows stored.
    pub stored: Vec<String>,
    /// Rows not stored because the cache holds a newer one (REQ-BDC-0502).
    pub skipped_newer: Vec<String>,
    /// Stored rows whose status changed.
    pub status_changed: Vec<String>,
    /// The revision stored with the rows, when given.
    pub revision: Option<String>,
}

/// `cache scopes announce`: whether the mark was set, and the scope's
/// failure state after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheScopesAnnounceData {
    /// The scope.
    pub scope: String,
    /// `true` when `since` was the scope's current `failing_since`; `false`
    /// when it is not failing, or began failing at another time (nothing changed).
    pub announced: bool,
    /// When the scope's current run of failures began, `null` while it syncs.
    pub failing_since: Option<String>,
    /// The `failing_since` last marked handled.
    pub notified_since: Option<String>,
}
