//! [`ApiError`] and the error table ([`ERROR_TABLE`], contract §4).
//!
//! Every front end reports one error shape, `{kind, code, message, details,
//! suggested_action}`, and takes a code's kind, exit and retryable flag from
//! [`ERROR_TABLE`], which equals the contract's error table (a test parses
//! the table out of `docs/bd-cache-cli/contract.md` and compares).
//!
//! [`From<CacheError>`](ApiError#impl-From<CacheError>-for-ApiError) maps the
//! core's error field by field (contract §4):
//!
//! | `CacheError` | `ApiError` |
//! |---|---|
//! | `code` | `code`, unchanged |
//! | `detail` | `message` |
//! | `fix` | `suggested_action` |
//! | `reason` | `details.v0_code` (else the table's `v0_code`) |
//! | `unmet` | `details.unmet: {status?: {expected, actual}, assignee?: …}` |
//! | `seq`, `bead`, `field`, `cause`, `known`, `source` | `details.*`, same names (`known` as scope names) |
//! | `phase` | `details.scope`, the scope's name (ADR-BDC-0013) |
//! | the payload of `CONFIG_INVALID`, `INGEST_INVALID`, `SCHEDULE_INVALID`, `TICK_RUN_FAILED`, `TICK_IN_FLIGHT` | `details.*` with REQ-BDC-0729's keys (`source`; `scope`, `reason`, `index`; `field`, `reason`; `row`, `run_id`, `reason`, `exit_status`, `stderr`, `restored`; `holder`) |
//! | [`CacheError::outage`] | `details.outage`, on every error that can come from bd (REQ-BDC-0729) |
//!
//! The api crate depends on neither the store nor the sync crate, so their
//! configuration errors reach it as text: a front end maps them with
//! [`ApiError::config_invalid`].

use std::fmt;

use bd_cache_model::{CacheError, ScopeId};
use serde::Serialize;
use serde_json::{Map, Value, json};

/// The prefix every code carries (`BD_CACHE.`); the table leaves it out.
pub const CODE_PREFIX: &str = "BD_CACHE.";

/// The kind of an error: what a caller branches on (contract §3, §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The request is wrong; repeating it fails the same way.
    InvalidInput,
    /// The bead or entry does not exist.
    NotFound,
    /// bd or the cached answer is not available now; a retry may succeed.
    Unavailable,
    /// The request is well formed, the state refuses it, nothing was written.
    Conflict,
    /// A bug or a broken store.
    Internal,
}

impl ErrorKind {
    /// The wire name, e.g. `invalid_input`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::Unavailable => "unavailable",
            Self::Conflict => "conflict",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One row of the error table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorSpec {
    /// The code without the `BD_CACHE.` prefix, e.g. `BAD_ARGS`.
    pub code: &'static str,
    /// The code's one fixed kind (REQ-BDC-0803).
    pub kind: ErrorKind,
    /// The process exit; `None` for a code that never ends a command
    /// (`BD_WRITE_FAILED` lives in `Entry.last_error` only).
    pub exit: Option<i32>,
    /// Whether a retry of the same request may succeed.
    pub retryable: bool,
    /// The `code` a `/v0` Problem would carry, when there is one.
    pub v0_code: Option<&'static str>,
}

const fn row(
    code: &'static str,
    kind: ErrorKind,
    exit: Option<i32>,
    retryable: bool,
    v0_code: Option<&'static str>,
) -> ErrorSpec {
    ErrorSpec {
        code,
        kind,
        exit,
        retryable,
        v0_code,
    }
}

use ErrorKind::{Conflict, Internal, InvalidInput, NotFound, Unavailable};

/// Exit of a usage error (contract §3).
const USAGE_EXIT: Option<i32> = Some(2);
/// Exit of every other typed error (contract §3).
const TYPED_EXIT: Option<i32> = Some(3);

/// The error table of contract §4: code, kind, exit, retryable and `v0_code`.
///
/// Every code is one row (REQ-BDC-0729). The `TICK_<REFUSAL>` family is
/// removed with the refusals it reported (ADR-BDC-0015, sprint b-8).
pub const ERROR_TABLE: &[ErrorSpec] = &[
    row("USAGE", InvalidInput, USAGE_EXIT, false, None),
    row(
        "BAD_ARGS",
        InvalidInput,
        TYPED_EXIT,
        false,
        Some("invalid_argument"),
    ),
    row(
        "BAD_PRECONDITION",
        InvalidInput,
        TYPED_EXIT,
        false,
        Some("invalid_argument"),
    ),
    row(
        "UNSUPPORTED",
        InvalidInput,
        TYPED_EXIT,
        false,
        Some("invalid_argument"),
    ),
    row("CONFIG_INVALID", InvalidInput, TYPED_EXIT, false, None),
    row(
        "BAD_OP",
        InvalidInput,
        TYPED_EXIT,
        false,
        Some("invalid_argument"),
    ),
    row("NOT_FOUND", NotFound, TYPED_EXIT, false, Some("not_found")),
    row("NO_DATABASE", NotFound, TYPED_EXIT, false, None),
    row("NOT_CACHED", Unavailable, TYPED_EXIT, true, None),
    row("SCOPE_UNKNOWN", Conflict, TYPED_EXIT, false, None),
    row(
        "PRECONDITION",
        Conflict,
        TYPED_EXIT,
        false,
        Some("precondition_failed"),
    ),
    row("IDEM_CONFLICT", Conflict, TYPED_EXIT, false, None),
    row("PRUNE_BLOCKED", Conflict, TYPED_EXIT, false, None),
    row("NOT_ESCALATED", Conflict, TYPED_EXIT, false, None),
    row("WRONG_DATABASE", Conflict, TYPED_EXIT, false, None),
    row("SCHEMA_VERSION", Conflict, TYPED_EXIT, false, None),
    row("STORE_IN_USE", Conflict, TYPED_EXIT, false, None),
    row(
        "BD_UNAVAILABLE",
        Unavailable,
        TYPED_EXIT,
        true,
        Some("db_unavailable"),
    ),
    row(
        "BD_VERSION_FAILED",
        Unavailable,
        TYPED_EXIT,
        true,
        Some("db_unavailable"),
    ),
    row(
        "BD_SCHEMA_FAILED",
        Unavailable,
        TYPED_EXIT,
        true,
        Some("db_unavailable"),
    ),
    row(
        "BD_LIST_FAILED",
        Unavailable,
        TYPED_EXIT,
        true,
        Some("db_unavailable"),
    ),
    row(
        "SYNC_FAILED",
        Unavailable,
        TYPED_EXIT,
        true,
        Some("db_unavailable"),
    ),
    row("BD_WRITE_FAILED", Unavailable, None, true, None),
    row("BD_CONTRACT", Internal, TYPED_EXIT, false, Some("internal")),
    row(
        "STORE_FAILED",
        Internal,
        TYPED_EXIT,
        false,
        Some("internal"),
    ),
    row("INTERNAL", Internal, TYPED_EXIT, false, Some("internal")),
    // REQ-BDC-0729 (ADR-BDC-0015): the phase-B codes, one row each.
    row("INGEST_INVALID", InvalidInput, TYPED_EXIT, false, None),
    row("SCHEDULE_INVALID", InvalidInput, TYPED_EXIT, false, None),
    row("TICK_RUN_FAILED", Unavailable, TYPED_EXIT, false, None),
    row("TICK_IN_FLIGHT", Conflict, TYPED_EXIT, true, None),
];

/// The table row of `code`, with or without the `BD_CACHE.` prefix.
///
/// # Examples
///
/// ```
/// use bd_cache_api::{ErrorKind, spec};
/// assert_eq!(spec("BD_CACHE.SCOPE_UNKNOWN").map(|s| s.kind), Some(ErrorKind::Conflict));
/// assert_eq!(spec("NOT_CACHED").map(|s| s.retryable), Some(true));
/// assert!(spec("NO_TOML").is_none());
/// ```
#[must_use]
pub fn spec(code: &str) -> Option<&'static ErrorSpec> {
    let bare = code.strip_prefix(CODE_PREFIX).unwrap_or(code);
    ERROR_TABLE.iter().find(|s| s.code == bare)
}

/// What kind of `/v0` name a request used that the cache does not serve
/// (`details.unsupported`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unsupported {
    /// A `/v0` operation (contract §5.4).
    Operation,
    /// A `/v0` query parameter, or a value of one.
    Parameter,
    /// A `/v0` request body member.
    Member,
}

impl Unsupported {
    /// The wire name: `operation`, `parameter` or `member`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Operation => "operation",
            Self::Parameter => "parameter",
            Self::Member => "member",
        }
    }
}

/// The error every `bd-cache-api` operation returns (contract §4, §8).
///
/// Serializes as `{kind, code, message, details, suggested_action}`, the
/// `error` member of the sc-command envelope.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApiError {
    kind: ErrorKind,
    code: String,
    message: String,
    details: Map<String, Value>,
    suggested_action: Option<String>,
    #[serde(skip)]
    exit: Option<i32>,
    #[serde(skip)]
    retryable: bool,
}

impl ApiError {
    /// An error with a table code (bare, e.g. `BAD_ARGS`).
    ///
    /// A code missing from the table is a bug in this crate: it is reported
    /// as `INTERNAL` naming the code, never with an invented kind.
    fn table(code: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        match spec(code) {
            Some(s) => Self {
                kind: s.kind,
                code: format!("{CODE_PREFIX}{}", s.code),
                message,
                details: Map::new(),
                suggested_action: None,
                exit: s.exit,
                retryable: s.retryable,
            },
            None => Self::table("INTERNAL", format!("unlisted error code {code}: {message}")),
        }
    }

    /// The kind.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The full code, e.g. `BD_CACHE.BAD_ARGS`.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// What happened.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The informative `details` object (contract §4; not covered by semver).
    #[must_use]
    pub fn details(&self) -> &Map<String, Value> {
        &self.details
    }

    /// What the caller can do about it.
    #[must_use]
    pub fn suggested_action(&self) -> Option<&str> {
        self.suggested_action.as_deref()
    }

    /// The process exit from the table: 2 for `USAGE`, 3 otherwise; `None`
    /// for a code that never ends a command (`BD_WRITE_FAILED`).
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.exit
    }

    /// Whether a retry of the same request may succeed (the table's column).
    #[must_use]
    pub fn retryable(&self) -> bool {
        self.retryable
    }

    /// Adds or replaces one `details` key.
    #[must_use]
    pub fn with_detail(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.details.insert(key.to_owned(), value.into());
        self
    }

    /// Sets the suggested action.
    #[must_use]
    pub fn with_suggested_action(mut self, action: impl Into<String>) -> Self {
        self.suggested_action = Some(action.into());
        self
    }

    /// The error as JSON: `{kind, code, message, details, suggested_action}`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "code": self.code,
            "message": self.message,
            "details": self.details,
            "suggested_action": self.suggested_action,
        })
    }

    /// `BAD_ARGS` (`v0_code: invalid_argument`).
    #[must_use]
    pub fn bad_args(message: impl Into<String>) -> Self {
        Self::table("BAD_ARGS", message)
            .with_detail("v0_code", "invalid_argument")
            .with_suggested_action("fix the request member named in the message")
    }

    /// `BAD_ARGS` naming the member at fault (`details.member`).
    #[must_use]
    pub fn bad_member(member: &str, message: impl Into<String>) -> Self {
        Self::bad_args(message).with_detail("member", member)
    }

    /// `BAD_ARGS` for a member the command does not know (contract §1 rule 5).
    #[must_use]
    pub fn unknown_member(member: &str) -> Self {
        Self::bad_member(member, format!("unknown request member {member:?}"))
            .with_suggested_action("remove the member; run the command's --help for its members")
    }

    /// `BAD_ARGS` for a cursor this crate did not mint (`v0_code: invalid_cursor`).
    #[must_use]
    pub fn invalid_cursor(message: impl Into<String>) -> Self {
        Self::bad_member("cursor", message)
            .with_detail("v0_code", "invalid_cursor")
            .with_suggested_action("pass back next_cursor unchanged, or start again without one")
    }

    /// `UNSUPPORTED`: a `/v0` name the cache does not serve (contract §1 rule 5).
    #[must_use]
    pub fn unsupported(what: Unsupported, name: &str) -> Self {
        let mut e = Self::table(
            "UNSUPPORTED",
            format!(
                "the cache does not serve the /v0 {} {name:?}",
                what.as_str()
            ),
        )
        .with_detail("v0_code", "invalid_argument")
        .with_detail("unsupported", what.as_str())
        .with_detail("name", name)
        .with_suggested_action("call bd or bd serve directly");
        if matches!(what, Unsupported::Parameter | Unsupported::Member) {
            e = e.with_detail("v0_reason", "unknown_parameter");
        }
        e
    }

    /// `NOT_CACHED`: a miss with `miss_timeout_ms: 0`, so bd was never asked.
    #[must_use]
    pub fn not_cached(id: &str) -> Self {
        Self::table(
            "NOT_CACHED",
            format!("bead {id} is not cached and the miss timeout is 0, so bd was not asked"),
        )
        .with_detail("bead", id)
        .with_suggested_action("retry with --miss-timeout-ms above 0")
    }

    /// `SCOPE_UNKNOWN` for a list-like read that names no scope (contract §5.1).
    #[must_use]
    pub fn scope_required(known: &[ScopeId]) -> Self {
        Self::table(
            "SCOPE_UNKNOWN",
            "a list-like read must name a known scope, with scope or a phase-<x> label",
        )
        .with_detail("known", labels(known))
        .with_suggested_action("pass scope, or a phase-<x> label of one of the known scopes")
    }

    /// `SCOPE_UNKNOWN` for a scope with no completed sync since it was last
    /// evicted, or ever (lead ruling S67): the cache does not serve it, and
    /// never answers it with an empty list.
    #[must_use]
    pub fn scope_not_served(scope: &ScopeId, known: &[ScopeId]) -> Self {
        let label = scope.name().to_owned();
        Self::table(
            "SCOPE_UNKNOWN",
            format!(
                "scope {label} has no completed sync (never synced, or evicted since): the \
                 cache does not serve it"
            ),
        )
        .with_detail("scope", label.as_str())
        .with_detail("known", labels(known))
        .with_suggested_action(format!("bdc cache refresh scopes --scope {label}"))
    }

    /// `CONFIG_INVALID`, keeping the store's or sync's code in `details.source`.
    ///
    /// `source` is the underlying code: `store_config_*`, `sync_config_*`,
    /// `sync_beads_dir_unresolvable` or `sync_repo_root_not_utf8`. `details`
    /// adds keys such as `key` (contract §2).
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_api::ApiError;
    /// let e = ApiError::config_invalid("store_config_toml", "bad toml", serde_json::Map::new());
    /// assert_eq!(e.code(), "BD_CACHE.CONFIG_INVALID");
    /// assert_eq!(e.details()["source"], "store_config_toml");
    /// ```
    #[must_use]
    pub fn config_invalid(
        source: &str,
        message: impl Into<String>,
        details: Map<String, Value>,
    ) -> Self {
        let mut e = Self::table("CONFIG_INVALID", message)
            .with_suggested_action("fix the configuration named in the message");
        e.details = details;
        e.with_detail("source", source)
    }

    /// `INTERNAL`: a bug, such as a caught panic.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::table("INTERNAL", message).with_detail("v0_code", "internal")
    }

    /// `USAGE`, for the shape of argv only (contract §3); `valid` lists the
    /// accepted words.
    #[must_use]
    pub fn usage(message: impl Into<String>, valid: &[&str]) -> Self {
        Self::table("USAGE", message)
            .with_detail("valid", valid.to_vec())
            .with_suggested_action("run `bdc <cmd> --help`")
    }
}

/// Scope names, as written: how every scope appears in the api (ADR-BDC-0013).
fn labels(scopes: &[ScopeId]) -> Vec<String> {
    scopes.iter().map(|s| s.name().to_owned()).collect()
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<CacheError> for ApiError {
    fn from(e: CacheError) -> Self {
        Self::from(&e)
    }
}

impl From<&CacheError> for ApiError {
    /// Field by field, as the module docs' table.
    fn from(e: &CacheError) -> Self {
        let mut out = Self::table(e.code(), e.detail()).with_suggested_action(e.fix());
        let table_v0 = spec(e.code()).and_then(|s| s.v0_code);
        if let Some(v0) = e.reason().or(table_v0) {
            out = out.with_detail("v0_code", v0);
        }
        match e {
            CacheError::Precondition { reason, .. } if reason.is_sync() => {
                // The sync's own reasons (REQ-BDC-0720, ADR-SYNC-0008).
                out = out.with_detail("reason", reason.as_str());
            }
            CacheError::Precondition { unmet, .. } if !unmet.is_empty() => {
                out = out.with_detail("unmet", json!(unmet));
            }
            CacheError::IdemConflict { seq, .. } => {
                out = out.with_detail("seq", seq.get());
            }
            CacheError::BdContract { bead, field, .. } => {
                out = out
                    .with_detail("bead", json!(bead))
                    .with_detail("field", field.as_str());
            }
            CacheError::SyncFailed { scope, cause, .. } => {
                out = out
                    .with_detail("scope", json!(scope.as_ref().map(ScopeId::name)))
                    .with_detail("cause", json!(cause));
            }
            CacheError::ScopeUnknown { known, .. } => {
                out = out.with_detail("known", labels(known));
            }
            CacheError::BdUnavailable { source, .. } => {
                // No source: the caller's own timeout cut the call (a miss
                // fill, a refresh); with one, the backend says whether it timed out.
                let timed_out = source.as_ref().is_none_or(|c| c.error().is_timeout());
                out = out.with_detail("timed_out", timed_out);
            }
            CacheError::ConfigInvalid { origin, .. } => {
                out = out.with_detail("source", origin.as_str());
            }
            CacheError::IngestInvalid {
                scope,
                reason,
                index,
                ..
            } => {
                out = out
                    .with_detail("scope", scope.name())
                    .with_detail("reason", reason.as_str());
                if let Some(index) = index {
                    out = out.with_detail("index", json!(index));
                }
            }
            CacheError::ScheduleInvalid { field, reason, .. } => {
                out = out
                    .with_detail("field", *field)
                    .with_detail("reason", reason.as_str());
            }
            CacheError::TickRunFailed {
                row,
                run_id,
                reason,
                exit_status,
                stderr,
                restored,
                ..
            } => {
                out = out
                    .with_detail("row", row.as_str())
                    .with_detail("run_id", run_id.as_str())
                    .with_detail("reason", reason.as_str())
                    .with_detail("exit_status", json!(exit_status))
                    .with_detail("stderr", stderr.as_str());
                if let Some(restored) = restored {
                    out = out.with_detail("restored", *restored);
                }
            }
            CacheError::TickInFlight { holder, .. } => {
                out = out.with_detail(
                    "holder",
                    json!({"pid": holder.pid, "host": holder.host, "expires_at": holder.expires_at}),
                );
            }
            _ => {}
        }
        // Every error that can come from bd says whether bd was reached (REQ-BDC-0729).
        if let Some(outage) = e.outage() {
            out = out.with_detail("outage", outage);
        }
        if let Some(cause) = e.backend_cause() {
            out = out.with_detail(
                "source",
                json!({"code": cause.error().code(), "detail": cause.error().to_string()}),
            );
        } else if let Some(cause) = e.store_cause() {
            out = out.with_detail("source", json!({"detail": cause.error().to_string()}));
        }
        out
    }
}
