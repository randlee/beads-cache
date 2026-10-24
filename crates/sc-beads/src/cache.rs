//! The cache extension of the backend contract: [`CacheBackend`] and its value types.
//!
//! btit's traits ([`BeadsBackend`] and friends) stay unchanged and keep
//! returning [`BeadsError`]. A cache needs more: batched id lists, scope lists
//! with a cursor, bd's own blocked and ready sets, guarded writes with an
//! actor, and a hard per-call [`Deadline`]. Those calls are on
//! [`CacheBackend`], a subtrait of [`BeadsBackend`] implemented by `sc-bd-cli`
//! (and later `sc-bd-http`) next to btit's traits, and they return
//! [`BeadsCacheError`], which wraps any [`BeadsError`] in
//! [`BeadsCacheError::Btit`] (ADR-BDC-0007, P-3 outcome).
//!
//! Ids are passed as [`BeadId`] here (btit passes `&str`). The patch shape is
//! aligned with bd serve's `/v0` vocabulary (ADR-BDC-0003).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::backend::BeadsBackend;
use crate::clock::Deadline;
use crate::error::BeadsError;
use crate::row::{Bead, DepType};
use crate::types::backend::ProjectRef;

macro_rules! string_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Wraps the given string.
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// The wrapped string.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Unwraps into the owned string.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

/// Why a string is not a valid [`BeadId`], [`Sha256Hex`] or other checked value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidValue {
    /// What was being parsed, e.g. `bead id`.
    pub kind: &'static str,
    /// The rejected text.
    pub value: String,
    /// Why it was rejected.
    pub reason: &'static str,
}

impl fmt::Display for InvalidValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {} {:?}: {}", self.kind, self.value, self.reason)
    }
}

impl std::error::Error for InvalidValue {}

/// A bead id, e.g. `pr-9b1`.
///
/// Nonempty, with no whitespace, control characters or commas (bd's
/// `--id a,b,...` separator), so every id can be passed to bd unquoted.
///
/// # Examples
///
/// ```
/// use sc_beads::BeadId;
/// assert_eq!(BeadId::new("pr-9b1").unwrap().as_str(), "pr-9b1");
/// assert!(BeadId::new("").is_err());
/// assert!(BeadId::new("a,b").is_err());
/// assert!(BeadId::new("a b").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BeadId(String);

impl BeadId {
    /// Validates and wraps a bead id.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for an empty id or one with whitespace, control
    /// characters or a comma.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidValue> {
        let value = value.into();
        let reason = if value.is_empty() {
            Some("empty")
        } else if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
            Some("contains whitespace or a control character")
        } else if value.contains(',') {
            Some("contains a comma")
        } else {
            None
        };
        match reason {
            Some(reason) => Err(InvalidValue {
                kind: "bead id",
                value,
                reason,
            }),
            None => Ok(Self(value)),
        }
    }

    /// The id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Unwraps into the owned string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for BeadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for BeadId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BeadId {
    type Error = InvalidValue;
    fn try_from(value: String) -> Result<Self, InvalidValue> {
        Self::new(value)
    }
}

impl TryFrom<&str> for BeadId {
    type Error = InvalidValue;
    fn try_from(value: &str) -> Result<Self, InvalidValue> {
        Self::new(value)
    }
}

impl std::str::FromStr for BeadId {
    type Err = InvalidValue;
    fn from_str(s: &str) -> Result<Self, InvalidValue> {
        Self::new(s)
    }
}

impl From<BeadId> for String {
    fn from(id: BeadId) -> String {
        id.0
    }
}

/// A lower-case hex sha256 digest (64 characters).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Sha256Hex(String);

impl Sha256Hex {
    /// Validates a digest.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] unless `value` is 64 lower-case hex digits.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidValue> {
        let value = value.into();
        if value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(value))
        } else {
            Err(InvalidValue {
                kind: "sha256",
                value,
                reason: "expected 64 lower-case hex digits",
            })
        }
    }

    /// The digest text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Sha256Hex {
    type Error = InvalidValue;
    fn try_from(value: String) -> Result<Self, InvalidValue> {
        Self::new(value)
    }
}

impl From<Sha256Hex> for String {
    fn from(h: Sha256Hex) -> String {
        h.0
    }
}

/// An assignee value in a guard or patch: unassigned, or a name.
///
/// On the wire (BD-CACHE preconditions, `bd --assignee`) unassigned is `""`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Assignee {
    /// No assignee (`""`).
    Unassigned,
    /// This assignee.
    Named(String),
}

impl Assignee {
    /// Reads the wire form: `""` is unassigned.
    #[must_use]
    pub fn from_wire(s: &str) -> Self {
        if s.is_empty() {
            Self::Unassigned
        } else {
            Self::Named(s.to_owned())
        }
    }

    /// The wire form: `""` for unassigned.
    #[must_use]
    pub fn as_wire(&self) -> &str {
        match self {
            Self::Unassigned => "",
            Self::Named(n) => n,
        }
    }

    /// True when a bead's assignee (`None` or `""` = unassigned) equals this one.
    #[must_use]
    pub fn matches(&self, current: Option<&str>) -> bool {
        current.unwrap_or("") == self.as_wire()
    }
}

impl fmt::Display for Assignee {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire())
    }
}

impl Serialize for Assignee {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_wire())
    }
}

impl<'de> Deserialize<'de> for Assignee {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(|s| Self::from_wire(&s))
    }
}

string_newtype!(
    /// The actor a write is made as (`bd ... --actor <actor>`).
    Actor
);
string_newtype!(
    /// An opaque sync cursor handed back to [`CacheBackend::list_scope`].
    ///
    /// The cache mints it as an RFC 3339 instant derived from the scope's
    /// `last_ok_at` (ADR-BDC-0004); a backend treats it as opaque, so a commit
    /// hash or events sequence can replace it later. `sc-bd-cli` passes it as
    /// `--updated-after`.
    SyncCursor
);

/// One outgoing dependency edge of a bead (`bd list --json` `dependencies`).
///
/// Serializes as `{"issue_id", "depends_on_id", "type"}`. Deserializing
/// tolerates the other keys bd emits on a dependency (`created_at`,
/// `created_by`, `metadata`, ...) so stored rows written by the Python cache
/// read back; strict validation of bd output happens in
/// [`parse_bd_row`](crate::parse_bd_row).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Edge {
    /// The bead the edge leaves from.
    pub issue_id: BeadId,
    /// The bead it depends on.
    pub depends_on_id: BeadId,
    /// The dependency type.
    #[serde(rename = "type")]
    pub kind: DepType,
}

impl Edge {
    /// Creates an edge `issue_id -> depends_on_id` of the given kind.
    #[must_use]
    pub fn new(issue_id: BeadId, depends_on_id: BeadId, kind: DepType) -> Self {
        Self {
            issue_id,
            depends_on_id,
            kind,
        }
    }
}

/// One row of `bd blocked --json`: a blocked bead and the ids blocking it.
///
/// A child of a blocked parent lists the **parent** as its blocker
/// (`docs/research/bd-blocked-state-probe.md`). Parse with
/// [`parse_blocked_rows`](crate::parse_blocked_rows), which validates the full
/// row strictly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedRow {
    /// The blocked bead.
    pub id: BeadId,
    /// The beads blocking it, as bd lists them.
    pub blocked_by: Vec<BeadId>,
}

/// What a backend reports about itself: the inputs of the rebuild rule (REQ-BDC-0105).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// `bd version` (e.g. `1.3.1`).
    pub bd_version: String,
    /// Lower-case hex sha256 of the full `bd schema` output.
    pub schema_sha256: Sha256Hex,
    /// `schema_version` from `bd schema`, as text (e.g. `1`).
    pub schema_version: String,
}

/// Which beads a scope list selects.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ScopeFilter {
    /// Beads carrying this label (`bd list -l <label>`).
    Label(String),
}

impl ScopeFilter {
    /// The label of a [`ScopeFilter::Label`].
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Label(l) => l,
        }
    }
}

/// Write guards: the write applies only while the bead still has these values.
///
/// [`Assignee::Unassigned`] (`""` on the wire) means unassigned (REQ-BDC-0302). `sc-bd-cli`
/// passes them as `--if-status` / `--if-assignee`; a refused guard is
/// [`Refusal::Precondition`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Guards {
    /// Required current status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Required current assignee.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<Assignee>,
}

impl Guards {
    /// True when no guard is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.status.is_none() && self.assignee.is_none()
    }
}

/// A metadata value that survives `bd --set-metadata` unchanged (REQ-BDC-0303).
///
/// Whether a string qualifies is checked when a write is parsed
/// (`bd-cache-model`'s `Op::parse`); this type only carries the value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetaValue {
    /// An integer.
    Int(i64),
    /// A boolean, passed to bd as `true` / `false`.
    Bool(bool),
    /// A string that bd stores as a string.
    Str(String),
}

impl MetaValue {
    /// The value as JSON (`5`, `true`, `"abc"`).
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Int(i) => serde_json::Value::from(*i),
            Self::Bool(b) => serde_json::Value::Bool(*b),
            Self::Str(s) => serde_json::Value::String(s.clone()),
        }
    }

    /// The text of `--set-metadata key=<text>`: ints in decimal, bools as `true`/`false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_beads::MetaValue;
    /// assert_eq!(MetaValue::Bool(true).to_bd_arg(), "true");
    /// assert_eq!(MetaValue::Int(-3).to_bd_arg(), "-3");
    /// assert_eq!(MetaValue::Str("abc".into()).to_bd_arg(), "abc");
    /// ```
    #[must_use]
    pub fn to_bd_arg(&self) -> String {
        match self {
            Self::Int(i) => i.to_string(),
            Self::Bool(b) => b.to_string(),
            Self::Str(s) => s.clone(),
        }
    }

    /// True when this value equals a JSON value read back from bd.
    #[must_use]
    pub fn matches(&self, value: &serde_json::Value) -> bool {
        match (self, value) {
            (Self::Int(i), v) => v.as_i64() == Some(*i),
            (Self::Bool(b), serde_json::Value::Bool(v)) => b == v,
            (Self::Str(s), serde_json::Value::String(v)) => s == v,
            _ => false,
        }
    }
}

/// An update patch, aligned with bd serve's `/v0` vocabulary (ADR-BDC-0003).
///
/// Fields apply in declaration order (REQ-BDC-0301): status, assignee, add
/// labels, remove labels, set metadata, unset metadata, append notes. Empty
/// collections mean "not requested".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Patch {
    /// New status.
    pub status: Option<String>,
    /// New assignee; [`Assignee::Unassigned`] unassigns.
    pub assignee: Option<Assignee>,
    /// Labels to add.
    pub add_labels: Vec<String>,
    /// Labels to remove.
    pub remove_labels: Vec<String>,
    /// Metadata keys to set.
    pub set_metadata: BTreeMap<String, MetaValue>,
    /// Metadata keys to remove.
    pub unset_metadata: Vec<String>,
    /// Text appended to the notes (joined with a newline).
    pub append_notes: Option<String>,
}

impl Patch {
    /// True when the patch requests nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.status.is_none()
            && self.assignee.is_none()
            && self.add_labels.is_empty()
            && self.remove_labels.is_empty()
            && self.set_metadata.is_empty()
            && self.unset_metadata.is_empty()
            && self.append_notes.is_none()
    }
}

/// Why a reachable backend refused a guarded write.
///
/// The CLI only produces [`Refusal::Precondition`] (bd exit 13);
/// [`Refusal::NotClosable`] and [`Refusal::AlreadyClaimed`] come from the HTTP
/// implementation's 409 codes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// A guard no longer holds (`bd` exit 13, `/v0` `precondition_failed`).
    Precondition,
    /// The bead cannot be closed (`/v0` `not_closable`).
    NotClosable,
    /// The bead is already claimed (`/v0` `already_claimed`).
    AlreadyClaimed,
    /// Any other refusal, with the backend's code.
    Other(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Precondition => f.write_str("precondition_failed"),
            Self::NotClosable => f.write_str("not_closable"),
            Self::AlreadyClaimed => f.write_str("already_claimed"),
            Self::Other(code) => f.write_str(code),
        }
    }
}

/// The steps of the backend check ([`CacheBackend::fingerprint`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckStep {
    /// `bd version`.
    Version,
    /// `bd schema`.
    Schema,
}

impl CheckStep {
    /// The bd subcommand: `version` or `schema`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Schema => "schema",
        }
    }
}

impl fmt::Display for CheckStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned by every [`CacheBackend`] call.
///
/// btit's [`BeadsError`] is unchanged; the cache's cases live here, and any
/// btit error converts into [`BeadsCacheError::Btit`] (ADR-BDC-0007).
#[derive(Debug)]
#[non_exhaustive]
pub enum BeadsCacheError {
    /// The backend could not answer: timeout, cannot start, connection refused.
    Unavailable {
        /// What happened, for logs and the outage record.
        reason: String,
        /// True when the call was stopped at its deadline.
        timed_out: bool,
    },
    /// A reachable backend refused a guarded write.
    Refused(Refusal),
    /// The backend's output broke the strict row contract (REQ-BDC-0007).
    Contract {
        /// The bead whose row failed, when known.
        bead: Option<BeadId>,
        /// The failing field path, e.g. `status` or `dependencies[0].type`.
        field: String,
        /// What was wrong.
        detail: String,
    },
    /// The backend ran the command and it failed (nonzero exit other than a refusal).
    Failed {
        /// Exit code.
        code: i32,
        /// Captured standard error.
        stderr: String,
    },
    /// A step of [`CacheBackend::fingerprint`] failed: which one, so the cache
    /// can tell `BD_VERSION_FAILED` from `BD_SCHEMA_FAILED`.
    CheckFailed {
        /// The failing step.
        step: CheckStep,
        /// Exit code.
        code: i32,
        /// Captured standard error.
        stderr: String,
    },
    /// Any btit error from shared transport code.
    Btit(BeadsError),
}

impl BeadsCacheError {
    /// Stable machine-readable code for this variant.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unavailable { .. } => "SC_BEADS_UNAVAILABLE",
            Self::Refused(_) => "SC_BEADS_REFUSED",
            Self::Contract { .. } => "SC_BEADS_CONTRACT",
            Self::Failed { .. } => "SC_BEADS_FAILED",
            Self::CheckFailed { .. } => "SC_BEADS_CHECK_FAILED",
            Self::Btit(e) => e.code(),
        }
    }

    /// A contract violation naming bead and field.
    #[must_use]
    pub fn contract(
        bead: Option<&BeadId>,
        field: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::Contract {
            bead: bead.cloned(),
            field: field.into(),
            detail: detail.into(),
        }
    }

    /// True for [`BeadsCacheError::Unavailable`] and for a btit spawn failure (bd cannot start).
    #[must_use]
    pub fn is_unavailable(&self) -> bool {
        matches!(
            self,
            Self::Unavailable { .. } | Self::Btit(BeadsError::Spawn { .. })
        )
    }

    /// True for an [`BeadsCacheError::Unavailable`] that hit its deadline.
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(
            self,
            Self::Unavailable {
                timed_out: true,
                ..
            }
        )
    }
}

impl fmt::Display for BeadsCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable {
                reason,
                timed_out: true,
            } => write!(f, "backend timed out: {reason}"),
            Self::Unavailable { reason, .. } => write!(f, "backend unavailable: {reason}"),
            Self::Refused(r) => write!(f, "backend refused the write: {r}"),
            Self::Contract {
                bead: Some(bead),
                field,
                detail,
            } => write!(f, "{bead}: {field}: {detail}"),
            Self::Contract {
                bead: None,
                field,
                detail,
            } if field.is_empty() => f.write_str(detail),
            Self::Contract { field, detail, .. } => write!(f, "{field}: {detail}"),
            Self::Failed { code, stderr } => write!(f, "exit {code}: {}", stderr.trim()),
            Self::CheckFailed { step, code, stderr } => {
                write!(f, "bd {step}: exit {code}: {}", stderr.trim())
            }
            Self::Btit(e) => fmt::Display::fmt(e, f),
        }
    }
}

impl std::error::Error for BeadsCacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Btit(e) => Some(e),
            _ => None,
        }
    }
}

impl From<BeadsError> for BeadsCacheError {
    fn from(e: BeadsError) -> Self {
        Self::Btit(e)
    }
}

/// The cache's backend surface; implemented by `sc-bd-cli` (a-2) and `FakeBackend`.
///
/// Implementations are constructed with the same `Arc<dyn Clock>` as the cache
/// and stop each call at the [`Deadline`] they are handed, returning
/// [`BeadsCacheError::Unavailable`] with `timed_out: true`. They apply no
/// per-call cap of their own: the cache owns the 30 s cap
/// (`CacheConfig::call_timeout`). Every row they return is strictly validated
/// ([`parse_bd_row`](crate::parse_bd_row)); a bad row fails the whole call
/// with [`BeadsCacheError::Contract`].
///
/// Every implementation must pass the contract suite in
/// `sc_beads::testing::contract` (feature `testing`).
///
/// # Errors
///
/// Every method returns a [`BeadsCacheError`]: `Unavailable` when the backend
/// could not answer (including a reached deadline), `Refused` for a refused
/// guard, `Contract` for output that breaks the row contract, `Failed` for
/// any other failed command, `Btit` for errors of the shared transport.
#[expect(
    clippy::missing_errors_doc,
    reason = "failure modes are documented once on the trait and on BeadsCacheError"
)]
pub trait CacheBackend: BeadsBackend + Send + Sync {
    /// `bd version` plus the sha256 of `bd schema` (never `bd context`, REQ-BDC-0106).
    ///
    /// A failing step is [`BeadsCacheError::CheckFailed`] naming it.
    fn fingerprint(&self, p: &ProjectRef, d: Deadline) -> Result<Fingerprint, BeadsCacheError>;

    /// Fresh rows for the given ids, all statuses (closed included), in one call.
    ///
    /// Ids bd does not know are absent from the result; order is unspecified.
    fn list_by_ids(
        &self,
        p: &ProjectRef,
        ids: &[BeadId],
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError>;

    /// Every bead matching the filter, all statuses; with a cursor, only beads updated after it.
    fn list_scope(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        since: Option<&SyncCursor>,
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError>;

    /// bd's blocked set for the filter (`bd blocked -l <label> --json`).
    ///
    /// Includes blockers outside the filter and parent-child inheritance (a
    /// child of a blocked parent lists the parent).
    fn list_blocked(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BlockedRow>, BeadsCacheError>;

    /// bd's ready set for the filter (`bd ready -l <label> -n 0 --json`), as ids.
    fn list_ready(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BeadId>, BeadsCacheError>;

    /// Closes the bead with an optional reason, under the guards, as the actor.
    fn close_guarded(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        reason: Option<&str>,
        g: &Guards,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError>;

    /// Applies the patch under the guards, as the actor.
    fn update_guarded(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        patch: &Patch,
        g: &Guards,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError>;

    /// Adds the edge `id -> on` of the given kind, as the actor.
    fn dep_add_typed(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        on: &BeadId,
        kind: DepType,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn btit_errors_convert_and_keep_their_code() {
        let e: BeadsCacheError = BeadsError::SchemaMigration {
            binary: "bd".into(),
        }
        .into();
        assert_eq!(e.code(), "BTIT_BEADS_SCHEMA_MIGRATION");
        assert!(e.to_string().starts_with("SCHEMA_MIGRATION_ERROR"));
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn unavailable_classification() {
        let timeout = BeadsCacheError::Unavailable {
            reason: "slow".into(),
            timed_out: true,
        };
        assert!(timeout.is_unavailable() && timeout.is_timeout());
        let spawn: BeadsCacheError = BeadsError::Spawn {
            binary: "bd".into(),
            operation: None,
            source: std::io::Error::other("missing"),
        }
        .into();
        assert!(spawn.is_unavailable() && !spawn.is_timeout());
        assert!(!BeadsCacheError::Refused(Refusal::Precondition).is_unavailable());
    }

    #[test]
    fn meta_value_matches_bd_json() {
        assert!(MetaValue::Int(5).matches(&serde_json::json!(5)));
        assert!(!MetaValue::Int(5).matches(&serde_json::json!("5")));
        assert!(MetaValue::Bool(false).matches(&serde_json::json!(false)));
        assert!(MetaValue::Str("x".into()).matches(&serde_json::json!("x")));
        assert_eq!(MetaValue::Str("x".into()).to_json(), serde_json::json!("x"));
    }

    #[test]
    fn edge_reads_python_stored_dependency_objects() {
        let e: Edge = serde_json::from_value(serde_json::json!({
            "issue_id": "ct-2", "depends_on_id": "ct-1", "type": "blocks",
            "created_at": "2026-10-23T19:57:24Z", "metadata": "{}"
        }))
        .unwrap();
        let id = |s: &str| BeadId::new(s).unwrap();
        assert_eq!(e, Edge::new(id("ct-2"), id("ct-1"), DepType::Blocks));
        assert_eq!(
            serde_json::to_value(&e).ok(),
            Some(
                serde_json::json!({"issue_id": "ct-2", "depends_on_id": "ct-1", "type": "blocks"})
            )
        );
    }

    #[test]
    fn sha256_hex_and_assignee_are_checked() {
        assert!(Sha256Hex::new("a".repeat(64)).is_ok());
        for bad in ["", "A".repeat(64).as_str(), "g".repeat(64).as_str(), "abc"] {
            assert!(Sha256Hex::new(bad).is_err(), "{bad}");
        }
        assert_eq!(Assignee::from_wire(""), Assignee::Unassigned);
        assert_eq!(Assignee::from_wire("dev").as_wire(), "dev");
        assert!(Assignee::Unassigned.matches(None) && Assignee::Unassigned.matches(Some("")));
        assert!(!Assignee::Unassigned.matches(Some("dev")));
        let g = Guards {
            status: None,
            assignee: Some(Assignee::Unassigned),
        };
        assert_eq!(
            serde_json::to_value(&g).unwrap(),
            serde_json::json!({"assignee": ""})
        );
        assert_eq!(
            serde_json::from_value::<Guards>(serde_json::json!({"assignee": ""})).unwrap(),
            g
        );
        assert!(serde_json::from_value::<BeadId>(serde_json::json!("a,b")).is_err());
    }
}
