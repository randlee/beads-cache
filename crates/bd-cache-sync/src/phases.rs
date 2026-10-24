//! The tick's settings and the phase files (REQ-BDC-0701, 0702).
//!
//! A phase is one `phase-<x>.toml` file in the phases directory
//! (`.atm-bd/` by default) holding the phase's `root` bead and its `lead`:
//!
//! ```toml
//! root = "atm-dev-bk"
//! lead = "team-lead"
//! ```
//!
//! Other keys are ignored: atm-bd keeps its own settings in the same files.
//! [`TickSettings`] validates the `[tick]` table of `.beads/bd-cache.toml`
//! once, so the tick and the stop rule never see an unchecked value.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use bd_cache_model::{PhaseName, ScopeId, TickConfig};
use sc_beads::BeadId;
use serde::Deserialize;

/// Where the phase files live when `[tick] phases_dir` is not set.
///
/// A relative directory is relative to the process working directory, which
/// is the repository root: the schedule is started from the beads root
/// (REQ-BDC-0701).
pub const DEFAULT_PHASES_DIR: &str = ".atm-bd";

/// `phase-<x>.toml`: the name prefix of a phase file.
const FILE_PREFIX: &str = "phase-";
/// `phase-<x>.toml`: the extension of a phase file.
const FILE_EXTENSION: &str = "toml";

/// The `[tick]` table cannot run a tick; the tick answers `refused` (REQ-BDC-0701, 0702).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigRefusal {
    /// `team` is missing or blank (REQ-BDC-0701).
    MissingTeam,
    /// `fallback_lead` is missing or blank (REQ-BDC-0701).
    MissingFallbackLead,
    /// `team` or `fallback_lead` cannot be part of an `atm send` address.
    BadMember {
        /// The key.
        key: &'static str,
        /// The value as written.
        value: String,
    },
    /// A `phases` entry is not a phase name (REQ-BDC-0702).
    MalformedPhases {
        /// The entry as written.
        entry: String,
    },
}

impl fmt::Display for ConfigRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTeam => f.write_str(
                "[tick] team is missing; fix: set team to the ATM team of the phase leads in .beads/bd-cache.toml",
            ),
            Self::MissingFallbackLead => f.write_str(
                "[tick] fallback_lead is missing; fix: set fallback_lead to the member who is told of escalations that belong to no phase",
            ),
            Self::BadMember { key, value } => write!(
                f,
                "[tick] {key} {value:?} cannot be addressed; fix: use a name without spaces, '@' or a leading '-'"
            ),
            Self::MalformedPhases { entry } => write!(
                f,
                "[tick] phases entry {entry:?} is not a phase name; fix: use letters, digits, '-', '_' and '.', with or without the phase- prefix"
            ),
        }
    }
}

impl std::error::Error for ConfigRefusal {}

/// The validated `[tick]` table.
///
/// # Examples
///
/// ```
/// use bd_cache_model::TickConfig;
/// use bd_cache_sync::TickSettings;
///
/// let cfg = TickConfig {
///     team: Some("atm-dev".into()),
///     fallback_lead: Some("team-lead".into()),
///     ..TickConfig::default()
/// };
/// let settings = TickSettings::from_config(&cfg).unwrap();
/// assert_eq!(settings.team(), "atm-dev");
/// // A missing team is refused (REQ-BDC-0701).
/// assert!(TickSettings::from_config(&TickConfig::default()).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickSettings {
    team: String,
    fallback_lead: String,
    phases_dir: PathBuf,
    only: Option<Vec<PhaseName>>,
}

impl TickSettings {
    /// Validates the `[tick]` table.
    ///
    /// # Errors
    ///
    /// [`ConfigRefusal`]: a missing team or fallback lead, a name that cannot
    /// be addressed, or a `phases` entry that is not a phase name.
    pub fn from_config(config: &TickConfig) -> Result<Self, ConfigRefusal> {
        let team = member(config.team.as_deref(), "team", ConfigRefusal::MissingTeam)?;
        let fallback_lead = member(
            config.fallback_lead.as_deref(),
            "fallback_lead",
            ConfigRefusal::MissingFallbackLead,
        )?;
        let only = match &config.phases {
            None => None,
            Some(list) => {
                let mut names: Vec<PhaseName> = Vec::new();
                for entry in list {
                    let name = ScopeId::parse(entry)
                        .map(|s| s.phase_name().clone())
                        .ok_or_else(|| ConfigRefusal::MalformedPhases {
                            entry: entry.clone(),
                        })?;
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
                Some(names)
            }
        };
        Ok(Self {
            team,
            fallback_lead,
            phases_dir: config
                .phases_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from(DEFAULT_PHASES_DIR)),
            only,
        })
    }

    /// The ATM team of the phase leads.
    #[must_use]
    pub fn team(&self) -> &str {
        &self.team
    }

    /// The lead told of escalations that belong to no phase.
    #[must_use]
    pub fn fallback_lead(&self) -> &str {
        &self.fallback_lead
    }

    /// The directory of the `phase-*.toml` files.
    #[must_use]
    pub fn phases_dir(&self) -> &Path {
        &self.phases_dir
    }

    /// The configured phase list, when it limits the phases.
    #[must_use]
    pub fn only(&self) -> Option<&[PhaseName]> {
        self.only.as_deref()
    }
}

/// A non-blank name that is safe in an `atm send <lead>@<team>` argument.
fn member(
    value: Option<&str>,
    key: &'static str,
    missing: ConfigRefusal,
) -> Result<String, ConfigRefusal> {
    let value = value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or(missing)?;
    if addressable(value) {
        Ok(value.to_owned())
    } else {
        Err(ConfigRefusal::BadMember {
            key,
            value: value.to_owned(),
        })
    }
}

/// True for a name that cannot be read as an option or split into two arguments.
fn addressable(name: &str) -> bool {
    !name.starts_with('-')
        && !name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '@')
}

/// One phase: a name, its root bead and its lead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase {
    /// The phase name (the `<x>` of `phase-<x>.toml` and of the `phase-<x>` label).
    pub name: PhaseName,
    /// The root bead; the phase is active while it is not closed.
    pub root: BeadId,
    /// Who is told about the phase; `None` when the file names no lead.
    pub lead: Option<String>,
}

impl Phase {
    /// The sync scope of the phase.
    #[must_use]
    pub fn scope(&self) -> ScopeId {
        ScopeId::Phase(self.name.clone())
    }
}

/// The phases found, and what was wrong with the files that were not usable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhaseSet {
    /// The usable phases, by name.
    pub phases: Vec<Phase>,
    /// One line per file or configured phase that could not be used.
    pub problems: Vec<String>,
}

/// The phases directory could not be read.
#[derive(Debug)]
pub struct PhasesError {
    /// The directory.
    pub dir: PathBuf,
    /// Why.
    pub source: io::Error,
}

impl fmt::Display for PhasesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot read the phases directory {}; fix: create it or set [tick] phases_dir",
            self.dir.display()
        )
    }
}

impl std::error::Error for PhasesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// The keys of a phase file this crate reads.
#[derive(Debug, Deserialize)]
struct PhaseFile {
    root: String,
    #[serde(default)]
    lead: Option<String>,
}

/// Reads the `phase-*.toml` files of `dir`, limited to `only` when given.
///
/// An unusable file (not TOML, no `root`, a bad name) is listed in
/// [`PhaseSet::problems`] and skipped, so one broken file does not hide the
/// others. A name in `only` with no file is a problem too.
///
/// # Errors
///
/// [`PhasesError`] when the directory cannot be read.
pub fn load_phases(dir: &Path, only: Option<&[PhaseName]>) -> Result<PhaseSet, PhasesError> {
    let fail = |source| PhasesError {
        dir: dir.to_path_buf(),
        source,
    };
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(fail)? {
        let path = entry.map_err(fail)?.path();
        let wanted = path.extension().is_some_and(|e| e == FILE_EXTENSION)
            && path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with(FILE_PREFIX));
        if wanted {
            files.push(path);
        }
    }
    files.sort();
    let mut set = PhaseSet::default();
    for path in files {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let Ok(name) = PhaseName::new(stem.strip_prefix(FILE_PREFIX).unwrap_or(stem)) else {
            set.problems
                .push(format!("{}: not a phase name", path.display()));
            continue;
        };
        if only.is_some_and(|only| !only.contains(&name)) {
            continue;
        }
        match read_phase(&path, name) {
            Ok(phase) => set.phases.push(phase),
            Err(problem) => set.problems.push(problem),
        }
    }
    for name in only.unwrap_or_default() {
        if !set.phases.iter().any(|p| p.name == *name)
            && !set
                .problems
                .iter()
                .any(|p| p.contains(&format!("phase-{name}.toml")))
        {
            set.problems.push(format!(
                "configured phase {name} has no phase-{name}.toml in {}",
                dir.display()
            ));
        }
    }
    Ok(set)
}

/// Parses one phase file.
fn read_phase(path: &Path, name: PhaseName) -> Result<Phase, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("{}: cannot be read: {e}", path.display()))?;
    let file: PhaseFile = toml::from_str(&text)
        .map_err(|e| format!("{}: not a phase file: {}", path.display(), e.message()))?;
    let root = BeadId::new(file.root.trim())
        .map_err(|e| format!("{}: root is not a bead id: {e}", path.display()))?;
    let lead = match file
        .lead
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        None => None,
        Some(l) if addressable(l) => Some(l.to_owned()),
        Some(l) => {
            return Err(format!(
                "{}: lead {l:?} cannot be addressed (no spaces, '@' or leading '-')",
                path.display()
            ));
        }
    };
    Ok(Phase { name, root, lead })
}
