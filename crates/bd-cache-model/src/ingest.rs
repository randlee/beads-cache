//! [`IngestSnapshot`]: a snapshot of one scope a caller fetched through bd,
//! checked by the built-in sync's rules before the store sees it
//! (REQ-BDC-0726, REQ-BDC-0007, REQ-BDC-0008).
//!
//! A snapshot is every row of the scope (`bd list --json` rows) and bd's
//! blocked rows for it (`bd blocked --json` rows). It is decoded with the
//! strict `sc-beads` row decoder; at most [`MAX_INGEST_ROWS`] rows and
//! [`MAX_INGEST_BYTES`] of input per call; no bead may be listed twice. Any
//! violation is `INGEST_INVALID` naming the reason, and the row's index when
//! one row is at fault: one invalid row among valid ones refuses the whole
//! snapshot, so no row of it is stored (REQ-BDC-0008).

use std::collections::BTreeSet;

use sc_beads::{Bead, BeadId, BlockedRow, parse_bd_row, parse_blocked_rows};
use serde_json::Value;

use crate::error::{CacheError, IngestReason};
use crate::scope::ScopeId;

/// The most rows one ingest call takes (REQ-BDC-0726).
pub const MAX_INGEST_ROWS: usize = 50_000;
/// The most bytes of input one ingest call takes: 64 MiB (REQ-BDC-0726).
pub const MAX_INGEST_BYTES: usize = 64 * 1024 * 1024;

/// A checked snapshot of one scope: rows and blocked rows, ready to ingest.
///
/// # Examples
///
/// ```
/// use bd_cache_model::{IngestSnapshot, ScopeId};
/// let x = ScopeId::new("x").unwrap();
/// let rows = r#"[{"id": "b1", "title": "t", "priority": 2, "status": "open",
///   "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"}]"#;
/// let snap = IngestSnapshot::from_json(&x, rows, "[]").unwrap();
/// assert_eq!(snap.rows().len(), 1);
/// let bad = r#"[{"id": "b2", "surprise": 1}]"#;
/// let err = IngestSnapshot::from_json(&x, bad, "[]").unwrap_err();
/// assert_eq!(err.code(), "BD_CACHE.INGEST_INVALID");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct IngestSnapshot {
    rows: Vec<Bead>,
    blocked: Vec<BlockedRow>,
}

impl IngestSnapshot {
    /// A snapshot of rows already decoded strictly (a Rust caller, or the
    /// regression test that runs a built-in snapshot through ingest).
    ///
    /// # Errors
    ///
    /// `INGEST_INVALID` for more than [`MAX_INGEST_ROWS`] rows
    /// (`too_many_rows`) or a bead listed twice among the rows or among the
    /// blocked rows (`membership`, with the index of the second listing).
    pub fn new(
        scope: &ScopeId,
        rows: Vec<Bead>,
        blocked: Vec<BlockedRow>,
    ) -> Result<Self, CacheError> {
        if rows.len() > MAX_INGEST_ROWS {
            return Err(invalid(
                scope,
                IngestReason::TooManyRows,
                None,
                &format!(
                    "{} rows; one ingest takes at most {MAX_INGEST_ROWS}",
                    rows.len()
                ),
            ));
        }
        check_unique(scope, "rows", rows.iter().map(|r| &r.id))?;
        check_unique(scope, "blocked", blocked.iter().map(|b| &b.id))?;
        Ok(Self { rows, blocked })
    }

    /// Decodes a snapshot: `rows_json` as `bd list --json` prints rows and
    /// `blocked_json` as `bd blocked --json` prints them, each a JSON array
    /// (or `null` for none).
    ///
    /// The input size is checked first, then the row count, then each row
    /// with the strict decoder, so a bad row is named by its index.
    ///
    /// # Errors
    ///
    /// `INGEST_INVALID` with reason `too_large` (input over
    /// [`MAX_INGEST_BYTES`]), `too_many_rows`, `row_invalid` (not JSON, not
    /// an array, or a row the strict decoder refuses, with its `index`) or
    /// `membership` (a bead listed twice).
    pub fn from_json(
        scope: &ScopeId,
        rows_json: &str,
        blocked_json: &str,
    ) -> Result<Self, CacheError> {
        let size = rows_json.len().saturating_add(blocked_json.len());
        if size > MAX_INGEST_BYTES {
            return Err(invalid(
                scope,
                IngestReason::TooLarge,
                None,
                &format!("{size} bytes; one ingest takes at most {MAX_INGEST_BYTES}"),
            ));
        }
        let rows = array(scope, "rows", rows_json)?;
        if rows.len() > MAX_INGEST_ROWS {
            return Err(invalid(
                scope,
                IngestReason::TooManyRows,
                None,
                &format!(
                    "{} rows; one ingest takes at most {MAX_INGEST_ROWS}",
                    rows.len()
                ),
            ));
        }
        let mut beads = Vec::with_capacity(rows.len());
        for (index, value) in rows.iter().enumerate() {
            let bead = parse_bd_row(value).map_err(|e| {
                invalid(
                    scope,
                    IngestReason::RowInvalid,
                    Some(index),
                    &format!("row {index}: {e}"),
                )
            })?;
            beads.push(bead);
        }
        let mut blocked = Vec::new();
        for (index, value) in array(scope, "blocked", blocked_json)?
            .into_iter()
            .enumerate()
        {
            // The strict decoder of `bd blocked --json`, one row at a time so
            // the error names its index.
            let one = Value::Array(vec![value]).to_string();
            let mut parsed = parse_blocked_rows(&one).map_err(|e| {
                invalid(
                    scope,
                    IngestReason::RowInvalid,
                    Some(index),
                    &format!("blocked row {index}: {e}"),
                )
            })?;
            blocked.append(&mut parsed);
        }
        Self::new(scope, beads, blocked)
    }

    /// The rows, as given.
    #[must_use]
    pub fn rows(&self) -> &[Bead] {
        &self.rows
    }

    /// The blocked rows, as given.
    #[must_use]
    pub fn blocked(&self) -> &[BlockedRow] {
        &self.blocked
    }

    /// True when the snapshot lists no row.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// `INGEST_INVALID` for `scope`.
fn invalid(
    scope: &ScopeId,
    reason: IngestReason,
    index: Option<usize>,
    detail: &str,
) -> CacheError {
    CacheError::IngestInvalid {
        detail: format!("ingest of scope {scope}: {detail}"),
        scope: scope.clone(),
        reason,
        index,
    }
}

/// The JSON array of one list of the snapshot; `null` is an empty list.
fn array(scope: &ScopeId, what: &str, json: &str) -> Result<Vec<Value>, CacheError> {
    match serde_json::from_str::<Value>(json) {
        Ok(Value::Array(values)) => Ok(values),
        Ok(Value::Null) => Ok(Vec::new()),
        Ok(other) => Err(invalid(
            scope,
            IngestReason::RowInvalid,
            None,
            &format!("{what} is a JSON {}, not an array", kind(&other)),
        )),
        Err(e) => Err(invalid(
            scope,
            IngestReason::RowInvalid,
            None,
            &format!("{what} is not JSON: {e}"),
        )),
    }
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// `membership` for the second listing of a bead.
fn check_unique<'a>(
    scope: &ScopeId,
    what: &str,
    ids: impl Iterator<Item = &'a BeadId>,
) -> Result<(), CacheError> {
    let mut seen = BTreeSet::new();
    for (index, id) in ids.enumerate() {
        if !seen.insert(id) {
            return Err(invalid(
                scope,
                IngestReason::Membership,
                Some(index),
                &format!("{what} {index} lists {id} a second time"),
            ));
        }
    }
    Ok(())
}
