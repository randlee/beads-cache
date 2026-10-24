//! The write journal: [`Seq`], [`IdemKey`], [`JournalState`], [`JournalEntry`], [`NewEntry`].

use std::fmt;

use sc_beads::{Actor, BeadId, Guards};
use serde::{Serialize, Serializer};

use crate::op::Op;
use crate::scope::ScopeId;

/// A journal sequence number (`journal.seq`), assigned by the store from 1 upwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Seq(i64);

impl Seq {
    /// Wraps a sequence number.
    #[must_use]
    pub fn new(seq: i64) -> Self {
        Self(seq)
    }

    /// The number.
    #[must_use]
    pub fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Seq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The caller's idempotency key (REQ-BDC-0004): unique across the journal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct IdemKey(String);

impl IdemKey {
    /// Wraps a key.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// The key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdemKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for IdemKey {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// The state of a journal entry (`journal.state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JournalState {
    /// Waiting for the drain; replayed on reads.
    Pending,
    /// Landed in bd (verified, or marked with a read-back error).
    Applied,
    /// Sent to the phase lead; never replayed.
    Escalated,
    /// Settled by hand after an escalation; never replayed.
    Resolved,
}

impl JournalState {
    /// The stored name: `pending`, `applied`, `escalated` or `resolved`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Escalated => "escalated",
            Self::Resolved => "resolved",
        }
    }

    /// Parses a stored name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "applied" => Some(Self::Applied),
            "escalated" => Some(Self::Escalated),
            "resolved" => Some(Self::Resolved),
            _ => None,
        }
    }
}

impl Serialize for JournalState {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl fmt::Display for JournalState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One journal row.
///
/// Serializes with `op` as its name and `args` as the BD-CACHE args object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    /// Sequence number.
    pub seq: Seq,
    /// Idempotency key.
    pub idem_key: IdemKey,
    /// The scope (phase) the bead was in when the write was journaled.
    pub scope: Option<ScopeId>,
    /// The bead written.
    pub bead: BeadId,
    /// The write.
    pub op: Op,
    /// The precondition checked at write time and again at drain time.
    pub precondition: Guards,
    /// Who wrote it.
    pub actor: Actor,
    /// Current state.
    pub state: JournalState,
    /// Failed write attempts on a reachable backend.
    pub attempts: u32,
    /// The latest error, escalation reason or resolve note.
    pub last_error: Option<String>,
    /// When the escalation was announced to the lead.
    pub announced_at: Option<String>,
    /// When it was journaled (RFC 3339).
    pub created_at: String,
    /// When it was marked applied, set by `apply_with_readback`.
    pub applied_at: Option<String>,
}

impl Serialize for JournalEntry {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a> {
            seq: Seq,
            idem_key: &'a IdemKey,
            phase: Option<&'a str>,
            bead: &'a BeadId,
            op: &'static str,
            args: serde_json::Value,
            precondition: &'a Guards,
            actor: &'a Actor,
            state: JournalState,
            attempts: u32,
            last_error: Option<&'a str>,
            announced_at: Option<&'a str>,
            created_at: &'a str,
            applied_at: Option<&'a str>,
        }
        Wire {
            seq: self.seq,
            idem_key: &self.idem_key,
            phase: self.scope.as_ref().map(ScopeId::name),
            bead: &self.bead,
            op: self.op.name(),
            args: self.op.args_value(),
            precondition: &self.precondition,
            actor: &self.actor,
            state: self.state,
            attempts: self.attempts,
            last_error: self.last_error.as_deref(),
            announced_at: self.announced_at.as_deref(),
            created_at: &self.created_at,
            applied_at: self.applied_at.as_deref(),
        }
        .serialize(s)
    }
}

/// A write to journal: [`CacheStore::insert_entry`](crate::CacheStore::insert_entry)
/// stores it `Pending` with zero attempts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntry {
    /// Idempotency key.
    pub idem_key: IdemKey,
    /// The bead's scope at write time.
    pub scope: Option<ScopeId>,
    /// The bead written.
    pub bead: BeadId,
    /// The write.
    pub op: Op,
    /// The precondition.
    pub precondition: Guards,
    /// Who wrote it.
    pub actor: Actor,
}
