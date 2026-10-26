//! [`CacheError`]: every `BD_CACHE.*` code with its payload, fix text and JSON shape.
//!
//! One variant per baseline code (REQ-BDC-0803), plus the additive
//! `SCOPE_UNKNOWN`, `STORE_FAILED` (a-1 amendment 1) and `STORE_IN_USE`
//! (PE-RBP-F008), and phase B's `CONFIG_INVALID` (a file scope changed through
//! the registry, ADR-BDC-0013), `INGEST_INVALID`, `SCHEDULE_INVALID`,
//! `TICK_RUN_FAILED` and `TICK_IN_FLIGHT` (REQ-BDC-0729). `NO_TOML` is
//! Python-only and never emitted. Every typed
//! error exits 3 (REQ-BDC-0802) and renders as
//! `{ok: false, code, detail, fix, reason?, ...payload, source?}`
//! ([`CacheError::to_json`]).
//!
//! An error mapped from a backend failure keeps that failure as its
//! [`std::error::Error::source`] (a [`BackendCause`]); `to_json` renders it as
//! a nested `source: {code, detail}` object. A store's own failure
//! (`STORE_FAILED`), and a `NO_DATABASE` from a `metadata.json` that cannot be
//! read or parsed, keep the underlying error as a [`StoreCause`], rendered as
//! `source: {detail}`.
//!
//! [`CacheError::from_backend`] maps a backend error to a code by the kind of
//! call that failed:
//!
//! | Backend error | Code |
//! |---|---|
//! | `Contract`, btit's `InvalidJson` / `UnexpectedShape` / `ParseFailed` | `BD_CONTRACT` |
//! | `Transport` (output unreadable or over the cap) | by [`CallKind`]: `Write` → `BD_WRITE_FAILED`; `Sync` → `SYNC_FAILED` (cause `BD_LIST_FAILED`); every other kind → `BD_LIST_FAILED` |
//! | `NotFound` | `NOT_FOUND` |
//! | `Refused(Precondition / NotClosable / AlreadyClaimed)` | `PRECONDITION` with `reason` |
//! | `Unavailable`, btit's `Spawn` | `BD_UNAVAILABLE` |
//! | `CheckFailed { step }` | `BD_VERSION_FAILED` / `BD_SCHEMA_FAILED` by step |
//! | `Failed`, `Refused(Other)`, any other btit error | by [`CallKind`]: `Write` → `BD_WRITE_FAILED`; `PreRead`, `ReadBack`, `Refresh`, `MissFill`, `Ready` → `BD_LIST_FAILED`; `Sync` → `SYNC_FAILED` (cause `BD_LIST_FAILED`); `Check` → `BD_VERSION_FAILED` |

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use sc_beads::{BeadId, BeadsCacheError, BeadsError, CheckStep, Refusal};
use serde::{Serialize, Serializer};
use serde_json::{Value, json};

use crate::call::CallKind;
use crate::journal::Seq;
use crate::scope::ScopeId;
use crate::state::LeaseHolder;

/// Exit code of every typed error (REQ-BDC-0802).
pub const TYPED_ERROR_EXIT: i32 = 3;

/// The codes that mean `bd` could not answer at all: a backend outage, never a
/// write attempt (the set the v3 Python writer calls `UNREACHABLE`).
///
/// [`CacheError::is_unreachable`] is the same set by variant; a test keeps the two equal.
pub const UNREACHABLE_CODES: [&str; 4] = [
    "BD_CACHE.BD_UNAVAILABLE",
    "BD_CACHE.BD_VERSION_FAILED",
    "BD_CACHE.BD_SCHEMA_FAILED",
    "BD_CACHE.BD_LIST_FAILED",
];

/// The keys of [`CacheError::to_json`] that every error object has, or has on its
/// own account (`reason`, for `PRECONDITION`); every other key is the code's payload
/// (`CacheError::payload_keys`, which the tests use to keep this list and `to_json` equal).
pub const ENVELOPE_KEYS: [&str; 5] = ["ok", "code", "detail", "fix", "reason"];

/// A precondition key (REQ-BDC-0302).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GuardKey {
    /// `status`.
    Status,
    /// `assignee`.
    Assignee,
}

impl GuardKey {
    /// The wire name: `status` or `assignee`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Assignee => "assignee",
        }
    }
}

impl fmt::Display for GuardKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for GuardKey {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// One unmet precondition key: what the write assumed and what the bead has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unmet {
    /// The value the write's precondition expected.
    pub expected: String,
    /// The bead's current value (`""` when unset).
    pub actual: String,
}

/// `/v0` refusal reason carried by a `PRECONDITION` error (ADR-BDC-0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreconditionReason {
    /// `precondition_failed`: a guard no longer holds.
    PreconditionFailed,
    /// `not_closable`: the bead cannot be closed.
    NotClosable,
    /// `already_claimed`: the bead is claimed by someone else.
    AlreadyClaimed,
}

impl PreconditionReason {
    /// The `/v0` name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreconditionFailed => "precondition_failed",
            Self::NotClosable => "not_closable",
            Self::AlreadyClaimed => "already_claimed",
        }
    }
}

/// The underlying code of a `SYNC_FAILED` (`cause` in the JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncCause {
    /// `BD_CACHE.BD_UNAVAILABLE`.
    BdUnavailable,
    /// `BD_CACHE.BD_VERSION_FAILED`.
    BdVersionFailed,
    /// `BD_CACHE.BD_SCHEMA_FAILED`.
    BdSchemaFailed,
    /// `BD_CACHE.BD_CONTRACT`.
    BdContract,
    /// `BD_CACHE.BD_LIST_FAILED`.
    BdListFailed,
}

impl SyncCause {
    /// The underlying code, e.g. `BD_CACHE.BD_CONTRACT`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BdUnavailable => "BD_CACHE.BD_UNAVAILABLE",
            Self::BdVersionFailed => "BD_CACHE.BD_VERSION_FAILED",
            Self::BdSchemaFailed => "BD_CACHE.BD_SCHEMA_FAILED",
            Self::BdContract => "BD_CACHE.BD_CONTRACT",
            Self::BdListFailed => "BD_CACHE.BD_LIST_FAILED",
        }
    }

    /// The cause a typed error stands for, when it is one of the sync causes.
    #[must_use]
    pub fn of(e: &CacheError) -> Option<Self> {
        match e {
            CacheError::BdUnavailable { .. } => Some(Self::BdUnavailable),
            CacheError::BdVersionFailed { .. } => Some(Self::BdVersionFailed),
            CacheError::BdSchemaFailed { .. } => Some(Self::BdSchemaFailed),
            CacheError::BdContract { .. } => Some(Self::BdContract),
            CacheError::BdListFailed { .. } => Some(Self::BdListFailed),
            _ => None,
        }
    }
}

impl Serialize for SyncCause {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// Why an ingest was refused (`INGEST_INVALID`'s `details.reason`, REQ-BDC-0729).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IngestReason {
    /// `row_invalid`: a row or blocked row the strict decoder refuses, or input that is not a JSON array.
    RowInvalid,
    /// `empty`: a snapshot with no row, without `allow_empty`.
    Empty,
    /// `too_many_rows`: more than 50 000 rows.
    TooManyRows,
    /// `too_large`: more than 64 MiB of input.
    TooLarge,
    /// `membership`: a bead listed twice.
    Membership,
}

impl IngestReason {
    /// The wire name, e.g. `row_invalid`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RowInvalid => "row_invalid",
            Self::Empty => "empty",
            Self::TooManyRows => "too_many_rows",
            Self::TooLarge => "too_large",
            Self::Membership => "membership",
        }
    }
}

impl Serialize for IngestReason {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// Why a sync row's run failed (`TICK_RUN_FAILED`'s `details.reason`, REQ-BDC-0729).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickRunReason {
    /// `exit`: a non-zero exit status.
    Exit,
    /// `timeout`: the run's deadline passed.
    Timeout,
    /// `spawn`: the process could not start.
    Spawn,
    /// `oversize`: its output went over the cap.
    Oversize,
    /// `stop_answer`: a stop answer other than `continue`.
    StopAnswer,
    /// `untrusted`: the cache file failed the trust check.
    Untrusted,
    /// `signal`: the command was stopped by a signal.
    Signal,
}

impl TickRunReason {
    /// The wire name, e.g. `stop_answer`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exit => "exit",
            Self::Timeout => "timeout",
            Self::Spawn => "spawn",
            Self::Oversize => "oversize",
            Self::StopAnswer => "stop_answer",
            Self::Untrusted => "untrusted",
            Self::Signal => "signal",
        }
    }
}

impl Serialize for TickRunReason {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// The backend error a [`CacheError`] was mapped from (its `source`).
///
/// Compares equal by code and text, so errors stay comparable in tests.
#[derive(Debug, Clone)]
pub struct BackendCause(Arc<BeadsCacheError>);

impl BackendCause {
    /// Wraps a backend error.
    #[must_use]
    pub fn new(e: BeadsCacheError) -> Self {
        Self(Arc::new(e))
    }

    /// The backend error.
    #[must_use]
    pub fn error(&self) -> &BeadsCacheError {
        &self.0
    }
}

impl PartialEq for BackendCause {
    fn eq(&self, other: &Self) -> bool {
        self.0.code() == other.0.code() && self.0.to_string() == other.0.to_string()
    }
}

impl Eq for BackendCause {}

/// The local failure a `STORE_FAILED` or `NO_DATABASE` was built from (its
/// `source`): a SQLite error, an I/O error, a stored value or a
/// `metadata.json` that does not decode.
///
/// Compares equal by text, so errors stay comparable in tests.
#[derive(Debug, Clone)]
pub struct StoreCause(Arc<dyn std::error::Error + Send + Sync + 'static>);

impl StoreCause {
    /// Wraps the underlying error.
    #[must_use]
    pub fn new(e: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self(Arc::new(e))
    }

    /// The underlying error.
    #[must_use]
    pub fn error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        &*self.0
    }
}

impl PartialEq for StoreCause {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string() == other.0.to_string()
    }
}

impl Eq for StoreCause {}

/// A typed bd-cache error (`BD_CACHE.*`).
///
/// Variants a backend failure can produce carry `source`, the
/// [`BackendCause`] they were mapped from (`None` when built directly).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CacheError {
    /// `BD_CACHE.BD_UNAVAILABLE`: bd timed out or could not start.
    BdUnavailable {
        /// What happened.
        detail: String,
        /// The backend error, when mapped from one.
        source: Option<BackendCause>,
    },
    /// `BD_CACHE.NO_DATABASE`: no readable `dolt_database` / `project_id` in metadata.json.
    NoDatabase {
        /// What happened, with the cause's text.
        detail: String,
        /// The I/O or parse error reading `metadata.json`, when there is one
        /// (PE-RBP-F014). A permission-denied I/O error changes the fix.
        source: Option<StoreCause>,
    },
    /// `BD_CACHE.WRONG_DATABASE`: the file's recorded project id differs.
    WrongDatabase {
        /// What happened.
        detail: String,
    },
    /// `BD_CACHE.NOT_FOUND`: bead (or journal entry) in neither cache nor bd.
    NotFound {
        /// What happened.
        detail: String,
    },
    /// `BD_CACHE.BD_VERSION_FAILED`: `bd version` failed.
    BdVersionFailed {
        /// What happened.
        detail: String,
        /// The backend error, when mapped from one.
        source: Option<BackendCause>,
    },
    /// `BD_CACHE.BD_SCHEMA_FAILED`: `bd schema` failed.
    BdSchemaFailed {
        /// What happened.
        detail: String,
        /// The backend error, when mapped from one.
        source: Option<BackendCause>,
    },
    /// `BD_CACHE.BD_CONTRACT`: a row fails the strict model, or bd printed invalid JSON.
    BdContract {
        /// The bead whose row failed, when known.
        bead: Option<BeadId>,
        /// The failing field path (`""` when the whole output was bad).
        field: String,
        /// What was wrong.
        detail: String,
        /// The backend error, when mapped from one.
        source: Option<BackendCause>,
    },
    /// `BD_CACHE.BD_WRITE_FAILED`: a write exited nonzero (not 13).
    BdWriteFailed {
        /// What happened.
        detail: String,
        /// The backend error, when mapped from one.
        source: Option<BackendCause>,
    },
    /// `BD_CACHE.BD_LIST_FAILED`: a read failed.
    BdListFailed {
        /// What happened.
        detail: String,
        /// The backend error, when mapped from one.
        source: Option<BackendCause>,
    },
    /// `BD_CACHE.SYNC_FAILED`: a sync failed.
    SyncFailed {
        /// What happened.
        detail: String,
        /// The scope that failed, when known (`phase` in the JSON).
        scope: Option<ScopeId>,
        /// The underlying code.
        cause: Option<SyncCause>,
        /// The backend error, when mapped from one.
        source: Option<BackendCause>,
    },
    /// `BD_CACHE.PRECONDITION`: a write's precondition is unmet, or bd refused a guard.
    Precondition {
        /// What happened.
        detail: String,
        /// Unmet keys with expected and actual values (empty when bd refused).
        unmet: BTreeMap<GuardKey, Unmet>,
        /// The `/v0` reason.
        reason: PreconditionReason,
        /// The backend refusal, when bd refused.
        source: Option<BackendCause>,
    },
    /// `BD_CACHE.IDEM_CONFLICT`: the idempotency key already journals another write.
    IdemConflict {
        /// What happened.
        detail: String,
        /// The entry that holds the key.
        seq: Seq,
    },
    /// `BD_CACHE.BAD_OP`: unknown op.
    BadOp {
        /// What happened.
        detail: String,
    },
    /// `BD_CACHE.BAD_ARGS`: invalid op args.
    BadArgs {
        /// What happened.
        detail: String,
    },
    /// `BD_CACHE.BAD_PRECONDITION`: invalid precondition.
    BadPrecondition {
        /// What happened.
        detail: String,
    },
    /// `BD_CACHE.SCHEMA_VERSION`: the cache file is newer than this code.
    SchemaVersion {
        /// What happened.
        detail: String,
    },
    /// `BD_CACHE.PRUNE_BLOCKED`: the scope has unapplied writes.
    PruneBlocked {
        /// What happened.
        detail: String,
    },
    /// `BD_CACHE.NOT_ESCALATED`: `resolve` on an entry that is not escalated.
    NotEscalated {
        /// What happened.
        detail: String,
    },
    /// `BD_CACHE.SCOPE_UNKNOWN` (additive): a `list` or `ready` outside every known scope.
    ScopeUnknown {
        /// What happened.
        detail: String,
        /// The scopes the cache knows.
        known: Vec<ScopeId>,
    },
    /// `BD_CACHE.STORE_FAILED` (additive, a-1 amendment 1): the store itself
    /// failed: SQLite, file I/O, or a stored value that does not decode.
    StoreFailed {
        /// The failing step and the file, e.g. `open /repo/.bd-cache/x.sqlite3: ...`.
        detail: String,
        /// The underlying error, when there is one.
        source: Option<StoreCause>,
    },
    /// `BD_CACHE.STORE_IN_USE` (additive, PE-RBP-F008): this process already
    /// has the cache file open through another handle (one connection per file
    /// per process, NFR-BDC-0007). Nothing is wrong with the file.
    StoreInUse {
        /// The file, e.g. `cache store /repo/.bd-cache/x.sqlite3: open: ...`.
        detail: String,
    },
    /// `BD_CACHE.CONFIG_INVALID` (ADR-BDC-0013): a scope `.beads/bd-cache.toml`
    /// defines was changed or removed through the registry; the file wins.
    ConfigInvalid {
        /// What happened.
        detail: String,
        /// The check that refused, `details.source` (`config_scope`).
        origin: &'static str,
    },
    /// `BD_CACHE.INGEST_INVALID` (REQ-BDC-0726, 0729): an ingest was refused
    /// and the stored scope is untouched.
    IngestInvalid {
        /// What happened.
        detail: String,
        /// The scope ingested.
        scope: ScopeId,
        /// Why.
        reason: IngestReason,
        /// The index of the row at fault, when one row is.
        index: Option<usize>,
    },
    /// `BD_CACHE.SCHEDULE_INVALID` (REQ-BDC-0729): a schedule or switch
    /// option is refused.
    ScheduleInvalid {
        /// What happened.
        detail: String,
        /// The option at fault.
        field: String,
        /// Why, e.g. `python_sync_enabled`.
        reason: String,
    },
    /// `BD_CACHE.TICK_RUN_FAILED` (REQ-BDC-0729): a sync row's run failed.
    TickRunFailed {
        /// What happened.
        detail: String,
        /// The row that ran.
        row: String,
        /// The run.
        run_id: String,
        /// Why.
        reason: TickRunReason,
        /// The exit status, when the process exited.
        exit_status: Option<i32>,
        /// Its stderr, truncated.
        stderr: String,
        /// Whether the previous state was restored (the command form only).
        restored: Option<bool>,
    },
    /// `BD_CACHE.TICK_IN_FLIGHT` (REQ-BDC-0722, 0729): another run holds the lease.
    TickInFlight {
        /// What happened.
        detail: String,
        /// Who holds it.
        holder: LeaseHolder,
    },
}

/// The fix of a `NO_DATABASE` whose `metadata.json` exists but this process
/// may not read (PE-RBP-F014): `bd init` would not help.
const NO_DATABASE_DENIED_FIX: &str = "metadata.json exists but this process may not read it: \
     fix its permissions (or those of its directory), or run as a user who can read it";

impl CacheError {
    /// The `BD_CACHE.*` code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::BdUnavailable { .. } => "BD_CACHE.BD_UNAVAILABLE",
            Self::NoDatabase { .. } => "BD_CACHE.NO_DATABASE",
            Self::WrongDatabase { .. } => "BD_CACHE.WRONG_DATABASE",
            Self::NotFound { .. } => "BD_CACHE.NOT_FOUND",
            Self::BdVersionFailed { .. } => "BD_CACHE.BD_VERSION_FAILED",
            Self::BdSchemaFailed { .. } => "BD_CACHE.BD_SCHEMA_FAILED",
            Self::BdContract { .. } => "BD_CACHE.BD_CONTRACT",
            Self::BdWriteFailed { .. } => "BD_CACHE.BD_WRITE_FAILED",
            Self::BdListFailed { .. } => "BD_CACHE.BD_LIST_FAILED",
            Self::SyncFailed { .. } => "BD_CACHE.SYNC_FAILED",
            Self::Precondition { .. } => "BD_CACHE.PRECONDITION",
            Self::IdemConflict { .. } => "BD_CACHE.IDEM_CONFLICT",
            Self::BadOp { .. } => "BD_CACHE.BAD_OP",
            Self::BadArgs { .. } => "BD_CACHE.BAD_ARGS",
            Self::BadPrecondition { .. } => "BD_CACHE.BAD_PRECONDITION",
            Self::SchemaVersion { .. } => "BD_CACHE.SCHEMA_VERSION",
            Self::PruneBlocked { .. } => "BD_CACHE.PRUNE_BLOCKED",
            Self::NotEscalated { .. } => "BD_CACHE.NOT_ESCALATED",
            Self::ScopeUnknown { .. } => "BD_CACHE.SCOPE_UNKNOWN",
            Self::StoreFailed { .. } => "BD_CACHE.STORE_FAILED",
            Self::StoreInUse { .. } => "BD_CACHE.STORE_IN_USE",
            Self::ConfigInvalid { .. } => "BD_CACHE.CONFIG_INVALID",
            Self::IngestInvalid { .. } => "BD_CACHE.INGEST_INVALID",
            Self::ScheduleInvalid { .. } => "BD_CACHE.SCHEDULE_INVALID",
            Self::TickRunFailed { .. } => "BD_CACHE.TICK_RUN_FAILED",
            Self::TickInFlight { .. } => "BD_CACHE.TICK_IN_FLIGHT",
        }
    }

    /// The fix carried with the error (the BD-CACHE baseline design, CLI table).
    #[must_use]
    pub fn fix(&self) -> &'static str {
        match self {
            Self::BdUnavailable { .. } => {
                "transient backend outage: the drain retries without counting an attempt; a timed-out write retries"
            }
            Self::NoDatabase { source, .. }
                if source.as_ref().is_some_and(is_permission_denied) =>
            {
                NO_DATABASE_DENIED_FIX
            }
            Self::NoDatabase { .. } => {
                "run from the beads repository root (or set BEADS_DIR) after bd init"
            }
            Self::WrongDatabase { .. } => {
                "move the cache file aside; it was built from a different dolt database"
            }
            Self::NotFound { .. } => "check the bead id",
            Self::BdVersionFailed { .. } => "check that bd is installed and on PATH",
            Self::BdSchemaFailed { .. } | Self::BdListFailed { .. } => {
                "transient: retried by the next run"
            }
            Self::BdContract { .. } => {
                "bd output no longer matches the strict model; regenerate sc-beads' rows from `bd schema`"
            }
            Self::BdWriteFailed { .. } => "transient: retried by the next drain",
            Self::SyncFailed { .. } => "retried by the next sync; see cause",
            Self::Precondition { .. } => "re-read the bead and decide again",
            Self::IdemConflict { .. } => "use a key derived from the whole operation",
            Self::BadOp { .. } => "use one of close, update, dep_add",
            Self::BadArgs { .. } => "fix the args named in detail",
            Self::BadPrecondition { .. } => "precondition keys must be status and assignee",
            Self::SchemaVersion { .. } => {
                "upgrade bd-cache, or move the file aside to rebuild it from bd"
            }
            Self::PruneBlocked { .. } => {
                "drain, or resolve the escalated entries with the lead, then prune"
            }
            Self::NotEscalated { .. } => "resolve only entries listed as escalated in status",
            Self::ScopeUnknown { .. } => "sync the scope first, or use one of the known scopes",
            Self::StoreFailed { .. } => {
                "check the cache file named in detail (permissions, free disk space, another process holding it); if it is corrupt, move it aside and the next sync rebuilds it from bd (pending writes in it are lost)"
            }
            Self::StoreInUse { .. } => {
                "another handle in this process already has this cache file open: close that handle, or reuse it instead of opening a second one; the file itself is fine"
            }
            Self::ConfigInvalid { .. } => {
                "change the scope in .beads/bd-cache.toml, which defines it; the file wins"
            }
            Self::IngestInvalid { .. } => {
                "fix the named row or limit and resend the whole snapshot"
            }
            Self::ScheduleInvalid { .. } => {
                "fix the named option; switch rows with bdc cache sync use-python|use-builtin"
            }
            Self::TickRunFailed { .. } => {
                "read stderr; bdc cache sync use-builtin restores the built-in sync"
            }
            Self::TickInFlight { .. } => "retry after expires_at",
        }
    }

    /// True when `bd` could not answer at all (the [`UNREACHABLE_CODES`] variants).
    ///
    /// An exhaustive match: a new variant must be classified here to compile.
    #[must_use]
    pub fn is_unreachable(&self) -> bool {
        match self {
            Self::BdUnavailable { .. }
            | Self::BdVersionFailed { .. }
            | Self::BdSchemaFailed { .. }
            | Self::BdListFailed { .. } => true,
            Self::NoDatabase { .. }
            | Self::WrongDatabase { .. }
            | Self::NotFound { .. }
            | Self::BdContract { .. }
            | Self::BdWriteFailed { .. }
            | Self::SyncFailed { .. }
            | Self::Precondition { .. }
            | Self::IdemConflict { .. }
            | Self::BadOp { .. }
            | Self::BadArgs { .. }
            | Self::BadPrecondition { .. }
            | Self::SchemaVersion { .. }
            | Self::PruneBlocked { .. }
            | Self::NotEscalated { .. }
            | Self::ScopeUnknown { .. }
            | Self::StoreFailed { .. }
            | Self::StoreInUse { .. }
            | Self::ConfigInvalid { .. }
            | Self::IngestInvalid { .. }
            | Self::ScheduleInvalid { .. }
            | Self::TickRunFailed { .. }
            | Self::TickInFlight { .. } => false,
        }
    }

    /// Whether bd could not be reached, for an error that can come from bd
    /// (`details.outage`, REQ-BDC-0729); `None` for an error that cannot.
    ///
    /// True only for a spawn failure, a timeout (a lock wait included) or a
    /// transport failure; an error from a bd that answered (a `BD_LIST_FAILED`
    /// for a nonzero exit, a contract break, a refusal) is `Some(false)`. Every
    /// `BD_UNAVAILABLE` is an outage, including one the caller's own deadline cut.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::{CacheError, CallKind};
    /// use sc_beads::BeadsCacheError;
    /// let down = BeadsCacheError::Unavailable { reason: "refused".into(), timed_out: false };
    /// let e = CacheError::from_backend(&down, CallKind::Refresh);
    /// assert_eq!(e.outage(), Some(true));
    /// let answered = BeadsCacheError::Failed { code: 1, stderr: "no".into() };
    /// let e = CacheError::from_backend(&answered, CallKind::Refresh);
    /// assert_eq!(e.code(), "BD_CACHE.BD_LIST_FAILED");
    /// assert_eq!(e.outage(), Some(false));
    /// assert_eq!(CacheError::BadArgs { detail: "x".into() }.outage(), None);
    /// ```
    #[must_use]
    pub fn outage(&self) -> Option<bool> {
        let unreached = |source: &Option<BackendCause>| {
            source.as_ref().is_some_and(|c| {
                c.error().is_unavailable() || matches!(c.error(), BeadsCacheError::Transport { .. })
            })
        };
        match self {
            // bd timed out, could not start or could not be reached, or the
            // caller's own deadline cut the call: always unreached.
            Self::BdUnavailable { .. } => Some(true),
            Self::BdVersionFailed { source, .. }
            | Self::BdSchemaFailed { source, .. }
            | Self::BdContract { source, .. }
            | Self::BdWriteFailed { source, .. }
            | Self::BdListFailed { source, .. }
            | Self::Precondition { source, .. } => Some(unreached(source)),
            Self::SyncFailed { source, cause, .. } => {
                Some(unreached(source) || matches!(cause, Some(SyncCause::BdUnavailable)))
            }
            Self::NoDatabase { .. }
            | Self::WrongDatabase { .. }
            | Self::NotFound { .. }
            | Self::IdemConflict { .. }
            | Self::BadOp { .. }
            | Self::BadArgs { .. }
            | Self::BadPrecondition { .. }
            | Self::SchemaVersion { .. }
            | Self::PruneBlocked { .. }
            | Self::NotEscalated { .. }
            | Self::ScopeUnknown { .. }
            | Self::StoreFailed { .. }
            | Self::StoreInUse { .. }
            | Self::ConfigInvalid { .. }
            | Self::IngestInvalid { .. }
            | Self::ScheduleInvalid { .. }
            | Self::TickRunFailed { .. }
            | Self::TickInFlight { .. } => None,
        }
    }

    /// The payload keys [`to_json`](Self::to_json) adds to the envelope for this
    /// code (not counting the `source` a mapped error adds).
    ///
    /// An exhaustive match: a new variant must be classified here to compile. Test
    /// only (`pub(crate)`, no consumer outside this crate's tests: ADR-MODEL-0002
    /// amendment, a-8).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn payload_keys(&self) -> &'static [&'static str] {
        match self {
            Self::BdContract { .. } => &["bead", "field"],
            Self::Precondition { .. } => &["unmet"],
            Self::IdemConflict { .. } => &["seq"],
            Self::SyncFailed { .. } => &["cause", "phase"],
            Self::ScopeUnknown { .. } => &["known"],
            Self::ConfigInvalid { .. } => &["config_source"],
            Self::IngestInvalid { .. } => &["scope", "ingest_reason", "index"],
            Self::ScheduleInvalid { .. } => &["field", "schedule_reason"],
            Self::TickRunFailed { .. } => &[
                "row",
                "run_id",
                "run_reason",
                "exit_status",
                "stderr",
                "restored",
            ],
            Self::TickInFlight { .. } => &["holder"],
            Self::BdUnavailable { .. }
            | Self::NoDatabase { .. }
            | Self::WrongDatabase { .. }
            | Self::NotFound { .. }
            | Self::BdVersionFailed { .. }
            | Self::BdSchemaFailed { .. }
            | Self::BdWriteFailed { .. }
            | Self::BdListFailed { .. }
            | Self::BadOp { .. }
            | Self::BadArgs { .. }
            | Self::BadPrecondition { .. }
            | Self::SchemaVersion { .. }
            | Self::PruneBlocked { .. }
            | Self::NotEscalated { .. }
            | Self::StoreFailed { .. }
            | Self::StoreInUse { .. } => &[],
        }
    }

    /// The `/v0` refusal reason: `precondition_failed`, `not_closable` or `already_claimed`.
    #[must_use]
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Precondition { reason, .. } => Some(reason.as_str()),
            _ => None,
        }
    }

    /// The process exit code: 3 for every typed error (REQ-BDC-0802).
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        TYPED_ERROR_EXIT
    }

    /// The detail text.
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            Self::BdUnavailable { detail, .. }
            | Self::NoDatabase { detail, .. }
            | Self::WrongDatabase { detail }
            | Self::NotFound { detail }
            | Self::BdVersionFailed { detail, .. }
            | Self::BdSchemaFailed { detail, .. }
            | Self::BdContract { detail, .. }
            | Self::BdWriteFailed { detail, .. }
            | Self::BdListFailed { detail, .. }
            | Self::SyncFailed { detail, .. }
            | Self::Precondition { detail, .. }
            | Self::IdemConflict { detail, .. }
            | Self::BadOp { detail }
            | Self::BadArgs { detail }
            | Self::BadPrecondition { detail }
            | Self::SchemaVersion { detail }
            | Self::PruneBlocked { detail }
            | Self::NotEscalated { detail }
            | Self::ScopeUnknown { detail, .. }
            | Self::StoreFailed { detail, .. }
            | Self::StoreInUse { detail }
            | Self::ConfigInvalid { detail, .. }
            | Self::IngestInvalid { detail, .. }
            | Self::ScheduleInvalid { detail, .. }
            | Self::TickRunFailed { detail, .. }
            | Self::TickInFlight { detail, .. } => detail,
        }
    }

    /// The backend error this one was mapped from.
    #[must_use]
    pub fn backend_cause(&self) -> Option<&BackendCause> {
        match self {
            Self::BdUnavailable { source, .. }
            | Self::BdVersionFailed { source, .. }
            | Self::BdSchemaFailed { source, .. }
            | Self::BdContract { source, .. }
            | Self::BdWriteFailed { source, .. }
            | Self::BdListFailed { source, .. }
            | Self::SyncFailed { source, .. }
            | Self::Precondition { source, .. } => source.as_ref(),
            _ => None,
        }
    }

    /// The JSON reply: `{ok: false, code, detail, fix, reason?, ...payload, source?}`.
    ///
    /// Payload keys: `BD_CONTRACT` `bead`, `field`; `PRECONDITION` `unmet`;
    /// `IDEM_CONFLICT` `seq`; `SYNC_FAILED` `phase`, `cause`; `SCOPE_UNKNOWN`
    /// `known`; `CONFIG_INVALID` `config_source`; `INGEST_INVALID` `scope`,
    /// `ingest_reason`, `index`; `SCHEDULE_INVALID` `field`, `schedule_reason`;
    /// `TICK_RUN_FAILED` `row`, `run_id`, `run_reason`, `exit_status`, `stderr`,
    /// `restored`; `TICK_IN_FLIGHT` `holder`. (`reason` is the `/v0` refusal
    /// reason of `PRECONDITION` only; `bd-cache-api` names these `reason`.) A mapped error adds `source: {code, detail}`, the backend error;
    /// a `STORE_FAILED` or `NO_DATABASE` with a cause adds `source: {detail}`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "ok": false,
            "code": self.code(),
            "detail": self.detail(),
            "fix": self.fix(),
        });
        let extra = match self {
            Self::BdContract { bead, field, .. } => json!({"bead": bead, "field": field}),
            Self::Precondition { unmet, reason, .. } => {
                json!({"unmet": unmet, "reason": reason.as_str()})
            }
            Self::IdemConflict { seq, .. } => json!({"seq": seq}),
            Self::SyncFailed { scope, cause, .. } => json!({
                "phase": scope.as_ref().map(ScopeId::name),
                "cause": cause,
            }),
            Self::ScopeUnknown { known, .. } => json!({"known": known}),
            Self::ConfigInvalid { origin, .. } => json!({"config_source": origin}),
            Self::IngestInvalid {
                scope,
                reason,
                index,
                ..
            } => json!({"scope": scope, "ingest_reason": reason, "index": index}),
            Self::ScheduleInvalid { field, reason, .. } => {
                json!({"field": field, "schedule_reason": reason})
            }
            Self::TickRunFailed {
                row,
                run_id,
                reason,
                exit_status,
                stderr,
                restored,
                ..
            } => json!({
                "row": row,
                "run_id": run_id,
                "run_reason": reason,
                "exit_status": exit_status,
                "stderr": stderr,
                "restored": restored,
            }),
            Self::TickInFlight { holder, .. } => json!({"holder": holder}),
            _ => Value::Null,
        };
        if let (Value::Object(base), Value::Object(more)) = (&mut v, extra) {
            base.extend(more);
        }
        if let (Value::Object(base), Some(cause)) = (&mut v, self.backend_cause()) {
            base.insert(
                "source".into(),
                json!({"code": cause.error().code(), "detail": cause.error().to_string()}),
            );
        }
        if let (Value::Object(base), Some(cause)) = (&mut v, self.store_cause()) {
            base.insert(
                "source".into(),
                json!({"detail": cause.error().to_string()}),
            );
        }
        v
    }

    /// The local failure this error was built from (`STORE_FAILED`, `NO_DATABASE`).
    #[must_use]
    pub fn store_cause(&self) -> Option<&StoreCause> {
        match self {
            Self::StoreFailed { source, .. } | Self::NoDatabase { source, .. } => source.as_ref(),
            _ => None,
        }
    }

    /// A `STORE_FAILED` for `detail` (the failing step and file), keeping `source`.
    #[must_use]
    pub fn store_failed(
        detail: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> CacheError {
        CacheError::StoreFailed {
            detail: detail.into(),
            source: Some(StoreCause::new(source)),
        }
    }

    /// Maps a backend error by the kind of call that failed (table in the module docs).
    #[must_use]
    pub fn from_backend(e: &BeadsCacheError, kind: CallKind) -> CacheError {
        let source = || Some(BackendCause::new(clone_backend(e)));
        match e {
            BeadsCacheError::Unavailable { reason, .. } => Self::BdUnavailable {
                detail: reason.clone(),
                source: source(),
            },
            BeadsCacheError::Btit(err @ BeadsError::Spawn { .. }) => Self::BdUnavailable {
                detail: err.to_string(),
                source: source(),
            },
            BeadsCacheError::Contract {
                bead,
                field,
                detail,
            } => Self::BdContract {
                bead: bead.clone(),
                field: field.clone(),
                detail: detail.clone(),
                source: source(),
            },
            BeadsCacheError::Btit(
                err @ (BeadsError::InvalidJson { .. }
                | BeadsError::UnexpectedShape { .. }
                | BeadsError::ParseFailed { .. }),
            ) => Self::BdContract {
                bead: None,
                field: String::new(),
                detail: err.to_string(),
                source: source(),
            },
            BeadsCacheError::Refused(
                r @ (Refusal::Precondition | Refusal::NotClosable | Refusal::AlreadyClaimed),
            ) => {
                let reason = match r {
                    Refusal::NotClosable => PreconditionReason::NotClosable,
                    Refusal::AlreadyClaimed => PreconditionReason::AlreadyClaimed,
                    _ => PreconditionReason::PreconditionFailed,
                };
                Self::Precondition {
                    detail: format!("bd refused: {r}"),
                    unmet: BTreeMap::new(),
                    reason,
                    source: source(),
                }
            }
            BeadsCacheError::CheckFailed {
                step: CheckStep::Schema,
                ..
            } => Self::BdSchemaFailed {
                detail: e.to_string(),
                source: source(),
            },
            BeadsCacheError::CheckFailed { .. } => Self::BdVersionFailed {
                detail: e.to_string(),
                source: source(),
            },
            BeadsCacheError::Transport { source: io, .. } => {
                transport_failed(kind, format!("{e}: {io}"), source())
            }
            BeadsCacheError::NotFound { bead } => Self::NotFound {
                detail: format!("bead {bead} is not in bd; nothing was written"),
            },
            // Refused before any bd call: the request, not bd, is at fault.
            BeadsCacheError::Invalid(invalid) => Self::BadArgs {
                detail: invalid.to_string(),
            },
            other => failed(kind, other.to_string(), source()),
        }
    }
}

/// A copy of a backend error for the source chain. btit's `BeadsError` holds
/// non-clonable `io::Error`s, so a btit error is kept by its text.
fn clone_backend(e: &BeadsCacheError) -> BeadsCacheError {
    match e {
        BeadsCacheError::Unavailable { reason, timed_out } => BeadsCacheError::Unavailable {
            reason: reason.clone(),
            timed_out: *timed_out,
        },
        BeadsCacheError::Refused(r) => BeadsCacheError::Refused(r.clone()),
        BeadsCacheError::Contract {
            bead,
            field,
            detail,
        } => BeadsCacheError::Contract {
            bead: bead.clone(),
            field: field.clone(),
            detail: detail.clone(),
        },
        BeadsCacheError::Failed { code, stderr } => BeadsCacheError::Failed {
            code: *code,
            stderr: stderr.clone(),
        },
        BeadsCacheError::CheckFailed { step, code, stderr } => BeadsCacheError::CheckFailed {
            step: *step,
            code: *code,
            stderr: stderr.clone(),
        },
        // `io::Error` is not `Clone`: the copy keeps its kind and text.
        BeadsCacheError::Transport { what, source } => BeadsCacheError::Transport {
            what: what.clone(),
            source: std::io::Error::new(source.kind(), source.to_string()),
        },
        BeadsCacheError::NotFound { bead } => BeadsCacheError::NotFound { bead: bead.clone() },
        BeadsCacheError::Invalid(invalid) => BeadsCacheError::Invalid(invalid.clone()),
        other => BeadsCacheError::Failed {
            code: -1,
            stderr: format!("{}: {other}", other.code()),
        },
    }
}

/// A reachable backend failing, by call kind.
fn failed(kind: CallKind, detail: String, source: Option<BackendCause>) -> CacheError {
    match kind {
        CallKind::Write => CacheError::BdWriteFailed { detail, source },
        CallKind::PreRead
        | CallKind::ReadBack
        | CallKind::Refresh
        | CallKind::MissFill
        | CallKind::Ready => CacheError::BdListFailed { detail, source },
        CallKind::Sync => CacheError::SyncFailed {
            detail,
            scope: None,
            cause: Some(SyncCause::BdListFailed),
            source,
        },
        CallKind::Check => CacheError::BdVersionFailed { detail, source },
    }
}

/// Output that could not be read whole (PE-RBP-F001), by call kind: the
/// transient codes, never `BD_CONTRACT` (nothing was parsed) and never
/// `BD_VERSION_FAILED` (bd is installed: it ran).
fn transport_failed(kind: CallKind, detail: String, source: Option<BackendCause>) -> CacheError {
    match kind {
        CallKind::Write => CacheError::BdWriteFailed { detail, source },
        CallKind::Sync => CacheError::SyncFailed {
            detail,
            scope: None,
            cause: Some(SyncCause::BdListFailed),
            source,
        },
        CallKind::Check
        | CallKind::PreRead
        | CallKind::ReadBack
        | CallKind::Refresh
        | CallKind::MissFill
        | CallKind::Ready => CacheError::BdListFailed { detail, source },
    }
}

/// True when the cause is an I/O error of kind `PermissionDenied`.
fn is_permission_denied(cause: &StoreCause) -> bool {
    cause
        .error()
        .downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied)
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.detail())
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Some(c) = self.store_cause() {
            return Some(c.error() as &(dyn std::error::Error + 'static));
        }
        self.backend_cause()
            .map(|c| c.error() as &(dyn std::error::Error + 'static))
    }
}

#[cfg(test)]
mod classification_tests {
    use std::collections::BTreeSet;

    use super::*;

    fn d() -> String {
        "detail".to_owned()
    }

    fn backend() -> BackendCause {
        BackendCause::new(BeadsCacheError::Unavailable {
            reason: "down".to_owned(),
            timed_out: false,
        })
    }

    /// Declares, once, the variant names and `witness`, the exhaustive match (no
    /// wildcard arm) that names each variant: a new variant does not compile until it is
    /// listed here, and `every_arm_has_a_sample` then needs a sample of it.
    macro_rules! arms {
        ($($v:ident),+ $(,)?) => {
            const ARMS: &[&str] = &[$(stringify!($v)),+];

            fn witness(e: &CacheError) -> &'static str {
                match e {
                    $(CacheError::$v { .. } => stringify!($v)),+
                }
            }
        };
    }

    arms!(
        BdUnavailable,
        NoDatabase,
        WrongDatabase,
        NotFound,
        BdVersionFailed,
        BdSchemaFailed,
        BdContract,
        BdWriteFailed,
        BdListFailed,
        SyncFailed,
        Precondition,
        IdemConflict,
        BadOp,
        BadArgs,
        BadPrecondition,
        SchemaVersion,
        PruneBlocked,
        NotEscalated,
        ScopeUnknown,
        StoreFailed,
        StoreInUse,
        ConfigInvalid,
        IngestInvalid,
        ScheduleInvalid,
        TickRunFailed,
        TickInFlight,
    );

    /// Samples of every variant, with and without a kept source.
    #[expect(clippy::too_many_lines, reason = "a table of one sample per case")]
    fn samples() -> Vec<CacheError> {
        let x = ScopeId::new("x").unwrap();
        let unmet = BTreeMap::from([(
            GuardKey::Status,
            Unmet {
                expected: "open".to_owned(),
                actual: "closed".to_owned(),
            },
        )]);
        let store = || Some(StoreCause::new(std::io::Error::other("disk")));
        vec![
            CacheError::BdUnavailable {
                detail: d(),
                source: None,
            },
            CacheError::BdUnavailable {
                detail: d(),
                source: Some(backend()),
            },
            CacheError::NoDatabase {
                detail: d(),
                source: None,
            },
            CacheError::NoDatabase {
                detail: d(),
                source: store(),
            },
            CacheError::WrongDatabase { detail: d() },
            CacheError::NotFound { detail: d() },
            CacheError::BdVersionFailed {
                detail: d(),
                source: None,
            },
            CacheError::BdVersionFailed {
                detail: d(),
                source: Some(backend()),
            },
            CacheError::BdSchemaFailed {
                detail: d(),
                source: None,
            },
            CacheError::BdSchemaFailed {
                detail: d(),
                source: Some(backend()),
            },
            CacheError::BdContract {
                bead: Some(BeadId::new("b1").unwrap()),
                field: "priority".to_owned(),
                detail: d(),
                source: None,
            },
            CacheError::BdContract {
                bead: None,
                field: String::new(),
                detail: d(),
                source: Some(backend()),
            },
            CacheError::BdWriteFailed {
                detail: d(),
                source: None,
            },
            CacheError::BdWriteFailed {
                detail: d(),
                source: Some(backend()),
            },
            CacheError::BdListFailed {
                detail: d(),
                source: None,
            },
            CacheError::BdListFailed {
                detail: d(),
                source: Some(backend()),
            },
            CacheError::SyncFailed {
                detail: d(),
                scope: Some(x.clone()),
                cause: Some(SyncCause::BdContract),
                source: None,
            },
            CacheError::SyncFailed {
                detail: d(),
                scope: None,
                cause: None,
                source: Some(backend()),
            },
            CacheError::Precondition {
                detail: d(),
                unmet: unmet.clone(),
                reason: PreconditionReason::PreconditionFailed,
                source: None,
            },
            CacheError::Precondition {
                detail: d(),
                unmet: BTreeMap::new(),
                reason: PreconditionReason::AlreadyClaimed,
                source: Some(backend()),
            },
            CacheError::IdemConflict {
                detail: d(),
                seq: Seq::new(1),
            },
            CacheError::BadOp { detail: d() },
            CacheError::BadArgs { detail: d() },
            CacheError::BadPrecondition { detail: d() },
            CacheError::SchemaVersion { detail: d() },
            CacheError::PruneBlocked { detail: d() },
            CacheError::NotEscalated { detail: d() },
            CacheError::ScopeUnknown {
                detail: d(),
                known: vec![x.clone()],
            },
            CacheError::ConfigInvalid {
                detail: d(),
                origin: "config_scope",
            },
            CacheError::IngestInvalid {
                detail: d(),
                scope: x,
                reason: IngestReason::RowInvalid,
                index: Some(3),
            },
            CacheError::ScheduleInvalid {
                detail: d(),
                field: "python".to_owned(),
                reason: "not_absolute".to_owned(),
            },
            CacheError::TickRunFailed {
                detail: d(),
                row: "python".to_owned(),
                run_id: "r1".to_owned(),
                reason: TickRunReason::Exit,
                exit_status: Some(1),
                stderr: "boom".to_owned(),
                restored: Some(true),
            },
            CacheError::TickInFlight {
                detail: d(),
                holder: LeaseHolder {
                    run_id: "r0".to_owned(),
                    pid: 7,
                    host: "h".to_owned(),
                    expires_at: sc_beads::Timestamp::parse("2026-10-23T10:00:00Z").unwrap(),
                },
            },
            CacheError::StoreFailed {
                detail: d(),
                source: None,
            },
            CacheError::StoreFailed {
                detail: d(),
                source: store(),
            },
            CacheError::StoreInUse { detail: d() },
        ]
    }

    /// The outage truth table of REQ-BDC-0729, one row per case (AC5): spawn
    /// failure, timeout, lock, transport, and a `BD_LIST_FAILED` from a bd
    /// that answered.
    #[test]
    fn outage_is_true_only_when_bd_could_not_be_reached() {
        let spawn: BeadsCacheError = BeadsError::Spawn {
            binary: "bd".into(),
            operation: None,
            source: std::io::Error::other("missing"),
        }
        .into();
        let timeout = BeadsCacheError::Unavailable {
            reason: "bd list did not finish before its deadline (child)".into(),
            timed_out: true,
        };
        let lock = BeadsCacheError::Unavailable {
            reason: "bd list did not finish before its deadline (lock)".into(),
            timed_out: true,
        };
        let transport = BeadsCacheError::Transport {
            what: "bd list".into(),
            source: std::io::Error::other("stdout exceeded 64 MiB"),
        };
        let answered = BeadsCacheError::Failed {
            code: 1,
            stderr: "Error: bad filter".into(),
        };
        for (case, e, want) in [
            ("spawn", &spawn, true),
            ("timeout", &timeout, true),
            ("lock", &lock, true),
            ("transport", &transport, true),
            ("answered", &answered, false),
        ] {
            for kind in [CallKind::Refresh, CallKind::Sync, CallKind::Ready] {
                let mapped = CacheError::from_backend(e, kind);
                assert_eq!(mapped.outage(), Some(want), "{case} {kind:?}: {mapped}");
            }
        }
        let answered = CacheError::from_backend(&answered, CallKind::Refresh);
        assert_eq!(answered.code(), "BD_CACHE.BD_LIST_FAILED");
        assert_eq!(answered.outage(), Some(false));
    }

    #[test]
    fn every_arm_has_a_sample() {
        let covered: BTreeSet<&str> = samples().iter().map(witness).collect();
        assert_eq!(covered, ARMS.iter().copied().collect::<BTreeSet<_>>());
    }

    #[test]
    fn unreachable_codes_are_exactly_the_unreachable_variants() {
        let listed: BTreeSet<&str> = UNREACHABLE_CODES.into_iter().collect();
        let mut found = BTreeSet::new();
        for e in samples() {
            assert_eq!(
                listed.contains(e.code()),
                e.is_unreachable(),
                "{}",
                e.code()
            );
            if e.is_unreachable() {
                // An unreachable error is one of the sync causes (the drain and the sync agree).
                assert!(SyncCause::of(&e).is_some(), "{}", e.code());
                found.insert(e.code());
            }
        }
        assert_eq!(found, listed);
    }

    #[test]
    fn the_envelope_source_and_payload_keys_match_to_json_for_every_sample() {
        let envelope: BTreeSet<&str> = ENVELOPE_KEYS.into_iter().collect();
        for e in &samples() {
            let body = e.to_json();
            let keys: BTreeSet<&str> = body
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            for always in ["ok", "code", "detail", "fix"] {
                assert!(keys.contains(always), "{}: no {always}", e.code());
            }
            assert_eq!(
                keys.contains("reason"),
                e.reason().is_some(),
                "{}",
                e.code()
            );
            let has_source = e.backend_cause().is_some() || e.store_cause().is_some();
            assert_eq!(keys.contains("source"), has_source, "{}", e.code());
            let payload: BTreeSet<&str> = keys
                .difference(&envelope)
                .copied()
                .filter(|k| *k != "source")
                .collect();
            let want: BTreeSet<&str> = e.payload_keys().iter().copied().collect();
            assert_eq!(payload, want, "{}", e.code());
        }
    }
}
