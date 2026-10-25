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
//! call_timeout_s = 30                                 # optional
//! miss_fill_timeout_ms = 2000                         # optional
//! max_attempts = 5                                    # optional
//! schema_packages = []                                # opaque in phase a
//!
//! # Scopes (REQ-BDC-0702): a name and a bd filter each. A plain string is the
//! # scope of the beads carrying that label, named after it.
//! scopes = [
//!   "release",
//!   { name = "open-bugs", status = ["open", "in_progress"], type = "bug" },
//!   { name = "core", labels = ["area"], labels_any = ["p0", "p1"], assignee = "dev-1" },
//!   { name = "everything", all = true },
//! ]
//! ```

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use sc_beads::{Deadline, FilterCriteria, InvalidValue, ProjectRef, ScopeFilter};
use serde::Deserialize;

use crate::scope::{Scope, ScopeId};

/// The per-call cap on every `bd` call (BD-CACHE.md, Drain bounds; NFR-BDC-0004).
pub const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a cache miss may wait on bd before `BD_UNAVAILABLE` (ADR-BDC-0005).
pub const DEFAULT_MISS_FILL_TIMEOUT: Duration = Duration::from_secs(2);
/// Write attempts on a reachable backend before escalation (BD-CACHE `MAX_ATTEMPTS`).
pub const DEFAULT_MAX_ATTEMPTS: u32 = 5;
/// The drain's start gate sits this far before the sync run's end (REQ-BDC-0402, 0703).
pub const START_GATE_MARGIN: Duration = Duration::from_secs(5);
/// The drain's hard deadline sits this far before the sync run's end (REQ-BDC-0402).
pub const HARD_MARGIN: Duration = Duration::from_secs(2);

/// The `.beads/bd-cache.toml` schema.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoConfig {
    /// Cache file; default `.bd-cache/<dolt_database>.bd-cache.sqlite3` (REQ-BDC-0101).
    #[serde(default)]
    pub cache_path: Option<PathBuf>,
    /// Configured scopes (REQ-BDC-0702), in sync order.
    #[serde(default)]
    pub scopes: Vec<ScopeEntry>,
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
}

/// One `schema_packages` entry, kept opaque in phase a.
///
/// The package format is decided in a later phase (`docs/architecture.md`,
/// Repository configuration); phase a only accepts and keeps the entry.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct SchemaPackageRef(toml::Value);

/// One `scopes` entry: a label, or a named filter table (REQ-BDC-0702).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum ScopeEntry {
    /// The beads carrying this label, in a scope named after it.
    Label(String),
    /// A named filter.
    Filter(ScopeTable),
}

impl From<&str> for ScopeEntry {
    fn from(label: &str) -> Self {
        Self::Label(label.to_owned())
    }
}

/// A `scopes` table entry: a name and the bd filter of the scope.
///
/// `all = true` selects every bead (`bd list --all`) and sets no other key;
/// otherwise at least one criterion is set, and a bead must meet every one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeTable {
    /// The scope's name ([`ScopeId`]).
    pub name: String,
    /// Every bead (`--all`).
    #[serde(default)]
    pub all: bool,
    /// Labels a bead carries every one of.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Labels a bead carries at least one of.
    #[serde(default)]
    pub labels_any: Vec<String>,
    /// Statuses a member has one of.
    #[serde(default)]
    pub status: Vec<String>,
    /// The issue type of a member.
    #[serde(default, rename = "type")]
    pub issue_type: Option<String>,
    /// The assignee of a member.
    #[serde(default)]
    pub assignee: Option<String>,
}

/// A `scopes` entry that is not a scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidScope {
    /// The entry's name, or its label for a plain string entry.
    pub entry: String,
    /// What was wrong.
    pub reason: String,
}

impl fmt::Display for InvalidScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scopes entry {:?} {}", self.entry, self.reason)
    }
}

impl std::error::Error for InvalidScope {}

impl ScopeEntry {
    /// The entry's name as written.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Label(l) => l,
            Self::Filter(t) => &t.name,
        }
    }

    /// The scope this entry configures.
    ///
    /// # Errors
    ///
    /// [`InvalidScope`] for a name that is not a [`ScopeId`], a filter value
    /// bd would misread, `all` with another key, or a table with no criterion.
    pub fn scope(&self) -> Result<Scope, InvalidScope> {
        let invalid = |e: InvalidValue| InvalidScope {
            entry: self.name().to_owned(),
            reason: format!("is not a scope: {} {:?} {}", e.kind, e.value, e.reason),
        };
        let id = ScopeId::new(self.name()).map_err(invalid)?;
        let filter = match self {
            Self::Label(_) => return Ok(Scope::label(id)),
            Self::Filter(t) if t.all => {
                let criteria = FilterCriteria {
                    labels: t.labels.clone(),
                    labels_any: t.labels_any.clone(),
                    status: t.status.clone(),
                    issue_type: t.issue_type.clone(),
                    assignee: t.assignee.clone(),
                };
                if criteria != FilterCriteria::default() {
                    return Err(InvalidScope {
                        entry: t.name.clone(),
                        reason: "sets all = true with another key; all selects every bead"
                            .to_owned(),
                    });
                }
                ScopeFilter::All
            }
            Self::Filter(t) => ScopeFilter::Match(FilterCriteria {
                labels: t.labels.clone(),
                labels_any: t.labels_any.clone(),
                status: t.status.clone(),
                issue_type: t.issue_type.clone(),
                assignee: t.assignee.clone(),
            }),
        };
        Scope::new(id, filter).map_err(invalid)
    }
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
    /// [`ConfigError`] for invalid TOML, an unknown key, a mistyped value, a
    /// zero `call_timeout_s`, `miss_fill_timeout_ms` or `max_attempts`
    /// ([`RepoConfig::validate`]), or a `scopes` entry that is not a scope
    /// ([`RepoConfig::scope_list`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::{RepoConfig, Scope, ScopeId};
    /// let text = "scopes = [\"bk\", { name = \"all\", all = true }]\nmax_attempts = 3\n";
    /// let cfg = RepoConfig::from_toml(text).unwrap();
    /// let cache = cfg.cache_config(sc_beads::ProjectRef::local(None));
    /// assert_eq!(cache.scopes[0], Scope::label(ScopeId::new("bk").unwrap()));
    /// assert_eq!(cache.scopes[1].filter(), &sc_beads::ScopeFilter::All);
    /// assert_eq!(cache.max_attempts, 3);
    /// assert!(RepoConfig::from_toml("surprise = 1").is_err());
    /// assert!(RepoConfig::from_toml("max_attempts = 0").is_err());
    /// // A scope is a filter, never a phase; the old tick table is gone.
    /// assert!(RepoConfig::from_toml("[tick]\nphases = [\"x\"]\n").is_err());
    /// assert!(RepoConfig::from_toml("scopes = [{ name = \"x\" }]").is_err());
    /// ```
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let config = Self::parse(text)?;
        config.scope_list().map_err(|e| ConfigError {
            detail: e.to_string(),
        })?;
        Ok(config)
    }

    /// [`RepoConfig::from_toml`] without the `scopes` check, for a loader
    /// that reports a bad scope on its own ([`RepoConfig::scope_list`]).
    ///
    /// # Errors
    ///
    /// As [`RepoConfig::from_toml`], a bad scope entry aside.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
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

    /// The configured scopes, in order.
    ///
    /// # Errors
    ///
    /// [`InvalidScope`] for the first entry that is not a scope, or a name
    /// used twice.
    pub fn scope_list(&self) -> Result<Vec<Scope>, InvalidScope> {
        let mut scopes: Vec<Scope> = Vec::with_capacity(self.scopes.len());
        for entry in &self.scopes {
            let scope = entry.scope()?;
            if scopes.iter().any(|s| s.id() == scope.id()) {
                return Err(InvalidScope {
                    entry: entry.name().to_owned(),
                    reason: "is configured twice".to_owned(),
                });
            }
            scopes.push(scope);
        }
        Ok(scopes)
    }

    /// The cache configuration for `project`, with defaults for unset keys.
    ///
    /// Entries that are not scopes are skipped (the loader and
    /// [`RepoConfig::validate`] refuse them); a name is kept once.
    #[must_use]
    pub fn cache_config(&self, project: ProjectRef) -> CacheConfig {
        let mut scopes: Vec<Scope> = Vec::new();
        for s in self.scopes.iter().filter_map(|e| e.scope().ok()) {
            if !scopes.iter().any(|k| k.id() == s.id()) {
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
    /// Configured scopes, in sync order; syncing another scope id registers
    /// it on demand as its label scope (REQ-BDC-0909).
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
    /// The budget for a sync run ending at `end`: start gate 5 s and hard deadline 2 s before it.
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
