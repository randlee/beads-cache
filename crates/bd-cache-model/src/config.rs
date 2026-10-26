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
//! ```
//!
//! The file cannot name code to run (REQ-BDC-0724, ADR-SYNC-0008): the sync
//! rows and the post-sync hook live in the cache file. There is no `[tick]`
//! table; `[tick]`, `[tick.hooks]` or `[tick.sync]` is an unknown key, so the
//! file is refused (`CONFIG_INVALID`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use sc_beads::{Deadline, InvalidValue, ProjectRef};
use serde::Deserialize;

use crate::compat::legacy_scope;
use crate::scope::{FilterParts, Scope, ScopeFilter, ScopeId};

/// The per-call cap on every `bd` call (the v3 drain bounds; NFR-BDC-0004).
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

impl fmt::Display for ScopeConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scope {:?}: {}", self.value, self.reason)
    }
}

impl std::error::Error for ScopeConfigError {}

/// The most scopes `.beads/bd-cache.toml` may define (the `scopes` entries
/// and the `[[scope]]` tables together).
///
/// Every configured scope is written to the registry when the cache opens and
/// synced by each tick; 256 is far past any repository's need and keeps the
/// open and the tick bounded.
pub const MAX_CONFIGURED_SCOPES: usize = 256;

/// The largest `call_timeout_s`: one hour.
///
/// The cap exists so every bd call ends; an hour is far past any list or
/// write bd answers, and a larger value would leave a hung bd holding the
/// project's lock for longer than any tick.
pub const MAX_CALL_TIMEOUT_S: u64 = 3_600;

/// The largest `miss_fill_timeout_ms`: one minute.
///
/// A cache miss blocks a reader; a minute is far past the 2 s default and any
/// one-bead `bd list --id`.
pub const MAX_MISS_FILL_TIMEOUT_MS: u64 = 60_000;

/// The largest `max_attempts`: 100.
///
/// Each attempt is a drain pass over a reachable backend; a write still
/// failing after 100 needs the lead, not another retry.
pub const MAX_MAX_ATTEMPTS: u32 = 100;

/// One `schema_packages` entry, kept opaque in phase a.
///
/// The package format is decided in a later phase (`docs/architecture.md`,
/// Repository configuration); phase a only accepts and keeps the entry.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct SchemaPackageRef(toml::Value);

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
    /// before its first attempt. Each is refused with its own message. Values
    /// over [`MAX_CALL_TIMEOUT_S`], [`MAX_MISS_FILL_TIMEOUT_MS`] or
    /// [`MAX_MAX_ATTEMPTS`] are refused too: they would remove the bound the
    /// key exists for.
    ///
    /// # Errors
    ///
    /// [`ConfigError`] naming the key and the range it takes.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let over = |key: &str, value: u64, most: u64, unit: &str| ConfigError {
            detail: format!(
                "{key} = {value} is out of range: it is at most {most} {unit}, so every bd call \
                 and every write stays bounded"
            ),
        };
        if let Some(t) = self.call_timeout_s.filter(|t| *t > MAX_CALL_TIMEOUT_S) {
            return Err(over("call_timeout_s", t, MAX_CALL_TIMEOUT_S, "seconds"));
        }
        if let Some(t) = self
            .miss_fill_timeout_ms
            .filter(|t| *t > MAX_MISS_FILL_TIMEOUT_MS)
        {
            return Err(over(
                "miss_fill_timeout_ms",
                t,
                MAX_MISS_FILL_TIMEOUT_MS,
                "milliseconds",
            ));
        }
        if let Some(n) = self.max_attempts.filter(|n| *n > MAX_MAX_ATTEMPTS) {
            return Err(over(
                "max_attempts",
                u64::from(n),
                u64::from(MAX_MAX_ATTEMPTS),
                "attempts",
            ));
        }
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

    /// Every scope the file defines, each with the text it was written as:
    /// the `scopes` entries as phase-a scopes (by the 0.1 rule, applied here
    /// once), then the `[[scope]]` tables. Each is validated and held to the
    /// limits of a new scope ([`Scope::check_new`]): the one rule
    /// [`RepoConfig::configured_scopes`] (strict) and
    /// [`RepoConfig::cache_config`] (lenient) both read.
    fn scope_entries(&self) -> impl Iterator<Item = (&str, Result<Scope, String>)> {
        let legacy = self.scopes.iter().map(|entry| {
            let scope = legacy_scope(entry)
                .map(Scope::legacy)
                .ok_or_else(|| format!("not a scope name ({})", crate::ScopeName::RULE));
            (entry.as_str(), scope)
        });
        let tables = self.scope_tables.iter().map(|table| {
            (
                table.name.as_str(),
                table.scope().map_err(|e| e.to_string()),
            )
        });
        legacy.chain(tables).map(|(value, scope)| {
            let checked = scope.and_then(|s| s.check_new().map(|()| s).map_err(|e| e.to_string()));
            (value, checked)
        })
    }

    /// Every scope the file defines, validated: the `scopes` entries as
    /// phase-a scopes (by the 0.1 rule), then the `[[scope]]` tables.
    ///
    /// # Errors
    ///
    /// The first [`ScopeConfigError`]: a `scopes` entry or a `[[scope]]` name
    /// that is not a valid scope name, a scope over the limits of a new scope
    /// ([`Scope::check_new`]), a `[[scope]]` filter that is refused, a name
    /// defined twice (ADR-STORE-0003 amendment as amended by ADR-BDC-0013), or
    /// more than [`MAX_CONFIGURED_SCOPES`] scopes. `x` and the prefixed `x` in
    /// `scopes` name one phase-a scope, kept once.
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
        let refused = |value: &str, reason: String| ScopeConfigError {
            value: value.to_owned(),
            reason,
        };
        let mut seen: BTreeMap<ScopeId, Scope> = BTreeMap::new();
        let mut scopes: Vec<Scope> = Vec::new();
        for (value, scope) in self.scope_entries() {
            let scope = scope.map_err(|reason| refused(value, reason))?;
            match seen.get(&scope.id) {
                Some(known) if *known == scope => continue,
                Some(_) => {
                    return Err(refused(
                        value,
                        format!("scope {} is defined twice", scope.id),
                    ));
                }
                None => {}
            }
            if scopes.len() == MAX_CONFIGURED_SCOPES {
                return Err(refused(
                    value,
                    format!("more than {MAX_CONFIGURED_SCOPES} scopes are configured"),
                ));
            }
            seen.insert(scope.id.clone(), scope.clone());
            scopes.push(scope);
        }
        Ok(scopes)
    }

    /// The cache configuration for `project`, with defaults for unset keys.
    ///
    /// Takes a configuration the loader accepted: `load_repo_config`
    /// (`bd-cache-store`) refuses a file whose [`RepoConfig::configured_scopes`]
    /// fails, so every scope of such a file is kept. Given a configuration
    /// that was not checked, the scopes `configured_scopes` would refuse are
    /// skipped, by the same rule and in the same order, and a name
    /// is kept once, the first definition winning.
    #[must_use]
    pub fn cache_config(&self, project: ProjectRef) -> CacheConfig {
        let mut seen: BTreeSet<ScopeId> = BTreeSet::new();
        let scopes = self
            .scope_entries()
            .filter_map(|(_, scope)| scope.ok())
            .filter(|s| seen.insert(s.id.clone()))
            .take(MAX_CONFIGURED_SCOPES)
            .collect();
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
