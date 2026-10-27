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
//! The snapshot is the one owner of both limits, for every constructor: the
//! store trusts a built snapshot and checks neither again. Rows are counted
//! as they decode ([`IngestSnapshot::from_json`], [`IngestSnapshot::from_values`]
//! and [`IngestSnapshot::from_reader`] read the array one row at a time), so
//! an input with too many rows is refused at the first row over the limit,
//! and at most one undecoded row is held at a time. [`IngestSnapshot::from_reader`]
//! reads a stream (`bdc cache scopes ingest`'s stdin) as it decodes it,
//! counting its bytes, so an input over [`MAX_INGEST_BYTES`] is refused before
//! it is held whole and nothing is encoded again; the memory a call holds is
//! then the decoded rows. [`IngestSnapshot::from_values`] decodes values a
//! caller already parsed (a `PyO3` list, a `--input` request) without
//! re-encoding them.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::fmt;
use std::io;

use sc_beads::{Bead, BeadId, BlockedRow, parse_bd_row, parse_blocked_rows};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
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

    /// Decodes a snapshot from values a caller already parsed: `rows` as
    /// `bd list --json` rows and `blocked` as `bd blocked --json` rows, each a
    /// JSON array, `null` or absent (`None`) for none.
    ///
    /// Nothing is encoded again: the rows are counted and decoded one at a
    /// time from `rows`, as [`IngestSnapshot::from_json`] reads them, so a
    /// row past [`MAX_INGEST_ROWS`] stops the decode.
    ///
    /// # Errors
    ///
    /// As [`IngestSnapshot::from_json`], except that the input size is the
    /// caller's (`too_large` is then the rows' encoded size only).
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::{IngestSnapshot, ScopeId};
    /// let x = ScopeId::new("x").unwrap();
    /// let rows = serde_json::json!([{"id": "b1", "title": "t", "priority": 2, "status": "open",
    ///   "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"}]);
    /// let snap = IngestSnapshot::from_values(&x, Some(&rows), None).unwrap();
    /// assert_eq!(snap.rows().len(), 1);
    /// ```
    pub fn from_values(
        scope: &ScopeId,
        rows: Option<&Value>,
        blocked: Option<&Value>,
    ) -> Result<Self, CacheError> {
        let mut fault = None;
        let beads = match rows {
            None => Vec::new(),
            Some(value) => {
                let decoded = RowsSeed {
                    scope,
                    fault: &mut fault,
                }
                .deserialize(value);
                match (decoded, fault) {
                    (_, Some(e)) => return Err(e),
                    (Ok(rows), None) => rows,
                    (Err(e), None) => {
                        return Err(invalid(
                            scope,
                            IngestReason::RowInvalid,
                            None,
                            &format!("rows is not a JSON array of rows: {e}"),
                        ));
                    }
                }
            }
        };
        let blocked = match blocked {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(values)) => decode_blocked_values(scope, values.iter())?,
            Some(other) => {
                return Err(invalid(
                    scope,
                    IngestReason::RowInvalid,
                    None,
                    &format!("blocked is a JSON {}, not an array", kind(other)),
                ));
            }
        };
        Self::new(scope, beads, blocked)
    }

    /// Decodes a snapshot while `reader` is read (REQ-BDC-0726): a JSON
    /// array of rows, or an object with at most the members `rows` and
    /// `blocked` (each an array or `null`).
    ///
    /// The bytes are counted as they are read and the rows as they decode,
    /// so input over [`MAX_INGEST_BYTES`] or with more than
    /// [`MAX_INGEST_ROWS`] rows is refused before it is held whole; nothing
    /// is encoded again. The read stops at the first refusal.
    ///
    /// # Errors
    ///
    /// `INGEST_INVALID` with reason `too_large` (more than
    /// [`MAX_INGEST_BYTES`] read), `too_many_rows`, `row_invalid` (not JSON,
    /// not an array or object, or a row the strict decoder refuses, with its
    /// `index`) or `membership`; `BAD_ARGS` for an object member other than
    /// `rows` and `blocked` (the scope and the other arguments come from the
    /// command, never from the snapshot), or a read that fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::{IngestSnapshot, ScopeId};
    /// let x = ScopeId::new("x").unwrap();
    /// let input = br#"{"rows": [], "blocked": null}"#;
    /// assert!(IngestSnapshot::from_reader(&x, &input[..]).unwrap().is_empty());
    /// let other = br#"{"rows": [], "scope": "y"}"#;
    /// assert_eq!(
    ///     IngestSnapshot::from_reader(&x, &other[..]).unwrap_err().code(),
    ///     "BD_CACHE.BAD_ARGS"
    /// );
    /// ```
    pub fn from_reader(scope: &ScopeId, reader: impl io::Read) -> Result<Self, CacheError> {
        // `Counted` borrows the flag: reader and flag live in this one frame.
        let over = Cell::new(false);
        let counted = Counted {
            inner: reader,
            read: 0,
            over: &over,
        };
        let mut fault = None;
        let mut de = serde_json::Deserializer::from_reader(counted);
        let decoded = SnapshotSeed {
            scope,
            fault: &mut fault,
        }
        .deserialize(&mut de)
        .and_then(|parts| de.end().map(|()| parts));
        if over.get() {
            return Err(invalid(
                scope,
                IngestReason::TooLarge,
                None,
                &format!(
                    "the input is over {MAX_INGEST_BYTES} bytes; one ingest takes at most that"
                ),
            ));
        }
        match (decoded, fault) {
            (_, Some(e)) => Err(e),
            (Ok((rows, blocked)), None) => Self::new(scope, rows, blocked),
            (Err(e), None) if e.is_io() => Err(CacheError::BadArgs {
                detail: format!("ingest of scope {scope}: the input cannot be read: {e}"),
            }),
            (Err(e), None) => Err(invalid(
                scope,
                IngestReason::RowInvalid,
                None,
                &format!("the input is not a JSON rows array or snapshot object: {e}"),
            )),
        }
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

/// A reader that counts its bytes and fails once past [`MAX_INGEST_BYTES`],
/// setting `over`, so a stream is never read past the limit.
struct Counted<'a, R> {
    inner: R,
    read: usize,
    over: &'a Cell<bool>,
}

impl<R: io::Read> io::Read for Counted<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read = self.read.saturating_add(n);
        if self.read > MAX_INGEST_BYTES {
            self.over.set(true);
            return Err(io::Error::other("over the ingest byte limit"));
        }
        Ok(n)
    }
}

/// The streaming reader of a whole snapshot: a rows array, or an object
/// with `rows` and `blocked` and no other member.
struct SnapshotSeed<'a> {
    scope: &'a ScopeId,
    fault: &'a mut Option<CacheError>,
}

impl<'de> DeserializeSeed<'de> for SnapshotSeed<'_> {
    type Value = (Vec<Bead>, Vec<BlockedRow>);

    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for SnapshotSeed<'_> {
    type Value = (Vec<Bead>, Vec<BlockedRow>);

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON array of rows, or an object with rows and blocked")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
        let rows = RowsSeed {
            scope: self.scope,
            fault: self.fault,
        }
        .visit_seq(seq)?;
        Ok((rows, Vec::new()))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let (mut rows, mut blocked) = (None, None);
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "rows" if rows.is_none() => {
                    rows = Some(map.next_value_seed(RowsSeed {
                        scope: self.scope,
                        fault: &mut *self.fault,
                    })?);
                }
                "blocked" if blocked.is_none() => {
                    blocked = Some(map.next_value_seed(BlockedSeed {
                        scope: self.scope,
                        fault: &mut *self.fault,
                    })?);
                }
                "rows" | "blocked" => {
                    *self.fault = Some(CacheError::BadArgs {
                        detail: format!(
                            "ingest of scope {}: the snapshot names {key} twice",
                            self.scope
                        ),
                    });
                    return Err(de::Error::custom("a member twice"));
                }
                _ => {
                    *self.fault = Some(CacheError::BadArgs {
                        detail: format!(
                            "ingest of scope {}: the snapshot object has the member {key:?}; \
                             it may hold only rows and blocked (the scope and the other \
                             arguments are the command's)",
                            self.scope
                        ),
                    });
                    return Err(de::Error::custom("an unknown member"));
                }
            }
        }
        Ok((rows.unwrap_or_default(), blocked.unwrap_or_default()))
    }
}

/// The streaming reader of the blocked rows: counted as they decode, each
/// decoded alone with the strict `bd blocked` decoder; `null` is none.
struct BlockedSeed<'a> {
    scope: &'a ScopeId,
    fault: &'a mut Option<CacheError>,
}

impl<'de> DeserializeSeed<'de> for BlockedSeed<'_> {
    type Value = Vec<BlockedRow>;

    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Vec<BlockedRow>, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for BlockedSeed<'_> {
    type Value = Vec<BlockedRow>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON array of blocked rows, or null")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Vec<BlockedRow>, E> {
        Ok(Vec::new())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<BlockedRow>, A::Error> {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(1024));
        while let Some(value) = seq.next_element::<Value>()? {
            let index = out.len();
            if index == MAX_INGEST_ROWS {
                *self.fault = Some(too_many_rows(self.scope, index + 1));
                return Err(de::Error::custom("too many blocked rows"));
            }
            match blocked_row(self.scope, index, &value) {
                Ok(row) => out.push(row),
                Err(e) => {
                    *self.fault = Some(e);
                    return Err(de::Error::custom("invalid blocked row"));
                }
            }
        }
        Ok(out)
    }
}

/// One blocked row, decoded alone by the strict `bd blocked` decoder.
fn blocked_row(scope: &ScopeId, index: usize, value: &Value) -> Result<BlockedRow, CacheError> {
    // The decoder takes `bd blocked --json` text; one row at a time keeps
    // at most one row's text alive, never the list's.
    let text = format!("[{value}]");
    let mut rows = parse_blocked_rows(&text).map_err(|e| {
        invalid(
            scope,
            IngestReason::RowInvalid,
            Some(index),
            &format!("blocked row {index}: {e}"),
        )
    })?;
    rows.pop().ok_or_else(|| {
        invalid(
            scope,
            IngestReason::RowInvalid,
            Some(index),
            &format!("blocked row {index} decoded to nothing"),
        )
    })
}

/// Blocked rows a caller already parsed, counted and decoded one at a time.
fn decode_blocked_values<'a>(
    scope: &ScopeId,
    values: impl Iterator<Item = &'a Value>,
) -> Result<Vec<BlockedRow>, CacheError> {
    let mut out = Vec::new();
    for (index, value) in values.enumerate() {
        if index == MAX_INGEST_ROWS {
            return Err(too_many_rows(scope, index + 1));
        }
        out.push(blocked_row(scope, index, value)?);
    }
    Ok(out)
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

    /// An endless-looking stream of minimal valid rows, `limit` bytes long,
    /// produced as it is read; `read` counts what the decoder took.
    struct RowStream {
        row: Vec<u8>,
        limit: usize,
        sent: usize,
        read: std::rc::Rc<std::cell::Cell<usize>>,
    }

    impl io::Read for RowStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut n = 0;
            while n < buf.len() && self.sent < self.limit {
                let byte = if self.sent == 0 {
                    b'['
                } else {
                    // Each row followed by a comma, from offset 1.
                    let i = (self.sent - 1) % (self.row.len() + 1);
                    self.row.get(i).copied().unwrap_or(b',')
                };
                if let Some(slot) = buf.get_mut(n) {
                    *slot = byte;
                }
                n += 1;
                self.sent += 1;
            }
            self.read.set(self.read.get() + n);
            Ok(n)
        }
    }

    fn stream(limit: usize) -> (RowStream, std::rc::Rc<std::cell::Cell<usize>>) {
        let read = std::rc::Rc::new(std::cell::Cell::new(0));
        let row = serde_json::json!({"id": "b1", "title": "t", "priority": 2, "status": "open",
            "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"})
        .to_string()
        .into_bytes();
        (
            RowStream {
                row,
                limit,
                sent: 0,
                read: std::rc::Rc::clone(&read),
            },
            read,
        )
    }

    /// 64 MiB of minimal rows on a stream is refused as `too_many_rows` at
    /// the row past the limit, having read only that far: the input is never
    /// held whole (REQ-BDC-0726, B8Q1-013). Planted negative: a stream that
    /// keeps going past 64 MiB of anything is `too_large`, not read on.
    #[test]
    fn a_stream_of_too_many_rows_is_refused_before_it_is_read_whole() {
        let (input, read) = stream(MAX_INGEST_BYTES);
        let e = IngestSnapshot::from_reader(&x(), input).unwrap_err();
        assert_eq!(reason(&e), (IngestReason::TooManyRows, None));
        assert!(
            read.get() < MAX_INGEST_BYTES / 4,
            "read {} bytes of {MAX_INGEST_BYTES}",
            read.get()
        );
        let spaces = io::Read::take(io::repeat(b' '), (MAX_INGEST_BYTES as u64) * 2);
        let e = IngestSnapshot::from_reader(&x(), spaces).unwrap_err();
        assert_eq!(reason(&e), (IngestReason::TooLarge, None));
    }

    /// A stream holds a rows array or `{rows, blocked}`; any other member is
    /// `BAD_ARGS`, so a snapshot can never name another scope (B8Q1-031).
    /// `from_values` decodes the same values without encoding them again.
    #[test]
    fn a_stream_takes_only_rows_and_blocked_and_from_values_agrees() {
        let rows = serde_json::Value::Array(vec![row_json(0), row_json(1)]);
        let blocked = serde_json::json!([{"id": "b1", "title": "t", "priority": 2,
            "status": "open", "created_at": "2026-10-23T09:00:00Z",
            "updated_at": "2026-10-23T09:00:00Z", "blocked_by": ["b0"]}]);
        let object = serde_json::json!({"rows": rows, "blocked": blocked}).to_string();
        let streamed = IngestSnapshot::from_reader(&x(), object.as_bytes()).unwrap();
        let parsed = IngestSnapshot::from_values(&x(), Some(&rows), Some(&blocked)).unwrap();
        let text =
            IngestSnapshot::from_json(&x(), &rows.to_string(), &blocked.to_string()).unwrap();
        assert_eq!(streamed, parsed);
        assert_eq!(streamed, text);
        assert_eq!(streamed.blocked().len(), 1);
        let array = IngestSnapshot::from_reader(&x(), rows.to_string().as_bytes()).unwrap();
        assert_eq!(array.rows(), streamed.rows());
        for bad in [
            r#"{"rows": [], "scope": "other"}"#,
            r#"{"rows": [], "rows": []}"#,
            r#"{"allow_empty": true}"#,
        ] {
            let e = IngestSnapshot::from_reader(&x(), bad.as_bytes()).unwrap_err();
            assert_eq!(e.code(), "BD_CACHE.BAD_ARGS", "{bad}");
        }
        for bad in ["3", "\"x\"", "[1]", "{} trailing"] {
            let e = IngestSnapshot::from_reader(&x(), bad.as_bytes()).unwrap_err();
            assert_eq!(reason(&e).0, IngestReason::RowInvalid, "{bad}");
        }
        let bad_blocked = serde_json::json!([{"id": "b0", "surprise": 1}]);
        let e = IngestSnapshot::from_values(&x(), Some(&rows), Some(&bad_blocked)).unwrap_err();
        assert_eq!(reason(&e), (IngestReason::RowInvalid, Some(0)));
    }

    /// The unknown-member detail is one clean sentence, with no run of
    /// padding left by a broken string continuation (PE1-RBP-F001).
    #[test]
    fn the_unknown_member_detail_is_one_clean_sentence() {
        let e = IngestSnapshot::from_reader(&x(), br#"{"rows": [], "scope": "other"}"#.as_slice())
            .unwrap_err();
        let CacheError::BadArgs { detail } = e else {
            panic!("expected BAD_ARGS, got {e:?}");
        };
        assert_eq!(
            detail,
            "ingest of scope x: the snapshot object has the member \"scope\"; \
             it may hold only rows and blocked (the scope and the other \
             arguments are the command's)"
        );
    }
}
