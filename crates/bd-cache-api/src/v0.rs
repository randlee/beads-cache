//! `/v0` names over the stored BD-CACHE args (contract §6 `Entry`, §5.3).
//!
//! The journal keeps the BD-CACHE vocabulary (`set_metadata`,
//! `unset_metadata`, `depends_on`), and the store is never rewritten. The
//! api speaks `/v0` both ways:
//!
//! - [`patch_args`]: an `issues update` patch to the BD-CACHE args that
//!   `write_returning` journals (`metadata.set` → `set_metadata`,
//!   `metadata.unset` → `unset_metadata`);
//! - [`entry_args`]: a stored op to the `/v0` form of `Entry.args`: the
//!   update patch, `{reason?}` for a close, `{edges: [edge]}` for a dep add.
//!
//! A patch decoded and journaled reads back as the same patch.

use bd_cache_model::Op;
use sc_beads::{BeadId, Patch};
use serde_json::{Map, Value, json};

use crate::request::{IssuePatch, MetadataPatch};

/// The BD-CACHE args object an `issues update` patch journals.
///
/// # Examples
///
/// ```
/// use bd_cache_api::{Command, Request, decode, patch_args};
/// let req = serde_json::json!({"id": "b1", "actor": "san", "idempotency_key": "k",
///     "patch": {"metadata": {"set": {"k": "v"}, "unset": ["old"]}}});
/// let Ok(Request::IssuesUpdate(u)) = decode(Command::IssuesUpdate, &req) else { panic!() };
/// assert_eq!(patch_args(u.patch()),
///     serde_json::json!({"set_metadata": {"k": "v"}, "unset_metadata": ["old"]}));
/// ```
#[must_use]
pub fn patch_args(p: &IssuePatch) -> Value {
    let strings = |v: &[String]| Value::Array(v.iter().cloned().map(Value::String).collect());
    let mut m = Map::new();
    if let Some(s) = &p.status {
        m.insert("status".into(), Value::String(s.clone()));
    }
    if let Some(a) = &p.assignee {
        m.insert("assignee".into(), Value::String(a.clone()));
    }
    if !p.add_labels.is_empty() {
        m.insert("add_labels".into(), strings(&p.add_labels));
    }
    if !p.remove_labels.is_empty() {
        m.insert("remove_labels".into(), strings(&p.remove_labels));
    }
    if let Some(meta) = &p.metadata {
        if !meta.set.is_empty() {
            m.insert("set_metadata".into(), Value::Object(meta.set.clone()));
        }
        if !meta.unset.is_empty() {
            m.insert("unset_metadata".into(), strings(&meta.unset));
        }
    }
    if let Some(n) = &p.append_notes {
        m.insert("append_notes".into(), Value::String(n.clone()));
    }
    Value::Object(m)
}

/// The `/v0` patch of a stored update.
pub(crate) fn patch_of(p: &Patch) -> IssuePatch {
    let metadata =
        (!p.set_metadata.is_empty() || !p.unset_metadata.is_empty()).then(|| MetadataPatch {
            set: p
                .set_metadata
                .iter()
                .map(|(k, v)| (k.clone(), v.to_json()))
                .collect(),
            unset: p.unset_metadata.clone(),
        });
    IssuePatch {
        status: p.status.clone(),
        assignee: p.assignee.as_ref().map(|a| a.as_wire().to_owned()),
        add_labels: p.add_labels.clone(),
        remove_labels: p.remove_labels.clone(),
        append_notes: p.append_notes.clone(),
        metadata,
    }
}

/// `Entry.args`: the stored op of `bead` in `/v0` names.
///
/// # Examples
///
/// ```
/// use bd_cache_api::entry_args;
/// use bd_cache_model::Op;
/// let op = Op::parse("update", r#"{"set_metadata": {"k": 1}}"#).unwrap();
/// let bead = sc_beads::BeadId::new("b1").unwrap();
/// assert_eq!(entry_args(&op, &bead), serde_json::json!({"metadata": {"set": {"k": 1}}}));
/// ```
#[must_use]
pub fn entry_args(op: &Op, bead: &BeadId) -> Value {
    match op {
        Op::Close { reason } => match reason {
            Some(r) => json!({"reason": r}),
            None => json!({}),
        },
        Op::Update(p) => json!(patch_of(p)),
        Op::DepAdd { depends_on, kind } => json!({
            "edges": [{"issue_id": bead, "depends_on_id": depends_on, "type": kind.as_str()}]
        }),
    }
}
