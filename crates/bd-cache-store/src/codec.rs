//! Encoding and decoding stored values, validated on every read.
//!
//! Every value read back is validated: `row_json` through the row model's
//! serde shape, ids, actors, keys and timestamps through their newtypes,
//! journal text through `Op::parse` and `parse_precondition`. A value that
//! does not decode is an error naming the entry, never skipped. A file
//! another implementation wrote is read the same way.

use bd_cache_model::{
    CacheError, IdemKey, JournalEntry, JournalState, Op, ScopeId, Seq, error_code_prefix,
    normalize, parse_precondition, python_json, split_resolve_note,
};
use sc_beads::{Actor, Bead, BeadId, BlockedRow, DepType, Edge, Timestamp};
use serde_json::Value;

use crate::error::{Step, corrupt, failed};

/// The `row_json` text of a stored row and its v3 `phase` column.
///
/// `row_json` is the normalized row (REQ-BDC-0108) with its outgoing edges as
/// `{issue_id, depends_on_id, type}`, rendered as Python's
/// `json.dumps(row, sort_keys=True)`, so `bd_cache.py` reads it unchanged.
pub(crate) fn encode_row(step: Step<'_>, row: &Bead) -> Result<(Bead, String), CacheError> {
    let stored = normalize(row);
    let value = serde_json::to_value(&stored).map_err(|e| failed(step, e))?;
    Ok((stored, python_json(&value)))
}

/// A stored `row_json` decoded into a [`Bead`] without its edges.
///
/// The decoding is the exact inverse of [`encode_row`]: the row model's serde
/// shape (unknown keys refused, types and ids checked), so every row an
/// upsert stored reads back. `dependencies` is ignored: the `edges` table is
/// the authority, and `bd_cache.py` keeps bd's full dependency objects there.
pub(crate) fn decode_row(step: Step<'_>, id: &str, row_json: &str) -> Result<Bead, CacheError> {
    let mut value: Value = serde_json::from_str(row_json)
        .map_err(|e| corrupt(step, format_args!("row_json of {id} is not JSON: {e}")))?;
    let Some(obj) = value.as_object_mut() else {
        return Err(corrupt(
            step,
            format_args!("row_json of {id} is not an object"),
        ));
    };
    obj.remove("dependencies");
    serde_json::from_value(value).map_err(|e| {
        corrupt(
            step,
            format_args!("row_json of {id} does not match the row model: {e}"),
        )
    })
}

/// An edge row of the `edges` table.
pub(crate) fn decode_edge(
    step: Step<'_>,
    issue_id: &str,
    depends_on_id: &str,
    kind: &str,
) -> Result<Edge, CacheError> {
    let kind = DepType::parse(kind).ok_or_else(|| {
        corrupt(
            step,
            format_args!("edge {issue_id} -> {depends_on_id} has unknown type {kind:?}"),
        )
    })?;
    Ok(Edge::new(
        bead_id(step, issue_id)?,
        bead_id(step, depends_on_id)?,
        kind,
    ))
}

/// A stored bead id.
pub(crate) fn bead_id(step: Step<'_>, s: &str) -> Result<BeadId, CacheError> {
    BeadId::new(s).map_err(|e| corrupt(step, format_args!("stored bead id: {e}")))
}

/// A stored timestamp (RFC 3339; compared as an instant, never as text).
pub(crate) fn timestamp(step: Step<'_>, what: &str, s: &str) -> Result<Timestamp, CacheError> {
    Timestamp::parse(s).map_err(|e| corrupt(step, format_args!("stored {what}: {e}")))
}

/// An optional stored timestamp.
pub(crate) fn opt_timestamp(
    step: Step<'_>,
    what: &str,
    s: Option<&str>,
) -> Result<Option<Timestamp>, CacheError> {
    s.map(|s| timestamp(step, what, s)).transpose()
}

/// A stored scope name (the v3 `phase` column) as a scope id.
pub(crate) fn scope(step: Step<'_>, name: &str) -> Result<ScopeId, CacheError> {
    ScopeId::new(name).map_err(|e| corrupt(step, format_args!("stored scope name: {e}")))
}

/// True when `stored` is a later instant than `incoming` (second precision).
///
/// When either value does not parse as RFC 3339, the stored row is not newer
/// and the incoming row is stored; text is never ordered (ADR-MODEL-0003).
pub(crate) fn is_newer(stored: Option<&str>, incoming: &str) -> bool {
    match (
        stored.and_then(sc_beads::rfc3339_to_unix),
        sc_beads::rfc3339_to_unix(incoming),
    ) {
        (Some(s), Some(i)) => s > i,
        _ => false,
    }
}

/// One `journal` row as read from SQLite, before validation.
#[derive(Debug)]
pub(crate) struct JournalRow {
    pub(crate) seq: i64,
    pub(crate) idem_key: String,
    pub(crate) phase: Option<String>,
    pub(crate) bead: String,
    pub(crate) op: String,
    pub(crate) args_json: String,
    pub(crate) precondition_json: String,
    pub(crate) actor: String,
    pub(crate) state: String,
    pub(crate) attempts: i64,
    pub(crate) last_error: Option<String>,
    pub(crate) announced_at: Option<String>,
    pub(crate) created_at: String,
    pub(crate) applied_at: Option<String>,
    pub(crate) last_error_code: Option<String>,
    pub(crate) note: Option<String>,
    pub(crate) changed: Option<i64>,
}

/// The `journal` columns [`JournalRow`] reads, in order, as a literal (for `concat!`).
macro_rules! journal_columns {
    () => {
        "seq, idem_key, phase, bead, op, args_json, precondition_json, actor, state, \
         attempts, last_error, announced_at, created_at, applied_at, last_error_code, note, changed"
    };
}
pub(crate) use journal_columns;

/// The `journal` columns [`JournalRow`] reads, in order.
pub(crate) const JOURNAL_COLUMNS: &str = journal_columns!();

impl JournalRow {
    /// Reads the [`JOURNAL_COLUMNS`] of a result row.
    pub(crate) fn from_sql(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            seq: r.get(0)?,
            idem_key: r.get(1)?,
            phase: r.get(2)?,
            bead: r.get(3)?,
            op: r.get(4)?,
            args_json: r.get(5)?,
            precondition_json: r.get(6)?,
            actor: r.get(7)?,
            state: r.get(8)?,
            attempts: r.get(9)?,
            last_error: r.get(10)?,
            announced_at: r.get(11)?,
            created_at: r.get(12)?,
            applied_at: r.get(13)?,
            last_error_code: r.get(14)?,
            note: r.get(15)?,
            changed: r.get(16)?,
        })
    }

    /// Validates the row into a [`JournalEntry`].
    ///
    /// An entry `bd_cache.py` wrote has the extension columns NULL, or stale
    /// where it rewrote `last_error` after this store did, and is read as this
    /// store would have written it (ADR-STORE-0001):
    ///
    /// - `note` NULL on a resolved entry: a trailing `" | resolved by <actor>:
    ///   <note>"` of `last_error` is split out into `note`;
    /// - `last_error_code`: the stored code while `last_error` still starts
    ///   with `<code>: `, else the code of the text's `CODE: ` prefix;
    /// - `changed`: `None` when NULL.
    pub(crate) fn decode(self, step: Step<'_>) -> Result<JournalEntry, CacheError> {
        let seq = self.seq;
        let state = JournalState::parse(&self.state).ok_or_else(|| {
            corrupt(
                step,
                format_args!("journal entry {seq}: state: unknown state {:?}", self.state),
            )
        })?;
        let (last_error, note) = match (self.note, self.last_error) {
            (None, Some(text)) if state == JournalState::Resolved => {
                match split_resolve_note(&text) {
                    Some((error, note)) => (error.map(str::to_owned), Some(note.to_owned())),
                    None => (Some(text), None),
                }
            }
            (note, last_error) => (last_error, note),
        };
        let last_error_code = match (self.last_error_code, last_error.as_deref()) {
            (Some(code), Some(text))
                if text
                    .strip_prefix(code.as_str())
                    .is_some_and(|rest| rest.starts_with(": ")) =>
            {
                Some(code)
            }
            (_, text) => text.and_then(error_code_prefix).map(str::to_owned),
        };
        let bad = |what: &str, e: &dyn std::fmt::Display| {
            corrupt(step, format_args!("journal entry {seq}: {what}: {e}"))
        };
        // Journal text that no longer parses keeps its own code, with the entry named.
        let in_entry = |e: CacheError| match e {
            CacheError::BadOp { detail } => CacheError::BadOp {
                detail: format!("journal entry {seq}: {detail}"),
            },
            CacheError::BadArgs { detail } => CacheError::BadArgs {
                detail: format!("journal entry {seq}: {detail}"),
            },
            CacheError::BadPrecondition { detail } => CacheError::BadPrecondition {
                detail: format!("journal entry {seq}: {detail}"),
            },
            other => other,
        };
        Ok(JournalEntry {
            seq: Seq::new(seq),
            idem_key: IdemKey::new(self.idem_key).map_err(|e| bad("idem_key", &e))?,
            scope: self.phase.as_deref().map(|p| scope(step, p)).transpose()?,
            bead: BeadId::new(self.bead).map_err(|e| bad("bead", &e))?,
            op: Op::parse(&self.op, &self.args_json).map_err(in_entry)?,
            precondition: parse_precondition(&self.precondition_json).map_err(in_entry)?,
            actor: Actor::new(self.actor).map_err(|e| bad("actor", &e))?,
            state,
            attempts: u32::try_from(self.attempts).map_err(|e| bad("attempts", &e))?,
            last_error,
            last_error_code,
            note,
            changed: self.changed.map(|v| v != 0),
            announced_at: opt_timestamp(step, "announced_at", self.announced_at.as_deref())?,
            created_at: timestamp(step, "created_at", &self.created_at)?,
            applied_at: opt_timestamp(step, "applied_at", self.applied_at.as_deref())?,
        })
    }
}

/// The `blocked_json` / `ready_json` texts of a ready base.
pub(crate) fn encode_ready(
    step: Step<'_>,
    blocked: &[BlockedRow],
    ready: &[BeadId],
) -> Result<(String, String), CacheError> {
    let blocked = serde_json::to_string(blocked).map_err(|e| failed(step, e))?;
    let ready = serde_json::to_string(ready).map_err(|e| failed(step, e))?;
    Ok((blocked, ready))
}

/// A stored ready base's lists; ids are validated as they deserialize.
pub(crate) fn decode_ready(
    step: Step<'_>,
    blocked: &str,
    ready: &str,
) -> Result<(Vec<BlockedRow>, Vec<BeadId>), CacheError> {
    let blocked = serde_json::from_str(blocked)
        .map_err(|e| corrupt(step, format_args!("ready base blocked_json: {e}")))?;
    let ready = serde_json::from_str(ready)
        .map_err(|e| corrupt(step, format_args!("ready base ready_json: {e}")))?;
    Ok((blocked, ready))
}

/// The rows of the per-bead index of a ready base: each blocked bead with its
/// blockers merged in order without repeats; a bead with no blockers is not
/// blocked and has no row.
pub(crate) fn index_blocked(
    step: Step<'_>,
    blocked: &[BlockedRow],
) -> Result<Vec<(BeadId, String)>, CacheError> {
    let mut merged: std::collections::BTreeMap<&BeadId, Vec<&BeadId>> =
        std::collections::BTreeMap::new();
    for row in blocked {
        let list = merged.entry(&row.id).or_default();
        for b in &row.blocked_by {
            if !list.contains(&b) {
                list.push(b);
            }
        }
    }
    merged
        .into_iter()
        .filter(|(_, list)| !list.is_empty())
        .map(|(id, list)| {
            let json = serde_json::to_string(&list).map_err(|e| failed(step, e))?;
            Ok((id.clone(), json))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_compares_instants_and_never_text() {
        assert!(is_newer(
            Some("2026-10-23T10:00:02Z"),
            "2026-10-23T10:00:01Z"
        ));
        assert!(!is_newer(
            Some("2026-10-23T12:00:00+02:00"),
            "2026-10-23T10:00:01Z"
        ));
        assert!(!is_newer(
            Some("2026-10-23T10:00:01.9Z"),
            "2026-10-23T09:00:01-01:00"
        ));
        assert!(!is_newer(Some("not-a-time"), "2026-10-23T10:00:00Z"));
        assert!(!is_newer(Some("2026-10-23T10:00:00Z"), "not-a-time"));
        assert!(!is_newer(None, "2026-10-23T10:00:00Z"));
    }
}
