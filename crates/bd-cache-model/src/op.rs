//! Journal operations: [`Op`] with parse, canonical args, overlay and achieved; [`replay`].
//!
//! The op table (BD-CACHE.md, Journal operations; REQ-BDC-0301):
//!
//! | op | args | overlay | achieved when |
//! |---|---|---|---|
//! | `close` | `{reason?}` | status `closed`, `close_reason` = reason | status is `closed` |
//! | `update` | nonempty subset of `status`, `assignee`, `add_labels`, `remove_labels`, `set_metadata`, `unset_metadata`, `append_notes` | each field, in that order | every requested field already holds |
//! | `dep_add` | `{depends_on, type}` | the edge, if absent | the edge is present |
//!
//! `update` also accepts the `/v0` form `metadata: {set, unset}` beside
//! `set_metadata` / `unset_metadata` (ADR-BDC-0003); [`Op::to_args_json`]
//! always writes the BD-CACHE form.
//!
//! Metadata values must survive bd's `--set-metadata` coercion
//! (REQ-BDC-0303): an int, a bool, or a string with no surrounding whitespace
//! that does not parse as a JSON number, boolean or null (`NaN` and
//! `Infinity` included, as Python's `json.loads` reads them). Floats, nulls,
//! arrays, objects and integers outside `i64` are `BAD_ARGS`, and so is a
//! metadata key that is empty or contains `=` (bd's `key=value` separator). A dependency
//! type must be one of bd's ([`DepType`], REQ-BDC-0304).
//!
//! Empty collections in `update` args mean "not requested"; an update that
//! requests nothing is `BAD_ARGS`.

use std::collections::BTreeMap;

use sc_beads::{Assignee, Bead, BeadId, DepType, Edge, Guards, MetaValue, Patch};
use serde_json::{Map, Value};

use crate::error::{CacheError, GuardKey, Unmet};
use crate::journal::{JournalEntry, JournalState};
use crate::pyjson::python_json;
use crate::scope::{PhaseName, ScopeId};

/// The status text of a closed bead (bd's `closed`): the one literal the model, the
/// cache and its callers compare against.
pub const CLOSED_STATUS: &str = "closed";

/// Op names, in BD-CACHE order.
pub const OPS: [&str; 3] = ["close", "update", "dep_add"];

const UPDATE_KEYS: [&str; 8] = [
    "status",
    "assignee",
    "add_labels",
    "remove_labels",
    "set_metadata",
    "unset_metadata",
    "append_notes",
    "metadata",
];

/// One journaled write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// `bd close <id> [--reason R]`.
    Close {
        /// The close reason.
        reason: Option<String>,
    },
    /// `bd update <id> ...`.
    Update(Patch),
    /// `bd dep add <id> <depends_on> --type T`.
    DepAdd {
        /// The bead depended on.
        depends_on: BeadId,
        /// The dependency type.
        kind: DepType,
    },
}

fn bad_args(detail: impl Into<String>) -> CacheError {
    CacheError::BadArgs {
        detail: detail.into(),
    }
}

fn string_of(v: &Value, what: &str) -> Result<String, CacheError> {
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| bad_args(format!("{what} must be a string")))
}

fn strings_of(v: &Value, what: &str) -> Result<Vec<String>, CacheError> {
    let items = v
        .as_array()
        .ok_or_else(|| bad_args(format!("{what} must be an array of strings")))?;
    items.iter().map(|i| string_of(i, what)).collect()
}

/// True when `s` reads as a JSON number, boolean or null (Python's `json.loads`,
/// which also reads `NaN`, `Infinity` and `-Infinity`).
fn looks_scalar(s: &str) -> bool {
    if matches!(
        s,
        "true" | "false" | "null" | "NaN" | "Infinity" | "-Infinity"
    ) {
        return true;
    }
    // JSON number grammar: -?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?
    let b = s.as_bytes();
    let mut i = usize::from(b.first() == Some(&b'-'));
    let digits = |i: &mut usize| {
        let start = *i;
        while b.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        *i - start
    };
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => {
            digits(&mut i);
        }
        _ => return false,
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        if digits(&mut i) == 0 {
            return false;
        }
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return false;
        }
    }
    i == b.len()
}

/// Parses one `set_metadata` value, applying the coercion guard (REQ-BDC-0303).
///
/// # Errors
///
/// `BAD_ARGS` for a value bd would not store as given.
pub fn meta_value(key: &str, v: &Value) -> Result<MetaValue, CacheError> {
    let refuse = || {
        bad_args(format!(
            "set_metadata {key}={v} would not be stored as given: bd coerces numeric, boolean and \
             null-looking text and has no float or structured values"
        ))
    };
    match v {
        Value::Bool(b) => Ok(MetaValue::Bool(*b)),
        Value::Number(n) => n.as_i64().map(MetaValue::Int).ok_or_else(refuse),
        Value::String(s) if s.trim() == s && !looks_scalar(s) => Ok(MetaValue::Str(s.clone())),
        _ => Err(refuse()),
    }
}

/// Refuses a `set_metadata` key `bd --set-metadata key=value` cannot carry: an
/// empty key, or one holding the `=` that separates key from value
/// (PE-RBP-F004). Refused by [`Op::parse_new`], at write time, it is one
/// `BAD_ARGS`; journaled, the drain would retry it `max_attempts` times before
/// escalating. Stored entries are decoded with [`Op::parse`], which does not
/// apply it, so a journal written by an earlier build still decodes and drains.
fn check_metadata_key(key: &str) -> Result<(), CacheError> {
    if key.is_empty() {
        return Err(bad_args("set_metadata key is empty; bd needs key=value"));
    }
    if key.contains('=') {
        return Err(bad_args(format!(
            "set_metadata key {key:?} contains '=', which bd --set-metadata key=value reads as \
             the end of the key"
        )));
    }
    Ok(())
}

fn parse_patch(args: &Map<String, Value>) -> Result<Patch, CacheError> {
    if let Some(k) = args.keys().find(|k| !UPDATE_KEYS.contains(&k.as_str())) {
        return Err(bad_args(format!(
            "unknown update key {k:?}; update keys must be a nonempty subset of {}",
            UPDATE_KEYS.join(", ")
        )));
    }
    let mut p = Patch::default();
    if let Some(v) = args.get("status") {
        p.status = Some(string_of(v, "status")?);
    }
    if let Some(v) = args.get("assignee") {
        p.assignee = Some(Assignee::from_wire(&string_of(v, "assignee")?));
    }
    if let Some(v) = args.get("add_labels") {
        p.add_labels = strings_of(v, "add_labels")?;
    }
    if let Some(v) = args.get("remove_labels") {
        p.remove_labels = strings_of(v, "remove_labels")?;
    }
    let mut set_maps: Vec<&Map<String, Value>> = Vec::new();
    let mut unsets: Vec<&Value> = Vec::new();
    if let Some(v) = args.get("set_metadata") {
        set_maps.push(
            v.as_object()
                .ok_or_else(|| bad_args("set_metadata must be an object"))?,
        );
    }
    if let Some(v) = args.get("unset_metadata") {
        unsets.push(v);
    }
    if let Some(v) = args.get("metadata") {
        let m = v
            .as_object()
            .ok_or_else(|| bad_args("metadata must be an object {set, unset}"))?;
        if let Some(k) = m.keys().find(|k| !matches!(k.as_str(), "set" | "unset")) {
            return Err(bad_args(format!(
                "unknown metadata key {k:?}; use set and unset"
            )));
        }
        if let Some(s) = m.get("set") {
            set_maps.push(
                s.as_object()
                    .ok_or_else(|| bad_args("metadata.set must be an object"))?,
            );
        }
        if let Some(u) = m.get("unset") {
            unsets.push(u);
        }
    }
    for map in set_maps {
        for (k, v) in map {
            let value = meta_value(k, v)?;
            if p.set_metadata.insert(k.clone(), value).is_some() {
                return Err(bad_args(format!("metadata key {k:?} is set twice")));
            }
        }
    }
    for u in unsets {
        for k in strings_of(u, "unset_metadata")? {
            if !p.unset_metadata.contains(&k) {
                p.unset_metadata.push(k);
            }
        }
    }
    if let Some(v) = args.get("append_notes") {
        let text = string_of(v, "append_notes")?;
        if !text.is_empty() {
            p.append_notes = Some(text);
        }
    }
    if p.is_empty() {
        return Err(bad_args("update must request at least one change"));
    }
    Ok(p)
}

impl Op {
    /// Parses a new write request: [`Op::parse`], plus the checks only a new
    /// write gets, so a stored entry that predates them still decodes.
    ///
    /// # Errors
    ///
    /// Everything [`Op::parse`] refuses, and `BAD_ARGS` for a metadata key
    /// that is empty or contains `=` (PE-RBP-F004).
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::Op;
    /// let args = r#"{"set_metadata": {"a=b": "x"}}"#;
    /// assert_eq!(Op::parse_new("update", args).unwrap_err().code(), "BD_CACHE.BAD_ARGS");
    /// assert!(Op::parse("update", args).is_ok(), "a stored entry still decodes");
    /// ```
    pub fn parse_new(op: &str, args_json: &str) -> Result<Op, CacheError> {
        let parsed = Self::parse(op, args_json)?;
        if let Op::Update(patch) = &parsed {
            for key in patch.set_metadata.keys() {
                check_metadata_key(key)?;
            }
        }
        Ok(parsed)
    }

    /// Parses an op and its args JSON: the decoding shared by new writes
    /// ([`Op::parse_new`]) and stored journal entries.
    ///
    /// # Errors
    ///
    /// `BAD_OP` for an unknown op name; `BAD_ARGS` for args that are not a
    /// JSON object, unknown or mistyped keys, an empty update, a metadata
    /// value bd would coerce, or an unknown dependency type.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::Op;
    /// let op = Op::parse("dep_add", r#"{"depends_on": "b1", "type": "blocks"}"#).unwrap();
    /// assert_eq!(op.to_args_json(), r#"{"depends_on": "b1", "type": "blocks"}"#);
    /// assert_eq!(Op::parse("delete", "{}").unwrap_err().code(), "BD_CACHE.BAD_OP");
    /// let bad = Op::parse("update", r#"{"set_metadata": {"k": "5"}}"#).unwrap_err();
    /// assert_eq!(bad.code(), "BD_CACHE.BAD_ARGS");
    /// ```
    pub fn parse(op: &str, args_json: &str) -> Result<Op, CacheError> {
        let args = || -> Result<Map<String, Value>, CacheError> {
            let args: Value = serde_json::from_str(args_json)
                .map_err(|e| bad_args(format!("args are not JSON: {e}")))?;
            match args {
                Value::Object(m) => Ok(m),
                _ => Err(bad_args("args must be a JSON object")),
            }
        };
        match op {
            "close" => {
                let args = args()?;
                if let Some(k) = args.keys().find(|k| k.as_str() != "reason") {
                    return Err(bad_args(format!("close takes only reason, got {k:?}")));
                }
                let reason = args
                    .get("reason")
                    .map(|v| string_of(v, "reason"))
                    .transpose()?;
                Ok(Op::Close { reason })
            }
            "update" => parse_patch(&args()?).map(Op::Update),
            "dep_add" => {
                let args = args()?;
                let keys: Vec<&str> = args.keys().map(String::as_str).collect();
                if keys != ["depends_on", "type"] {
                    return Err(bad_args("dep_add takes depends_on and type"));
                }
                let depends_on = args
                    .get("depends_on")
                    .map_or(Ok(String::new()), |v| string_of(v, "depends_on"))?;
                let depends_on = BeadId::new(depends_on)
                    .map_err(|e| bad_args(format!("dep_add depends_on: {e}")))?;
                let t = args
                    .get("type")
                    .map_or(Ok(String::new()), |v| string_of(v, "type"))?;
                let kind = DepType::parse(&t).ok_or_else(|| {
                    bad_args(format!(
                        "unknown dependency type {t:?}; use a bd dependency type"
                    ))
                })?;
                Ok(Op::DepAdd { depends_on, kind })
            }
            _ => Err(CacheError::BadOp {
                detail: format!("unknown op {op:?}; use one of {}", OPS.join(", ")),
            }),
        }
    }

    /// The op name: `close`, `update` or `dep_add`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Op::Close { .. } => "close",
            Op::Update(_) => "update",
            Op::DepAdd { .. } => "dep_add",
        }
    }

    /// The args as a JSON value, in the BD-CACHE vocabulary.
    #[must_use]
    pub fn args_value(&self) -> Value {
        let mut m = Map::new();
        match self {
            Op::Close { reason } => {
                if let Some(r) = reason {
                    m.insert("reason".into(), Value::String(r.clone()));
                }
            }
            Op::Update(p) => {
                let strings =
                    |v: &[String]| Value::Array(v.iter().cloned().map(Value::String).collect());
                if let Some(s) = &p.status {
                    m.insert("status".into(), Value::String(s.clone()));
                }
                if let Some(a) = &p.assignee {
                    m.insert("assignee".into(), Value::String(a.as_wire().to_owned()));
                }
                if !p.add_labels.is_empty() {
                    m.insert("add_labels".into(), strings(&p.add_labels));
                }
                if !p.remove_labels.is_empty() {
                    m.insert("remove_labels".into(), strings(&p.remove_labels));
                }
                if !p.set_metadata.is_empty() {
                    let set = p
                        .set_metadata
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_json()))
                        .collect();
                    m.insert("set_metadata".into(), Value::Object(set));
                }
                if !p.unset_metadata.is_empty() {
                    m.insert("unset_metadata".into(), strings(&p.unset_metadata));
                }
                if let Some(n) = &p.append_notes {
                    m.insert("append_notes".into(), Value::String(n.clone()));
                }
            }
            Op::DepAdd { depends_on, kind } => {
                m.insert(
                    "depends_on".into(),
                    Value::String(depends_on.as_str().to_owned()),
                );
                m.insert("type".into(), Value::String(kind.as_str().to_owned()));
            }
        }
        Value::Object(m)
    }

    /// The canonical `args_json`, byte-identical to Python's `json.dumps(args, sort_keys=True)`.
    #[must_use]
    pub fn to_args_json(&self) -> String {
        python_json(&self.args_value())
    }

    /// The bead as it will be once this op is written (pure).
    #[must_use]
    pub fn overlay(&self, row: &Bead) -> Bead {
        let mut row = row.clone();
        match self {
            Op::Close { reason } => {
                row.status = Some(CLOSED_STATUS.into());
                if let Some(r) = reason {
                    row.close_reason = Some(r.clone());
                }
            }
            Op::Update(p) => {
                if let Some(s) = &p.status {
                    row.status = Some(s.clone());
                }
                if let Some(a) = &p.assignee {
                    // As bd stores it: an unassigned bead has no assignee key.
                    row.assignee = match a {
                        Assignee::Unassigned => None,
                        Assignee::Named(n) => Some(n.clone()),
                    };
                }
                row.labels.retain(|l| !p.remove_labels.contains(l));
                for l in &p.add_labels {
                    if !row.labels.contains(l) {
                        row.labels.push(l.clone());
                    }
                }
                for (k, v) in &p.set_metadata {
                    row.metadata.insert(k.clone(), v.to_json());
                }
                for k in &p.unset_metadata {
                    row.metadata.remove(k);
                }
                if let Some(text) = &p.append_notes {
                    row.notes = Some(match row.notes.take().filter(|n| !n.is_empty()) {
                        Some(n) => format!("{n}\n{text}"),
                        None => text.clone(),
                    });
                }
            }
            Op::DepAdd { depends_on, kind } => {
                let edge = Edge::new(row.id.clone(), depends_on.clone(), *kind);
                if !row.dependencies.contains(&edge) {
                    row.dependencies.push(edge);
                }
            }
        }
        row
    }

    /// True when the backend row already shows this op's effect (a write that landed before a crash).
    #[must_use]
    pub fn achieved(&self, row: &Bead) -> bool {
        match self {
            Op::Close { .. } => row.status.as_deref() == Some(CLOSED_STATUS),
            Op::DepAdd { depends_on, kind } => row
                .dependencies
                .iter()
                .any(|e| e.issue_id == row.id && e.depends_on_id == *depends_on && e.kind == *kind),
            Op::Update(p) => {
                p.status
                    .as_ref()
                    .is_none_or(|s| row.status.as_ref() == Some(s))
                    && p.assignee
                        .as_ref()
                        .is_none_or(|a| a.matches(row.assignee.as_deref()))
                    && p.add_labels.iter().all(|l| row.labels.contains(l))
                    && !p.remove_labels.iter().any(|l| row.labels.contains(l))
                    && p.set_metadata
                        .iter()
                        .all(|(k, v)| row.metadata.get(k).is_some_and(|got| v.matches(got)))
                    && !p
                        .unset_metadata
                        .iter()
                        .any(|k| row.metadata.contains_key(k))
                    && p.append_notes
                        .as_deref()
                        .is_none_or(|t| row.notes.as_deref().unwrap_or("").contains(t))
            }
        }
    }
}

/// The view of a bead: `row` with every `Pending` entry overlaid in `seq` order.
///
/// Escalated, resolved and applied entries are skipped (REQ-BDC-0202, 0305).
#[must_use]
pub fn replay(row: &Bead, pending: &[JournalEntry]) -> Bead {
    let mut entries: Vec<&JournalEntry> = pending
        .iter()
        .filter(|e| e.state == JournalState::Pending)
        .collect();
    entries.sort_by_key(|e| e.seq);
    entries.iter().fold(row.clone(), |r, e| e.op.overlay(&r))
}

/// The stored form of a row (REQ-BDC-0108): derived counts dropped, outgoing
/// edges only, sorted and without duplicates.
///
/// A store's `get_row(upsert(b))` equals `normalize(&b)`.
#[must_use]
pub fn normalize(row: &Bead) -> Bead {
    let mut b = row.clone();
    b.dependency_count = None;
    b.dependent_count = None;
    b.dependencies.retain(|e| e.issue_id == b.id);
    b.dependencies.sort();
    b.dependencies.dedup();
    b
}

/// The first valid `phase-<x>` label's suffix: the v3 `phase` column.
#[must_use]
pub fn phase_of(row: &Bead) -> Option<PhaseName> {
    ScopeId::from_labels(&row.labels).map(|s| s.phase_name().clone())
}

/// Parses a write precondition: `{status?, assignee?}`, with the `/v0` aliases
/// `expected_status` / `expected_assignee` (ADR-BDC-0003). `""` means
/// unassigned or empty.
///
/// # Errors
///
/// `BAD_PRECONDITION` for invalid JSON, unknown keys, non-string values, or
/// an alias that disagrees with the key it names.
pub fn parse_precondition(json: &str) -> Result<Guards, CacheError> {
    let bad = |detail: String| CacheError::BadPrecondition { detail };
    let text = if json.trim().is_empty() { "{}" } else { json };
    let v: Value =
        serde_json::from_str(text).map_err(|e| bad(format!("precondition is not JSON: {e}")))?;
    let Value::Object(m) = v else {
        return Err(bad("precondition must be a JSON object".into()));
    };
    let mut status: Option<String> = None;
    let mut assignee: Option<String> = None;
    for (k, v) in &m {
        let slot = match k.as_str() {
            "status" | "expected_status" => &mut status,
            "assignee" | "expected_assignee" => &mut assignee,
            _ => {
                return Err(bad(format!(
                    "unknown precondition key {k:?}; use status and assignee"
                )));
            }
        };
        let s = v
            .as_str()
            .ok_or_else(|| bad(format!("precondition {k} must be a string")))?
            .to_owned();
        if slot.as_ref().is_some_and(|prev| *prev != s) {
            return Err(bad(format!("precondition {k} disagrees with its alias")));
        }
        *slot = Some(s);
    }
    Ok(Guards {
        status,
        assignee: assignee.as_deref().map(Assignee::from_wire),
    })
}

/// The canonical `precondition_json`, byte-identical to Python's `json.dumps(..., sort_keys=True)`.
#[must_use]
pub fn precondition_json(g: &Guards) -> String {
    let mut m = Map::new();
    if let Some(a) = &g.assignee {
        m.insert("assignee".into(), Value::String(a.as_wire().to_owned()));
    }
    if let Some(s) = &g.status {
        m.insert("status".into(), Value::String(s.clone()));
    }
    python_json(&Value::Object(m))
}

/// The precondition keys `row` does not meet, with expected and actual values.
#[must_use]
pub fn unmet(row: &Bead, g: &Guards) -> BTreeMap<GuardKey, Unmet> {
    let mut out = BTreeMap::new();
    let mut check = |key: GuardKey, want: Option<&str>, have: Option<&str>| {
        if let Some(want) = want {
            let actual = have.unwrap_or_default();
            if want != actual {
                out.insert(
                    key,
                    Unmet {
                        expected: want.to_owned(),
                        actual: actual.to_owned(),
                    },
                );
            }
        }
    };
    check(GuardKey::Status, g.status.as_deref(), row.status.as_deref());
    check(
        GuardKey::Assignee,
        g.assignee.as_ref().map(Assignee::as_wire),
        row.assignee.as_deref(),
    );
    out
}
