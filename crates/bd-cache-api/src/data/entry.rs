//! `Entry`: one journal entry in the api's vocabulary (contract §6).

use bd_cache_model::{JournalEntry, ScopeId};
use sc_beads::BeadId;
use serde::Serialize;
use serde_json::Value;

use crate::b1_stub;
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
    /// The bead's scope when journaled, in label form.
    pub scope: Option<String>,
    /// The bead written.
    pub bead: BeadId,
    /// `close`, `update` or `dep_add`.
    pub op: &'static str,
    /// The write in `/v0` names.
    pub args: Value,
    /// The status guard.
    pub expected_status: Option<String>,
    /// The assignee guard (`""` = unassigned).
    pub expected_assignee: Option<String>,
    /// Who wrote it.
    pub actor: String,
    /// `pending`, `applied`, `escalated` or `resolved`.
    pub state: &'static str,
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
        let extras = b1_stub::extras(e);
        Self {
            seq: e.seq.get(),
            idempotency_key: e.idem_key.as_str().to_owned(),
            scope: e.scope.as_ref().map(ScopeId::label),
            bead: e.bead.clone(),
            op: e.op.name(),
            args: entry_args(&e.op, &e.bead),
            expected_status: e.precondition.status.clone(),
            expected_assignee: e
                .precondition
                .assignee
                .as_ref()
                .map(|a| a.as_wire().to_owned()),
            actor: e.actor.as_str().to_owned(),
            state: e.state.as_str(),
            attempts: e.attempts,
            last_error: extras
                .last_error
                .as_deref()
                .map(|t| OpError::from_parts(extras.last_error_code.as_deref(), t)),
            note: extras.note,
            announced_at: opt_time(e.announced_at.as_ref()),
            created_at: cache_time(&e.created_at),
            applied_at: opt_time(e.applied_at.as_ref()),
        }
    }
}
