//! [`IngestSnapshot`]: a snapshot of one scope a caller fetched through bd,
//! checked by the built-in sync's rules before the store sees it
//! (REQ-BDC-0726, REQ-BDC-0007, REQ-BDC-0008).
//!
//! A snapshot is every row of the scope (`bd list --json` rows) and bd's
//! blocked rows for it (`bd blocked --json` rows). It is decoded with the
//! strict `sc-beads` row decoder; at most [`MAX_INGEST_ROWS`] rows and
//! [`MAX_INGEST_BYTES`] of rows per call; no bead may be listed twice. Any
//! violation is `INGEST_INVALID` naming the reason, and the row's index when
//! one row is at fault: one invalid row among valid ones refuses the whole
//! snapshot, so no row of it is stored (REQ-BDC-0008).
//!
//! The snapshot is the one owner of both limits, for both constructors: the
//! store trusts a built snapshot and checks neither again. Rows are counted
//! as they decode ([`IngestSnapshot::from_json`] reads the array one row at a
//! time), so an input with too many rows is refused at the first row over the
//! limit, and at most one undecoded row is held at a time. The memory a
//! successful call holds is the decoded rows (about the size of their JSON)
//! plus the caller's input text, which is at most [`MAX_INGEST_BYTES`].

use std::collections::BTreeSet;
use std::fmt;
use std::io;

use sc_beads::{Bead, BeadId, BlockedRow, parse_bd_row, parse_blocked_rows};
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use serde_json::Value;

use crate::error::{CacheError, IngestReason};
use crate::scope::ScopeId;

/// The most rows one ingest call takes (REQ-BDC-0726).
pub const MAX_INGEST_ROWS: usize = 50_000;
/// The most bytes one ingest call takes: 64 MiB of input text, and 64 MiB of
/// rows as JSON (REQ-BDC-0726).
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
    /// (`too_many_rows`), rows over [`MAX_INGEST_BYTES`] as JSON
    /// (`too_large`, measured by [`encoded_rows_within_limit`], which stops at
    /// the limit), or a bead listed twice among the rows or among the blocked
    /// rows (`membership`, with the index of the second listing).
    pub fn new(
        scope: &ScopeId,
        rows: Vec<Bead>,
        blocked: Vec<BlockedRow>,
    ) -> Result<Self, CacheError> {
        if rows.len() > MAX_INGEST_ROWS {
            return Err(too_many_rows(scope, rows.len()));
        }
        if !encoded_rows_within_limit(&rows) {
            return Err(invalid(
                scope,
                IngestReason::TooLarge,
                None,
                &format!("the rows are over {MAX_INGEST_BYTES} bytes as JSON"),
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
    /// The input size is checked first; the rows are then read one at a
    /// time, each counted and decoded with the strict decoder as it is read,
    /// so a bad row is named by its index and a row over the limit stops the
    /// read; the blocked rows are decoded once.
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
        let beads = decode_rows(scope, rows_json)?;
        let blocked = decode_blocked(scope, blocked_json)?;
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

/// True when `rows`, encoded as JSON, take at most [`MAX_INGEST_BYTES`].
///
/// The one measure of the byte limit: each row is encoded into a counter, not
/// a buffer, and the count stops as soon as the running total passes the
/// limit, so an oversized snapshot is not encoded whole.
#[must_use]
pub fn encoded_rows_within_limit(rows: &[Bead]) -> bool {
    let mut counter = ByteCounter(0);
    for row in rows {
        if serde_json::to_writer(&mut counter, row).is_err() || counter.0 > MAX_INGEST_BYTES {
            return false;
        }
    }
    true
}

/// Counts the bytes written to it, failing once past the ingest limit.
struct ByteCounter(usize);

impl io::Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 = self.0.saturating_add(buf.len());
        if self.0 > MAX_INGEST_BYTES {
            return Err(io::Error::other("over the ingest byte limit"));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `too_many_rows` for a count over the limit.
fn too_many_rows(scope: &ScopeId, count: usize) -> CacheError {
    invalid(
        scope,
        IngestReason::TooManyRows,
        None,
        &format!("{count} rows; one ingest takes at most {MAX_INGEST_ROWS}"),
    )
}

/// The rows of `json`, read one at a time: each counted and decoded strictly
/// as it is read (REQ-BDC-0726).
fn decode_rows(scope: &ScopeId, json: &str) -> Result<Vec<Bead>, CacheError> {
    let mut fault = None;
    let mut de = serde_json::Deserializer::from_str(json);
    let decoded = RowsSeed {
        scope,
        fault: &mut fault,
    }
    .deserialize(&mut de)
    .and_then(|rows| de.end().map(|()| rows));
    match (decoded, fault) {
        (_, Some(e)) => Err(e),
        (Ok(rows), None) => Ok(rows),
        (Err(e), None) => Err(invalid(
            scope,
            IngestReason::RowInvalid,
            None,
            &format!("rows is not a JSON array of rows: {e}"),
        )),
    }
}

/// The streaming reader of the rows array: `null` is no row.
struct RowsSeed<'a> {
    scope: &'a ScopeId,
    /// The typed refusal, when a row stopped the read.
    fault: &'a mut Option<CacheError>,
}

impl<'de> DeserializeSeed<'de> for RowsSeed<'_> {
    type Value = Vec<Bead>;

    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Vec<Bead>, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for RowsSeed<'_> {
    type Value = Vec<Bead>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON array of rows, or null")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Vec<Bead>, E> {
        Ok(Vec::new())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<Bead>, A::Error> {
        let mut beads = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(1024));
        while let Some(value) = seq.next_element::<Value>()? {
            let index = beads.len();
            if index == MAX_INGEST_ROWS {
                *self.fault = Some(too_many_rows(self.scope, index + 1));
                return Err(de::Error::custom("too many rows"));
            }
            match parse_bd_row(&value) {
                Ok(bead) => beads.push(bead),
                Err(e) => {
                    *self.fault = Some(invalid(
                        self.scope,
                        IngestReason::RowInvalid,
                        Some(index),
                        &format!("row {index}: {e}"),
                    ));
                    return Err(de::Error::custom("invalid row"));
                }
            }
        }
        Ok(beads)
    }
}

/// The blocked rows of `json`, decoded once with the strict `bd blocked`
/// decoder; on a refusal the faulty row is found and named by its index.
fn decode_blocked(scope: &ScopeId, json: &str) -> Result<Vec<BlockedRow>, CacheError> {
    match parse_blocked_rows(json) {
        Ok(rows) => Ok(rows),
        Err(e) => {
            // The error path only: find which row the decoder refused.
            let values = array(scope, "blocked", json)?;
            let index = values.iter().position(|v| {
                parse_blocked_rows(&Value::Array(vec![v.clone()]).to_string()).is_err()
            });
            Err(invalid(
                scope,
                IngestReason::RowInvalid,
                index,
                &match index {
                    Some(i) => format!("blocked row {i}: {e}"),
                    None => format!("blocked: {e}"),
                },
            ))
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn x() -> ScopeId {
        ScopeId::new("x").unwrap()
    }

    fn reason(e: &CacheError) -> (IngestReason, Option<usize>) {
        match e {
            CacheError::IngestInvalid { reason, index, .. } => (*reason, *index),
            other => panic!("not INGEST_INVALID: {other:?}"),
        }
    }

    fn row_json(i: usize) -> serde_json::Value {
        let mut b = sc_beads::testing::bead(&format!("b{i}"));
        b.labels = vec!["l".into()];
        serde_json::to_value(b).unwrap()
    }

    /// Rows are counted as they decode: the row past the limit stops the
    /// read with `too_many_rows`, before any later row is decoded (B7Q1-017).
    #[test]
    fn rows_are_counted_as_they_decode() {
        let mut rows: Vec<serde_json::Value> = (0..=MAX_INGEST_ROWS).map(row_json).collect();
        // A row after the limit that would fail to decode is never reached.
        rows.push(serde_json::json!({"id": "bad", "surprise": 1}));
        let json = serde_json::Value::Array(rows).to_string();
        let e = IngestSnapshot::from_json(&x(), &json, "[]").unwrap_err();
        assert_eq!(reason(&e), (IngestReason::TooManyRows, None));
    }

    /// A refused row and a refused blocked row are named by their index;
    /// `null` is no row; anything else is `row_invalid`.
    #[test]
    fn a_refused_row_is_named_by_its_index() {
        let rows = serde_json::Value::Array(vec![
            row_json(0),
            row_json(1),
            serde_json::json!({"id": "b2", "surprise": 1}),
        ])
        .to_string();
        let e = IngestSnapshot::from_json(&x(), &rows, "[]").unwrap_err();
        assert_eq!(reason(&e), (IngestReason::RowInvalid, Some(2)));
        let one = serde_json::Value::Array(vec![row_json(0)]).to_string();
        let blocked = r#"[{"id": "b0", "surprise": 1}]"#;
        let e = IngestSnapshot::from_json(&x(), &one, blocked).unwrap_err();
        assert_eq!(reason(&e), (IngestReason::RowInvalid, Some(0)));
        assert!(
            IngestSnapshot::from_json(&x(), "null", "null")
                .unwrap()
                .is_empty()
        );
        for bad in ["{}", "3", "[1] x", ""] {
            let e = IngestSnapshot::from_json(&x(), bad, "[]").unwrap_err();
            assert_eq!(reason(&e).0, IngestReason::RowInvalid, "{bad:?}");
        }
    }

    /// `new` owns the byte limit too: rows over it as JSON are `too_large`,
    /// measured without encoding the whole snapshot (B7Q1-018).
    #[test]
    fn new_refuses_rows_over_the_byte_limit() {
        let mut big = sc_beads::testing::bead("b1");
        big.description = Some("d".repeat(MAX_INGEST_BYTES));
        assert!(!encoded_rows_within_limit(std::slice::from_ref(&big)));
        let e = IngestSnapshot::new(&x(), vec![big], vec![]).unwrap_err();
        assert_eq!(reason(&e), (IngestReason::TooLarge, None));
        let small = sc_beads::testing::bead("b2");
        assert!(encoded_rows_within_limit(&[small]));
    }
}
