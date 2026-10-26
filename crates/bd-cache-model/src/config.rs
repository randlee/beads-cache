//! Repository configuration (`.beads/bd-cache.toml`), [`CacheConfig`] and [`DrainBudget`].
//!
//! The file is described in the approved "Repository configuration" section
//! of `docs/architecture.md`. `bd-cache-store` owns the one loader
//! (`load_repo_config`, a-4), which reads the file and parses it with
//! [`RepoConfig::from_toml`]. Unknown keys are an error; a missing file means
//! [`RepoConfig::default`]. `toml` is a private parsing dependency
//! (ADR-MODEL-0004): no toml type appears in this API.
//!
//! ```toml
//! cache_path = ".bd-cache/dev.bd-cache.sqlite3"       # optional
//! scopes = ["bk", "bj"]                               # phase-a scopes, by the 0.1 rule
//! call_timeout_s = 30                                 # optional
//! miss_fill_timeout_ms = 2000                         # optional
//! max_attempts = 5                                    # optional
//! schema_packages = []                                # opaque in phase a
//!
//! [[scope]]                                           # a named bd filter (ADR-BDC-0013)
//! name = "open-bugs"
//! labels_all = ["area"]                               # also labels_any, status, type, assignee
//! status = ["open", "in_progress"]                    # or `all = true` alone
//!
//! [tick]                                              # bd-cache-sync's keys (a-7)
//! team = "dev"
//! fallback_lead = "lead-1"
//! phases_dir = ".phases"
//! phases = ["bk"]
//! ```

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use sc_beads::{Deadline, InvalidValue, ProjectRef};
use serde::Deserialize;

use crate::compat::legacy_scope;
use crate::scope::{FilterParts, Scope, ScopeFilter, ScopeId};

/// The per-call cap on every `bd` call (the BD-CACHE baseline design, Drain bounds; NFR-BDC-0004).
pub const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a cache miss may wait on bd before `BD_UNAVAILABLE` (ADR-BDC-0005).
pub const DEFAULT_MISS_FILL_TIMEOUT: Duration = Duration::from_secs(2);
/// Write attempts on a reachable backend before escalation (BD-CACHE `MAX_ATTEMPTS`).
pub const DEFAULT_MAX_ATTEMPTS: u32 = 5;
/// The drain's start gate sits this far before the tick end (REQ-BDC-0402, 0703).
pub const START_GATE_MARGIN: Duration = Duration::from_secs(5);
/// The drain's hard deadline sits this far before the tick end (REQ-BDC-0402).
pub const HARD_MARGIN: Duration = Duration::from_secs(2);

/// The `.beads/bd-cache.toml` schema.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoConfig {
    /// Cache file; default `.bd-cache/<dolt_database>.bd-cache.sqlite3` (REQ-BDC-0101).
    #[serde(default)]
    pub cache_path: Option<PathBuf>,
    /// Configured phase-a scopes, by the 0.1 rule: each entry `x` or the
    /// legacy prefix followed by `x` names the phase-a scope stored as `x`
    /// (ADR-BDC-0013).
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Configured filter scopes: the `[[scope]]` tables (ADR-BDC-0013).
    #[serde(default, rename = "scope")]
    pub scope_tables: Vec<ScopeTable>,
    /// Per-call cap in seconds; default 30.
    #[serde(default)]
    pub call_timeout_s: Option<u64>,
    /// Miss-fill timeout in milliseconds; default 2000.
    #[serde(default)]
    pub miss_fill_timeout_ms: Option<u64>,
    /// Write attempts before escalation; default 5.
    #[serde(default)]
    pub max_attempts: Option<u32>,
    /// Schema packages to load by path (`docs/architecture.md`, Repository
    /// configuration); opaque and ignored in phase a.
    #[serde(default)]
    pub schema_packages: Vec<SchemaPackageRef>,
    /// bd-cache-sync's keys, read by a-7.
    #[serde(default)]
    pub tick: Option<TickConfig>,
}

/// One `[[scope]]` table of `.beads/bd-cache.toml`: a name and a filter
/// (ADR-BDC-0013). Validated by [`ScopeTable::scope`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeTable {
    /// The scope's name, taken as written.
    pub name: String,
    /// Every bead; refused together with a criterion.
    #[serde(default)]
    pub all: bool,
    /// Labels a bead must all carry.
    #[serde(default)]
    pub labels_all: Vec<String>,
    /// Labels of which a bead must carry one.
    #[serde(default)]
    pub labels_any: Vec<String>,
    /// bd statuses a member must have.
    #[serde(default)]
    pub status: Vec<String>,
    /// The issue type.
    #[serde(default, rename = "type")]
    pub issue_type: Option<String>,
    /// The assignee.
    #[serde(default)]
    pub assignee: Option<String>,
}

impl ScopeTable {
    /// The validated scope this table defines.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for a name that is not a valid scope name, or a filter
    /// [`ScopeFilter::from_parts`] refuses.
    pub fn scope(&self) -> Result<Scope, InvalidValue> {
        let id = ScopeId::new(self.name.clone())?;
        let filter = ScopeFilter::from_parts(FilterParts {
            all: self.all,
            labels_all: self.labels_all.clone(),
            labels_any: self.labels_any.clone(),
            status: self.status.clone(),
            issue_type: self.issue_type.clone(),
            assignee: self.assignee.clone(),
        })?;
        Ok(Scope::new(id, filter))
    }
}

/// A configured scope that is refused: the entry as written and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeConfigError {
    /// The `scopes` entry, or the `[[scope]]` table's name, as written.
    pub value: String,
    /// Why it is refused.
    pub reason: String,
}

/// One `schema_packages` entry, kept opaque in phase a.
///
/// The package format is decided in a later phase (`docs/architecture.md`,
/// Repository configuration); phase a only accepts and keeps the entry.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct SchemaPackageRef(toml::Value);

/// The `[tick]` table: the sync task's keys (bd-cache-sync, a-7).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickConfig {
    /// The team of the phase leads (REQ-BDC-0701).
    #[serde(default)]
    pub team: Option<String>,
    /// The lead told when no phase lead is known.
    #[serde(default)]
    pub fallback_lead: Option<String>,
    /// Where the phase files live.
    #[serde(default)]
    pub phases_dir: Option<PathBuf>,
    /// Limits the synced phases (REQ-BDC-0702).
    #[serde(default)]
    pub phases: Option<Vec<String>>,
}

/// `.beads/bd-cache.toml` did not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// What was wrong, with the position.
    pub detail: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid .beads/bd-cache.toml: {}", self.detail)
    }
}

impl std::error::Error for ConfigError {}

impl RepoConfig {
    /// Parses the text of `.beads/bd-cache.toml`.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] for invalid TOML, an unknown key, a mistyped value, or
    /// a zero `call_timeout_s`, `miss_fill_timeout_ms` or `max_attempts`
    /// ([`RepoConfig::validate`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::{RepoConfig, Scope, ScopeId};
    /// let cfg = RepoConfig::from_toml("scopes = [\"bk\"]\nmax_attempts = 3\n").unwrap();
    /// let cache = cfg.cache_config(sc_beads::ProjectRef::local(None));
    /// assert_eq!(cache.scopes, vec![Scope::legacy(ScopeId::new("bk").unwrap())]);
    /// assert_eq!(cache.max_attempts, 3);
    /// assert!(RepoConfig::from_toml("surprise = 1").is_err());
    /// assert!(RepoConfig::from_toml("max_attempts = 0").is_err());
    /// ```
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(|e| ConfigError {
            detail: e.to_string(),
        })?;
        config.validate()?;
        Ok(config)
    }

    /// Refuses values that parse but cannot work (PE-RSH-006).
    ///
    /// A zero `call_timeout_s` or `miss_fill_timeout_ms` would give every `bd`
    /// call (or every cache-miss fill) a deadline that has already passed, so
    /// bd would never be asked; a zero `max_attempts` would escalate a write
    /// before its first attempt. Each is refused with its own message.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] naming the key and the smallest value it takes.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let zero = |key: &str, least: &str, why: &str| ConfigError {
            detail: format!("{key} = 0 is out of range: it must be at least {least} ({why})"),
        };
        if self.call_timeout_s == Some(0) {
            return Err(zero(
                "call_timeout_s",
                "1 second",
                "a zero cap stops every bd call before it starts",
            ));
        }
        if self.miss_fill_timeout_ms == Some(0) {
            return Err(zero(
                "miss_fill_timeout_ms",
                "1 millisecond",
                "a zero timeout fails every cache miss without asking bd",
            ));
        }
        if self.max_attempts == Some(0) {
            return Err(zero(
                "max_attempts",
                "1 attempt",
                "zero escalates every write before it is tried",
            ));
        }
        Ok(())
    }

    /// Every scope the file defines, validated: the `scopes` entries as
    /// phase-a scopes (by the 0.1 rule), then the `[[scope]]` tables.
    ///
    /// # Errors
    ///
    /// The first [`ScopeConfigError`]: a `scopes` entry or a `[[scope]]` name
    /// that is not a valid scope name, a `[[scope]]` filter that is refused,
    /// or a name defined twice (ADR-STORE-0003 amendment as amended by
    /// ADR-BDC-0013).
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::RepoConfig;
    /// let cfg = RepoConfig::from_toml(
    ///     "scopes = [\"bk\"]\n[[scope]]\nname = \"bugs\"\ntype = \"bug\"\n",
    /// ).unwrap();
    /// let scopes = cfg.configured_scopes().unwrap();
    /// assert!(scopes[0].is_legacy() && !scopes[1].is_legacy());
    /// let twice = RepoConfig::from_toml("scopes = [\"x\"]\n[[scope]]\nname = \"x\"\nall = true\n").unwrap();
    /// assert!(twice.configured_scopes().is_err());
    /// ```
    pub fn configured_scopes(&self) -> Result<Vec<Scope>, ScopeConfigError> {
        fn add(scopes: &mut Vec<Scope>, scope: Scope, value: &str) -> Result<(), ScopeConfigError> {
            if scopes.iter().any(|s| s.id == scope.id) {
                return Err(ScopeConfigError {
                    value: value.to_owned(),
                    reason: format!("scope {} is defined twice", scope.id),
                });
            }
            scopes.push(scope);
            Ok(())
        }
        let mut scopes: Vec<Scope> = Vec::new();
        for entry in &self.scopes {
            let id = legacy_scope(entry).ok_or_else(|| ScopeConfigError {
                value: entry.clone(),
                reason: format!("not a scope name ({})", crate::ScopeName::RULE),
            })?;
            // `x` and the prefixed `x` name one phase-a scope: kept once.
            let scope = Scope::legacy(id);
            if !scopes.contains(&scope) {
                add(&mut scopes, scope, entry)?;
            }
        }
        for table in &self.scope_tables {
            let scope = table.scope().map_err(|e| ScopeConfigError {
                value: table.name.clone(),
                reason: e.to_string(),
            })?;
            add(&mut scopes, scope, &table.name)?;
        }
        Ok(scopes)
    }

    /// The cache configuration for `project`, with defaults for unset keys.
    ///
    /// Invalid scopes are skipped (the loader reports them,
    /// [`RepoConfig::configured_scopes`]); a name is kept once, the first
    /// definition winning.
    #[must_use]
    pub fn cache_config(&self, project: ProjectRef) -> CacheConfig {
        let mut scopes: Vec<Scope> = Vec::new();
        let legacy = self
            .scopes
            .iter()
            .filter_map(|s| legacy_scope(s))
            .map(Scope::legacy);
        let tables = self.scope_tables.iter().filter_map(|t| t.scope().ok());
        for s in legacy.chain(tables) {
            if !scopes.iter().any(|known| known.id == s.id) {
                scopes.push(s);
            }
        }
        CacheConfig {
            project,
            scopes,
            call_timeout: self
                .call_timeout_s
                .map_or(DEFAULT_CALL_TIMEOUT, Duration::from_secs),
            miss_fill_timeout: self
                .miss_fill_timeout_ms
                .map_or(DEFAULT_MISS_FILL_TIMEOUT, Duration::from_millis),
            max_attempts: self.max_attempts.unwrap_or(DEFAULT_MAX_ATTEMPTS),
        }
    }
}

/// The plain values the cache core runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheConfig {
    /// The beads project.
    pub project: ProjectRef,
    /// The scopes `.beads/bd-cache.toml` defines: created or updated in the
    /// registry when the cache opens, and the file wins (ADR-BDC-0013). Syncing
    /// another name registers it on demand (REQ-BDC-0909).
    pub scopes: Vec<Scope>,
    /// The single owner of the 30 s per-call cap.
    pub call_timeout: Duration,
    /// Cache-miss fetch limit, default 2 s (ADR-BDC-0005).
    pub miss_fill_timeout: Duration,
    /// Attempts before escalation, default 5.
    pub max_attempts: u32,
}

impl CacheConfig {
    /// Defaults for `project` with the given scopes.
    #[must_use]
    pub fn new(project: ProjectRef, scopes: Vec<Scope>) -> Self {
        Self {
            project,
            scopes,
            call_timeout: DEFAULT_CALL_TIMEOUT,
            miss_fill_timeout: DEFAULT_MISS_FILL_TIMEOUT,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
        }
    }
}

/// The drain's two deadlines (REQ-BDC-0402).
///
/// Entries not started by `start_gate` are deferred; every bd call is capped
/// at `hard`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrainBudget {
    /// No entry starts after this.
    pub start_gate: Deadline,
    /// No bd call runs past this.
    pub hard: Deadline,
}

impl DrainBudget {
    /// The budget for a tick ending at `end`: start gate 5 s and hard deadline 2 s before it.
    #[must_use]
    pub fn before_tick_end(end: Deadline) -> Self {
        let at = |margin: Duration| {
            Deadline::at(end.instant().checked_sub(margin).unwrap_or(end.instant()))
        };
        Self {
            start_gate: at(START_GATE_MARGIN),
            hard: at(HARD_MARGIN),
        }
    }
}
