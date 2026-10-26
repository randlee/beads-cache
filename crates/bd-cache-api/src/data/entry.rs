//! `Entry`: one journal entry in the api's vocabulary (contract §6).

use bd_cache_model::{JournalEntry, JournalState, Op};
use sc_beads::BeadId;
use serde::Serialize;
use serde_json::Value;

use crate::data::x_cache::{OpError, cache_time, opt_time};
use crate::v0::entry_args;

/// A journal entry: `{seq, idempotency_key, scope, bead, op, args,
/// expected_status, expected_assignee, actor, state, attempts, last_error,
/// note, announced_at, created_at, applied_at}`.
///
/// `args` is the `/v0` form of the stored op ([`entry_args`]); the store
/// keeps the BD-CACHE form and is not rewritten. Every member is always
/// present; `null` means none.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    /// The sequence number.
    pub seq: i64,
    /// The caller's key.
    pub idempotency_key: String,
    /// The bead's scope when journaled, by its name as written (ADR-BDC-0013).
    pub scope: Option<String>,
    /// The bead written.
    pub bead: BeadId,
    /// `close`, `update` or `dep_add`.
    pub op: EntryOp,
    /// The write in `/v0` names.
    pub args: Value,
    /// The status guard.
    pub expected_status: Option<String>,
    /// The assignee guard (`""` = unassigned).
    pub expected_assignee: Option<String>,
    /// Who wrote it.
    pub actor: String,
    /// `pending`, `applied`, `escalated` or `resolved`.
    pub state: EntryState,
    /// Failed attempts on a reachable backend.
    pub attempts: u32,
    /// The latest failure or escalation reason.
    pub last_error: Option<OpError>,
    /// The resolve note, `resolved by <actor>: <note>` (lead ruling S65).
    pub note: Option<String>,
    /// When the escalation was announced.
    pub announced_at: Option<String>,
    /// When it was journaled.
    pub created_at: String,
    /// When it was marked applied.
    pub applied_at: Option<String>,
}

impl Entry {
    /// The api form of a stored entry.
    #[must_use]
    pub fn of(e: &JournalEntry) -> Self {
        Self {
            seq: e.seq.get(),
            idempotency_key: e.idem_key.as_str().to_owned(),
            scope: e.scope.as_ref().map(|s| s.name().to_owned()),
            bead: e.bead.clone(),
            op: EntryOp::of(&e.op),
            args: entry_args(&e.op, &e.bead),
            expected_status: e.precondition.status.clone(),
            expected_assignee: e
                .precondition
                .assignee
                .as_ref()
                .map(|a| a.as_wire().to_owned()),
            actor: e.actor.as_str().to_owned(),
            state: e.state.into(),
            attempts: e.attempts,
            last_error: e
                .last_error
                .as_deref()
                .map(|t| OpError::from_parts(e.last_error_code.as_deref(), t)),
            note: e.note.clone(),
            announced_at: opt_time(e.announced_at.as_ref()),
            created_at: cache_time(&e.created_at),
            applied_at: opt_time(e.applied_at.as_ref()),
        }
    }
}

/// A journal entry's state on the wire ([`JournalState`], one variant each),
/// serialized as `pending`, `applied`, `escalated` or `resolved`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    /// [`JournalState::Pending`].
    Pending,
    /// [`JournalState::Applied`].
    Applied,
    /// [`JournalState::Escalated`].
    Escalated,
    /// [`JournalState::Resolved`].
    Resolved,
}

impl From<JournalState> for EntryState {
    fn from(s: JournalState) -> Self {
        match s {
            JournalState::Pending => Self::Pending,
            JournalState::Applied => Self::Applied,
            JournalState::Escalated => Self::Escalated,
            JournalState::Resolved => Self::Resolved,
        }
    }
}

/// A journal entry's op on the wire ([`Op`]'s kind), serialized as `close`,
/// `update` or `dep_add`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryOp {
    /// [`Op::Close`].
    Close,
    /// [`Op::Update`].
    Update,
    /// [`Op::DepAdd`].
    DepAdd,
}

impl EntryOp {
    /// The kind of `op`.
    #[must_use]
    pub fn of(op: &Op) -> Self {
        match op {
            Op::Close { .. } => Self::Close,
            Op::Update(_) => Self::Update,
            Op::DepAdd { .. } => Self::DepAdd,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// B9Q1-036: the typed state and op serialize byte-identically to the
    /// stored names they replace.
    #[test]
    fn the_typed_state_and_op_keep_the_wire_names() {
        for s in [
            JournalState::Pending,
            JournalState::Applied,
            JournalState::Escalated,
            JournalState::Resolved,
        ] {
            assert_eq!(
                serde_json::to_value(EntryState::from(s)).ok(),
                Some(serde_json::Value::from(s.as_str()))
            );
        }
        let close = Op::Close { reason: None };
        assert_eq!(
            serde_json::to_value(EntryOp::of(&close)).ok(),
            Some(serde_json::Value::from(close.name()))
        );
        assert_eq!(
            serde_json::to_value(EntryOp::DepAdd).ok(),
            Some(serde_json::Value::from("dep_add"))
        );
    }
}
