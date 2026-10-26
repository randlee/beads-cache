//! The phase-B verbs' requests (ADR-BDC-0015, REQ-BDC-0720..0731).
//!
//! The store-only verbs (`cache scopes add|set|remove|list|ingest|announce`)
//! execute here, over `bd-cache` ([`execute`](crate::execute)). The others
//! (`cache schedule *`, `cache cron run`, `cache tick`, `cache sync
//! use-python|use-builtin`) need `bd-cache-sync`, which this crate does not
//! depend on: they are decoded here, so every front end refuses the same
//! input with the same code, and executed by the front end (`bd-cache-cli`,
//! `bd-cache-py`, later the MCP server).

use bd_cache_model::{FailingSince, IngestSnapshot, Scope, ScopeId};
use schemars::JsonSchema;

/// `cache scopes add <scope> [filter]`: registers a new scope.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct ScopesAdd {
    #[schemars(with = "String")]
    pub(crate) scope: Scope,
}

impl ScopesAdd {
    /// The scope to register, with its validated filter.
    #[must_use]
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
}

/// `cache scopes set <scope> [filter]`: changes a registered scope's filter.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct ScopesSet {
    #[schemars(with = "String")]
    pub(crate) scope: Scope,
}

impl ScopesSet {
    /// The scope and its new filter.
    #[must_use]
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
}

/// `cache scopes remove <scope>`: `cache evict` plus the registry removal.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct ScopesRemove {
    #[schemars(with = "String")]
    pub(crate) scope: ScopeId,
}

impl ScopesRemove {
    /// The scope to remove.
    #[must_use]
    pub fn scope(&self) -> &ScopeId {
        &self.scope
    }
}

/// `cache scopes list`: no members; `cache scopes` is its alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[non_exhaustive]
pub struct ScopesList;

/// `cache scopes ingest <scope>`: a consumer's snapshot (REQ-BDC-0726).
#[derive(Debug, Clone, PartialEq)]
pub struct ScopesIngest {
    pub(crate) scope: ScopeId,
    pub(crate) snapshot: IngestSnapshot,
    pub(crate) revision: Option<String>,
    pub(crate) allow_empty: bool,
}

impl ScopesIngest {
    /// The registered scope the snapshot replaces.
    #[must_use]
    pub fn scope(&self) -> &ScopeId {
        &self.scope
    }

    /// The strictly decoded snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &IngestSnapshot {
        &self.snapshot
    }

    /// The opaque revision stored with it.
    #[must_use]
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }

    /// Whether an empty snapshot is allowed.
    #[must_use]
    pub fn allow_empty(&self) -> bool {
        self.allow_empty
    }
}

/// `cache scopes announce <scope> --since <token>`: marks the current failure
/// notified, compare-and-set (REQ-BDC-0728).
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct ScopesAnnounce {
    #[schemars(with = "String")]
    pub(crate) scope: ScopeId,
    #[schemars(with = "String")]
    pub(crate) since: FailingSince,
}

impl ScopesAnnounce {
    /// The scope.
    #[must_use]
    pub fn scope(&self) -> &ScopeId {
        &self.scope
    }

    /// The `failing_since` token the caller saw.
    #[must_use]
    pub fn since(&self) -> &FailingSince {
        &self.since
    }
}

/// `cache schedule set <cron-expr> [...]` (REQ-BDC-0720); executed by the front end.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct ScheduleSet {
    pub(crate) cron: String,
    pub(crate) budget_ms: Option<u64>,
    pub(crate) hook_reserve_ms: Option<u64>,
    pub(crate) post_sync: Option<String>,
    pub(crate) python: Option<String>,
    pub(crate) env: Vec<String>,
}

impl ScheduleSet {
    /// The cron expression.
    #[must_use]
    pub fn cron(&self) -> &str {
        &self.cron
    }
    /// `--budget-ms`.
    #[must_use]
    pub fn budget_ms(&self) -> Option<u64> {
        self.budget_ms
    }
    /// `--hook-reserve-ms`.
    #[must_use]
    pub fn hook_reserve_ms(&self) -> Option<u64> {
        self.hook_reserve_ms
    }
    /// `--post-sync <module:function>`.
    #[must_use]
    pub fn post_sync(&self) -> Option<&str> {
        self.post_sync.as_deref()
    }
    /// `--python <interpreter>`.
    #[must_use]
    pub fn python(&self) -> Option<&str> {
        self.python.as_deref()
    }
    /// `--env NAME`, in order.
    #[must_use]
    pub fn env(&self) -> &[String] {
        &self.env
    }
}

/// `cache schedule show|enable|disable|clear`: no members (REQ-BDC-0720, 0731).
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[non_exhaustive]
pub struct ScheduleNoArgs;

/// `cache cron run [--once] [--shutdown-timeout-ms N]` (REQ-BDC-0730).
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct CronRun {
    pub(crate) once: bool,
    pub(crate) shutdown_timeout_ms: u64,
}

impl CronRun {
    /// The default `--shutdown-timeout-ms`.
    pub const DEFAULT_SHUTDOWN_TIMEOUT_MS: u64 = 10_000;
    /// The largest `--shutdown-timeout-ms`.
    pub const MAX_SHUTDOWN_TIMEOUT_MS: u64 = 600_000;

    /// Run what is due once and exit.
    #[must_use]
    pub fn once(&self) -> bool {
        self.once
    }
    /// How long a stop waits for the runs in flight (0–600000 ms).
    #[must_use]
    pub fn shutdown_timeout_ms(&self) -> u64 {
        self.shutdown_timeout_ms
    }
}

/// `cache sync use-python --python P --module M [...]` (REQ-BDC-0724).
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct SyncUsePython {
    pub(crate) python: String,
    pub(crate) module: String,
    pub(crate) cron: Option<String>,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) env: Vec<String>,
}

impl SyncUsePython {
    /// `--python <absolute interpreter>`.
    #[must_use]
    pub fn python(&self) -> &str {
        &self.python
    }
    /// `--module <module>`.
    #[must_use]
    pub fn module(&self) -> &str {
        &self.module
    }
    /// `--cron <expr>`.
    #[must_use]
    pub fn cron(&self) -> Option<&str> {
        self.cron.as_deref()
    }
    /// `--timeout-ms N`.
    #[must_use]
    pub fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }
    /// `--env NAME`, in order.
    #[must_use]
    pub fn env(&self) -> &[String] {
        &self.env
    }
}
