//! The write journal: [`Seq`], [`IdemKey`], [`JournalState`], [`JournalEntry`], [`NewEntry`].
//!
//! Also the text rules shared by the store and the 0.1 wire form of an entry:
//! the `CODE: detail` prefix of an error ([`error_code_prefix`]) and the
//! resolve note ([`resolve_note`], [`split_resolve_note`]).

use std::fmt;

use sc_beads::{Actor, BeadId, Guards, InvalidValue, Timestamp};
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

/// The caller's idempotency key (REQ-BDC-0004): nonempty, unique across the journal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct IdemKey(String);

impl IdemKey {
    /// Validates and wraps a key.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for an empty or all-whitespace key.
    pub fn new(key: impl Into<String>) -> Result<Self, InvalidValue> {
        let key = key.into();
        if key.trim().is_empty() {
            return Err(InvalidValue {
                kind: "idem_key",
                value: key,
                reason: "empty",
            });
        }
        Ok(Self(key))
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
/// Serializes as the 0.1 Python `entry` dict, whose key set is fixed (REQ-BDC-0910):
/// `op` as its name, `args` as the BD-CACHE args object, and `last_error`
/// with the resolve note appended as the v3 Python writer writes it
/// ([`JournalEntry::legacy_last_error`]). `last_error_code`, `note` and
/// `changed` are not serialized; `bd-cache-api` has its own data model.
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
    /// The latest error or escalation reason, as `CODE: detail` when it has a code.
    ///
    /// The resolve note is no longer appended here; it is [`JournalEntry::note`].
    pub last_error: Option<String>,
    /// The `CODE` of [`JournalEntry::last_error`] (`BD_CACHE.PRECONDITION`, ...),
    /// `None` when it has none ([`error_code_prefix`]).
    pub last_error_code: Option<String>,
    /// The resolve note, `resolved by <actor>: <note>` ([`resolve_note`]);
    /// `None` until the entry is resolved.
    pub note: Option<String>,
    /// Whether the write changed the view when it was journaled ([`Op::changes`]),
    /// as a duplicate returns it (lead ruling S62); `None` for an entry written
    /// without it (by the v3 Python writer, or before the column existed).
    pub changed: Option<bool>,
    /// When the escalation was announced to the lead.
    pub announced_at: Option<Timestamp>,
    /// When it was journaled.
    pub created_at: Timestamp,
    /// When it was marked applied, set by `apply_with_readback`.
    pub applied_at: Option<Timestamp>,
    /// When it was first offered to a post-sync hook (`journal.first_offered_at`,
    /// ADR-STORE-0006): set once, never changed after (REQ-BDC-0727, 0728).
    /// Not part of the 0.1 `entry` dict.
    pub first_offered_at: Option<Timestamp>,
}

impl JournalEntry {
    /// `last_error` as the 0.1 replies and the v3 Python writer show it: the error,
    /// then `" | "` and the resolve note when there is one.
    ///
    /// This is the text the v3 Python writer stores on a resolve
    /// (`COALESCE(last_error, '') || ' | resolved by <actor>: <note>'`), byte
    /// for byte.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::{IdemKey, JournalEntry, JournalState, Op, Seq};
    /// use sc_beads::{Actor, BeadId, Guards, Timestamp};
    ///
    /// let e = JournalEntry {
    ///     seq: Seq::new(1),
    ///     idem_key: IdemKey::new("k1").unwrap(),
    ///     scope: None,
    ///     bead: BeadId::new("b1").unwrap(),
    ///     op: Op::Close { reason: None },
    ///     precondition: Guards::default(),
    ///     actor: Actor::new("dev").unwrap(),
    ///     state: JournalState::Resolved,
    ///     attempts: 0,
    ///     last_error: Some("BD_CACHE.PRECONDITION: no".into()),
    ///     last_error_code: Some("BD_CACHE.PRECONDITION".into()),
    ///     note: Some("resolved by lead: set by hand".into()),
    ///     changed: Some(true),
    ///     announced_at: None,
    ///     created_at: Timestamp::parse("2026-10-23T10:00:00Z").unwrap(),
    ///     applied_at: None,
    ///     first_offered_at: None,
    /// };
    /// assert_eq!(
    ///     e.legacy_last_error().as_deref(),
    ///     Some("BD_CACHE.PRECONDITION: no | resolved by lead: set by hand")
    /// );
    /// ```
    #[must_use]
    pub fn legacy_last_error(&self) -> Option<String> {
        match &self.note {
            Some(note) => Some(format!(
                "{}{RESOLVE_SEPARATOR}{note}",
                self.last_error.as_deref().unwrap_or_default()
            )),
            None => self.last_error.clone(),
        }
    }
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
            last_error: Option<String>,
            announced_at: Option<&'a Timestamp>,
            created_at: &'a Timestamp,
            applied_at: Option<&'a Timestamp>,
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
            last_error: self.legacy_last_error(),
            announced_at: self.announced_at.as_ref(),
            created_at: &self.created_at,
            applied_at: self.applied_at.as_ref(),
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
    /// Whether the write changes the view ([`Op::changes`]), stored with the
    /// entry in the same transaction (REQ-BDC-0809); `None` stores no value.
    pub changed: Option<bool>,
}

/// What separates the error from the resolve note in a the v3 Python writer `last_error`.
pub const RESOLVE_SEPARATOR: &str = " | ";

/// What starts every resolve note.
const RESOLVED_BY: &str = "resolved by ";

/// The resolve note stored for `note` by `actor`: `resolved by <actor>: <note>`.
///
/// The text the v3 Python writer appends to `last_error` after [`RESOLVE_SEPARATOR`],
/// so the 0.1 rendering ([`JournalEntry::legacy_last_error`]) is unchanged and
/// the resolver is kept (the journal has no column for it).
///
/// # Examples
///
/// ```
/// let lead = sc_beads::Actor::new("lead").unwrap();
/// assert_eq!(bd_cache_model::resolve_note(&lead, "set by hand"), "resolved by lead: set by hand");
/// ```
#[must_use]
pub fn resolve_note(actor: &Actor, note: &str) -> String {
    format!("{RESOLVED_BY}{actor}: {note}")
}

/// Splits a the v3 Python writer resolved `last_error` into the error and the note.
///
/// the v3 Python writer appends `" | resolved by <actor>: <note>"` to `last_error`
/// when it resolves (its `COALESCE` makes the error `""` when there was none).
/// Returns `(error, note)`, the error `None` when it is empty, for the last
/// such suffix; `None` when `text` has no suffix of that form.
///
/// # Examples
///
/// ```
/// use bd_cache_model::split_resolve_note;
/// assert_eq!(
///     split_resolve_note("BD_CACHE.PRECONDITION: no | resolved by lead: by hand"),
///     Some((Some("BD_CACHE.PRECONDITION: no"), "resolved by lead: by hand"))
/// );
/// assert_eq!(split_resolve_note(" | resolved by lead: x"), Some((None, "resolved by lead: x")));
/// assert_eq!(split_resolve_note("BD_CACHE.PRECONDITION: no"), None);
/// ```
#[must_use]
pub fn split_resolve_note(text: &str) -> Option<(Option<&str>, &str)> {
    let marker = format!("{RESOLVE_SEPARATOR}{RESOLVED_BY}");
    let at = text.rfind(&marker)?;
    let (error, rest) = text.split_at(at);
    let note = rest.get(RESOLVE_SEPARATOR.len()..)?;
    // `resolved by <actor>: <note>`: an actor, then `: `.
    let actor = note.get(RESOLVED_BY.len()..)?.split_once(": ")?.0;
    if actor.is_empty() {
        return None;
    }
    Some(((!error.is_empty()).then_some(error), note))
}

/// The `CODE` of an error stored as `CODE: detail`, or `None`.
///
/// A code is one or more ASCII uppercase letters, digits, `_` or `.`
/// (`BD_CACHE.PRECONDITION`), as [`CacheError`](crate::CacheError)'s `Display`
/// and the v3 Python writer both write it. Text without such a prefix (`retries
/// exhausted after 3 attempts; ...`) has no code.
///
/// # Examples
///
/// ```
/// use bd_cache_model::error_code_prefix;
/// assert_eq!(error_code_prefix("BD_CACHE.PRECONDITION: no"), Some("BD_CACHE.PRECONDITION"));
/// assert_eq!(error_code_prefix("retries exhausted: x"), None);
/// assert_eq!(error_code_prefix(""), None);
/// ```
#[must_use]
pub fn error_code_prefix(text: &str) -> Option<&str> {
    let (code, _) = text.split_once(": ")?;
    let ok = !code.is_empty()
        && code
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, '_' | '.'));
    ok.then_some(code)
}
