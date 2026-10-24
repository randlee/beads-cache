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
//! cache_path = ".bd-cache/atm-dev.bd-cache.sqlite3"   # optional
//! scopes = ["bk", "phase-bj"]                         # phase names, `phase-` optional
//! call_timeout_s = 30                                 # optional
//! miss_fill_timeout_ms = 2000                         # optional
//! max_attempts = 5                                    # optional
//! schema_packages = []                                # opaque in phase a
//!
//! [tick]                                              # bd-cache-sync's keys (a-7)
//! team = "atm-dev"
//! fallback_lead = "team-lead"
//! phases_dir = ".atm-bd"
//! phases = ["bk"]
//! ```

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use sc_beads::{Deadline, ProjectRef};
use serde::Deserialize;

use crate::scope::ScopeId;

/// The per-call cap on every `bd` call (BD-CACHE.md, Drain bounds; NFR-BDC-0004).
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
    /// Configured scopes: phase names, with or without the `phase-` prefix.
    #[serde(default)]
    pub scopes: Vec<String>,
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
    /// The ATM team of the phase leads (REQ-BDC-0701).
    #[serde(default)]
    pub team: Option<String>,
    /// The lead told when no phase lead is known.
    #[serde(default)]
    pub fallback_lead: Option<String>,
    /// Where the `phase-*.toml` phase files live.
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
    /// [`ConfigError`] for invalid TOML, an unknown key or a mistyped value.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::{RepoConfig, ScopeId};
    /// let cfg = RepoConfig::from_toml("scopes = [\"bk\"]\nmax_attempts = 3\n").unwrap();
    /// let cache = cfg.cache_config(sc_beads::ProjectRef::local(None));
    /// assert_eq!(cache.scopes, vec![ScopeId::phase("bk").unwrap()]);
    /// assert_eq!(cache.max_attempts, 3);
    /// assert!(RepoConfig::from_toml("surprise = 1").is_err());
    /// ```
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        toml::from_str(text).map_err(|e| ConfigError {
            detail: e.to_string(),
        })
    }

    /// The cache configuration for `project`, with defaults for unset keys.
    ///
    /// Blank or invalid scope names are skipped (the loader reports them);
    /// duplicates are kept once.
    #[must_use]
    pub fn cache_config(&self, project: ProjectRef) -> CacheConfig {
        let mut scopes: Vec<ScopeId> = Vec::new();
        for s in self.scopes.iter().filter_map(|s| ScopeId::parse(s)) {
            if !scopes.contains(&s) {
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
    /// Configured scopes; syncing another phase registers it on demand (REQ-BDC-0909).
    pub scopes: Vec<ScopeId>,
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
    pub fn new(project: ProjectRef, scopes: Vec<ScopeId>) -> Self {
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
