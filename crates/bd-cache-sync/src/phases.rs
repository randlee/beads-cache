//! The tick's settings and the phase files (REQ-BDC-0701, 0702; ADR-SYNC-0007).
//!
//! A phase is one `phase-<x>.toml` file in the phases directory
//! (`.atm-bd/` by default) holding the phase's `root` bead and its `lead`:
//!
//! ```toml
//! root = "atm-phase-bk"
//! lead = "solar"
//! ```
//!
//! This is the file atm-bd keeps (`.atm-bd/phase-bk.toml` at atm-core
//! `05831df` also has `plan`, `integration_branch` and `[templates]`, which
//! are ignored here). [`TickSettings`] validates the `[tick]` table of
//! `.beads/bd-cache.toml` once, so the tick and the stop rule never see an
//! unchecked value.

use std::fmt;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};

use bd_cache_model::{ScopeId, ScopeName, TickConfig};
use sc_beads::BeadId;
use serde::Deserialize;

use crate::reply::{Refusal, RefusalCode};

/// Where the phase files live when `[tick] phases_dir` is not set.
///
/// A relative directory is relative to the process working directory. [`compose`](crate::compose)
/// resolves it against the repository root instead (REQ-SYNC-0002).
pub const DEFAULT_PHASES_DIR: &str = ".atm-bd";

/// The most bytes of a phase file that are read (a real one is under 1 KiB).
pub const MAX_PHASE_FILE_BYTES: u64 = 64 * 1024;
/// The most phase files read from the directory (atm-core has five).
pub const MAX_PHASE_FILES: usize = 256;
/// The most directory entries of any kind a scan of the phases directory
/// visits (PE-RSH-010): the tick holds the cache while it scans, so a
/// directory full of other files must not make it unbounded. Four times
/// [`MAX_PHASE_FILES`] leaves room for editor backups and notes beside them.
/// With `[tick] phases` set the directory is not scanned at all.
pub const MAX_PHASES_DIR_ENTRIES: usize = 4 * MAX_PHASE_FILES;

/// The member-name rule, in words (the model's, so every message states the same).
pub(crate) use bd_cache_model::MEMBER_NAME_RULE as MEMBER_RULE;

/// `phase-<x>.toml`: the name prefix of a phase file.
const FILE_PREFIX: &str = "phase-";
/// `phase-<x>.toml`: the extension of a phase file.
const FILE_EXTENSION: &str = "toml";

/// A member name cannot be part of an `atm send <lead>@<team>` address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidMember {
    /// The value as written.
    pub value: String,
}

impl fmt::Display for InvalidMember {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} cannot be addressed; a name must be {MEMBER_RULE}",
            self.value
        )
    }
}

impl InvalidMember {
    /// A stable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        "invalid_member"
    }
}

impl std::error::Error for InvalidMember {}

/// A lead's or a team's name, valid in an `atm send` address.
///
/// A name is not empty, has no whitespace, control character or `@`, and does
/// not start with `-`, so it cannot be read as an option or split into two
/// arguments. This is the one validator: settings and phase
/// files use it, and the sender repeats it as a guard.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::MemberName;
/// assert_eq!(MemberName::new("team-lead").unwrap().as_str(), "team-lead");
/// assert!(MemberName::new("-x").is_err());
/// assert!(MemberName::new("a b").is_err());
/// assert!(MemberName::new("a@b").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct MemberName(String);

impl MemberName {
    /// Validates `name` as given (callers trim).
    ///
    /// # Errors
    ///
    /// [`InvalidMember`] for an empty name or one with a character above.
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidMember> {
        let name = name.into();
        let ok = !name.is_empty()
            && !name.starts_with('-')
            && !name
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || c == '@');
        if ok {
            Ok(Self(name))
        } else {
            Err(InvalidMember { value: name })
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MemberName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

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
    /// `phases` is an empty list, which would end the schedule at once.
    EmptyPhases,
}

impl ConfigRefusal {
    /// The refusal code a tick reports.
    #[must_use]
    pub fn code(&self) -> RefusalCode {
        match self {
            Self::MissingTeam => RefusalCode::MissingTeam,
            Self::MissingFallbackLead => RefusalCode::MissingFallbackLead,
            Self::BadMember { .. } => RefusalCode::BadMember,
            Self::MalformedPhases { .. } => RefusalCode::MalformedPhases,
            Self::EmptyPhases => RefusalCode::EmptyPhases,
        }
    }
}

impl fmt::Display for ConfigRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTeam => f.write_str("[tick] team is missing"),
            Self::MissingFallbackLead => f.write_str("[tick] fallback_lead is missing"),
            Self::BadMember { key, value } => {
                write!(
                    f,
                    "[tick] {key} {value:?} cannot be addressed: a name must be {MEMBER_RULE}"
                )
            }
            Self::MalformedPhases { entry } => {
                write!(f, "[tick] phases entry {entry:?} is not a phase name")
            }
            Self::EmptyPhases => f.write_str("[tick] phases is an empty list"),
        }
    }
}

impl std::error::Error for ConfigRefusal {}

impl From<ConfigRefusal> for Refusal {
    fn from(c: ConfigRefusal) -> Self {
        Refusal::new(c.code(), c.to_string())
    }
}

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
/// assert_eq!(settings.team().as_str(), "atm-dev");
/// // A missing team is refused (REQ-BDC-0701).
/// assert!(TickSettings::from_config(&TickConfig::default()).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickSettings {
    team: MemberName,
    fallback_lead: MemberName,
    phases_dir: PathBuf,
    only: Option<Vec<ScopeName>>,
}

impl TickSettings {
    /// Validates the `[tick]` table.
    ///
    /// # Errors
    ///
    /// [`ConfigRefusal`]: a missing team or fallback lead, a name that cannot
    /// be addressed, a `phases` entry that is not a phase name, or an empty
    /// `phases` list.
    pub fn from_config(config: &TickConfig) -> Result<Self, ConfigRefusal> {
        let team = member(config.team.as_deref(), "team", ConfigRefusal::MissingTeam)?;
        let fallback_lead = member(
            config.fallback_lead.as_deref(),
            "fallback_lead",
            ConfigRefusal::MissingFallbackLead,
        )?;
        let only = match &config.phases {
            None => None,
            Some(list) if list.is_empty() => return Err(ConfigRefusal::EmptyPhases),
            Some(list) => {
                let mut names: Vec<ScopeName> = Vec::new();
                for entry in list {
                    let name = bd_cache_model::legacy_scope(entry)
                        .map(|s| s.scope_name().clone())
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
    pub fn team(&self) -> &MemberName {
        &self.team
    }

    /// The lead told of escalations that belong to no phase.
    #[must_use]
    pub fn fallback_lead(&self) -> &MemberName {
        &self.fallback_lead
    }

    /// The directory of the `phase-*.toml` files.
    #[must_use]
    pub fn phases_dir(&self) -> &Path {
        &self.phases_dir
    }

    /// The configured phase list, when it limits the phases.
    #[must_use]
    pub fn only(&self) -> Option<&[ScopeName]> {
        self.only.as_deref()
    }
}

/// The phases directory a repository uses: `dir` (default [`DEFAULT_PHASES_DIR`]),
/// resolved against `repo_root` when relative. The one implementation of the
/// rule; `compose` calls it.
#[must_use]
pub fn rooted_phases_dir(dir: Option<&Path>, repo_root: &Path) -> PathBuf {
    let dir = dir.unwrap_or_else(|| Path::new(DEFAULT_PHASES_DIR));
    if dir.is_relative() {
        repo_root.join(dir)
    } else {
        dir.to_path_buf()
    }
}

/// A non-blank, addressable name.
fn member(
    value: Option<&str>,
    key: &'static str,
    missing: ConfigRefusal,
) -> Result<MemberName, ConfigRefusal> {
    let value = value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or(missing)?;
    MemberName::new(value).map_err(|e| ConfigRefusal::BadMember {
        key,
        value: e.value,
    })
}

/// One phase: a name, its root bead and its lead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase {
    /// The phase name (the `<x>` of `phase-<x>.toml` and of the `phase-<x>` label).
    pub name: ScopeName,
    /// The root bead; the phase is active while it is not closed.
    pub root: BeadId,
    /// Who is told about the phase; `None` when the file names no lead.
    pub lead: Option<MemberName>,
}

impl Phase {
    /// The sync scope of the phase.
    #[must_use]
    pub fn scope(&self) -> ScopeId {
        ScopeId::from_name(self.name.clone())
    }
}

/// Why a phase file, or a configured phase, could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PhaseFileProblem {
    /// The file name after `phase-` is not a phase name.
    NotAScopeName {
        /// The file.
        file: PathBuf,
    },
    /// The file could not be read.
    Unreadable {
        /// The file.
        file: PathBuf,
        /// The I/O error.
        reason: String,
    },
    /// The file is larger than [`MAX_PHASE_FILE_BYTES`].
    TooLarge {
        /// The file.
        file: PathBuf,
        /// Its size.
        bytes: u64,
    },
    /// The file is not TOML, or has no string `root`.
    NotAPhaseFile {
        /// The file.
        file: PathBuf,
        /// What the parser said.
        reason: String,
    },
    /// `root` is not a bead id.
    BadRoot {
        /// The file.
        file: PathBuf,
        /// What was wrong.
        reason: String,
    },
    /// `lead` cannot be addressed.
    BadLead {
        /// The file.
        file: PathBuf,
        /// The value as written.
        lead: String,
    },
    /// A phase named in `[tick] phases` has no file.
    NoFile {
        /// The phase.
        phase: ScopeName,
        /// Where it was looked for.
        dir: PathBuf,
    },
    /// The path is not a regular file (a pipe, a device, a directory).
    NotAFile {
        /// The path.
        file: PathBuf,
    },
    /// The directory holds more than [`MAX_PHASE_FILES`] phase files.
    TooManyFiles {
        /// The directory.
        dir: PathBuf,
    },
    /// The directory holds more than [`MAX_PHASES_DIR_ENTRIES`] entries; the
    /// scan stopped there (PE-RSH-010).
    TooManyEntries {
        /// The directory.
        dir: PathBuf,
    },
}

impl fmt::Display for PhaseFileProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAScopeName { file } => {
                write!(
                    f,
                    "{}: the name after phase- is not a phase name",
                    file.display()
                )
            }
            Self::Unreadable { file, reason } => {
                write!(f, "{}: cannot be read: {reason}", file.display())
            }
            Self::TooLarge { file, bytes } => write!(
                f,
                "{}: {bytes} bytes, more than the {MAX_PHASE_FILE_BYTES} a phase file may have",
                file.display()
            ),
            Self::NotAPhaseFile { file, reason } => {
                write!(f, "{}: not a phase file: {reason}", file.display())
            }
            Self::BadRoot { file, reason } => {
                write!(f, "{}: root is not a bead id: {reason}", file.display())
            }
            Self::BadLead { file, lead } => write!(
                f,
                "{}: lead {lead:?} cannot be addressed (a name must be {MEMBER_RULE})",
                file.display()
            ),
            Self::NotAFile { file } => {
                write!(f, "{}: not a regular file", file.display())
            }
            Self::NoFile { phase, dir } => write!(
                f,
                "configured phase {phase} has no phase-{phase}.toml in {}",
                dir.display()
            ),
            Self::TooManyFiles { dir } => write!(
                f,
                "{} holds more than {MAX_PHASE_FILES} phase files; the rest were not read",
                dir.display()
            ),
            Self::TooManyEntries { dir } => write!(
                f,
                "{} holds more than {MAX_PHASES_DIR_ENTRIES} entries; the rest were not looked at \
                 (list the phases in [tick] phases to read them by name)",
                dir.display()
            ),
        }
    }
}

impl PhaseFileProblem {
    /// The refusal code a tick reports for it.
    #[must_use]
    pub fn code(&self) -> RefusalCode {
        match self {
            Self::NotAScopeName { .. } => RefusalCode::PhaseNotNamed,
            Self::Unreadable { .. } => RefusalCode::PhaseFileUnreadable,
            Self::TooLarge { .. } => RefusalCode::PhaseFileTooLarge,
            Self::NotAFile { .. } => RefusalCode::PhaseFileNotRegular,
            Self::NotAPhaseFile { .. } | Self::BadRoot { .. } | Self::BadLead { .. } => {
                RefusalCode::PhaseFileInvalid
            }
            Self::NoFile { .. } => RefusalCode::PhaseFileMissing,
            Self::TooManyFiles { .. } | Self::TooManyEntries { .. } => {
                RefusalCode::PhaseFilesTooMany
            }
        }
    }

    /// What to do about it; the code's fix, except where the problem knows more.
    #[must_use]
    pub fn fix(&self) -> String {
        match self {
            Self::BadLead { .. } => {
                format!("set lead to a name that is {MEMBER_RULE}, or remove it")
            }
            _ => self.code().fix().to_owned(),
        }
    }
}

impl std::error::Error for PhaseFileProblem {}

impl From<PhaseFileProblem> for Refusal {
    fn from(p: PhaseFileProblem) -> Self {
        Refusal::new(p.code(), p.to_string()).with_fix(p.fix())
    }
}

/// The phases found, and what was wrong with the files that were not usable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhaseSet {
    /// The usable phases, by name.
    pub phases: Vec<Phase>,
    /// What could not be used.
    pub problems: Vec<PhaseFileProblem>,
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
        write!(f, "cannot read the phases directory {}", self.dir.display())
    }
}

impl std::error::Error for PhasesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl PhasesError {
    /// A stable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        RefusalCode::PhasesDirUnreadable.as_str()
    }

    /// The refusal a tick reports for it (the cause is spelled out in the detail).
    #[must_use]
    pub fn refusal(&self) -> Refusal {
        Refusal::new(
            RefusalCode::PhasesDirUnreadable,
            format!("{self}: {}", self.source),
        )
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
/// An unusable file (not TOML, no `root`, a bad name, too large) is listed in
/// [`PhaseSet::problems`] and skipped, so one broken file does not hide the
/// others. A name in `only` with no file is a problem too.
///
/// With `only`, the directory is not scanned: each configured
/// `phase-<name>.toml` is looked up by name, so no cap can hide one
/// (PE-RSH-010). Without it, the scan visits at most
/// [`MAX_PHASES_DIR_ENTRIES`] entries and reads at most [`MAX_PHASE_FILES`]
/// phase files.
///
/// # Errors
///
/// [`PhasesError`] when the directory cannot be read.
pub fn load_phases(dir: &Path, only: Option<&[ScopeName]>) -> Result<PhaseSet, PhasesError> {
    let fail = |source| PhasesError {
        dir: dir.to_path_buf(),
        source,
    };
    // Opened in both cases, so a missing or unreadable directory is the same
    // error whether or not the phases are listed.
    let entries = std::fs::read_dir(dir).map_err(fail)?;
    match only {
        Some(only) => Ok(load_named(dir, only)),
        None => scan(dir, entries).map_err(fail),
    }
}

/// The configured phases, each looked up by its file name.
fn load_named(dir: &Path, only: &[ScopeName]) -> PhaseSet {
    let mut set = PhaseSet::default();
    let mut names: Vec<&ScopeName> = only.iter().collect();
    names.sort();
    names.dedup();
    for name in names {
        let path = dir.join(format!("{FILE_PREFIX}{name}.{FILE_EXTENSION}"));
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                set.problems.push(PhaseFileProblem::NoFile {
                    phase: name.clone(),
                    dir: dir.to_path_buf(),
                });
            }
            // Anything else (a link, a pipe, a permission error) is read as a
            // scanned file would be, and reported by `read_phase`.
            _ => match read_phase(&path, name.clone()) {
                Ok(phase) => set.phases.push(phase),
                Err(problem) => set.problems.push(problem),
            },
        }
    }
    set
}

/// Every `phase-*.toml` file of a directory, within the two caps.
fn scan(dir: &Path, entries: std::fs::ReadDir) -> io::Result<PhaseSet> {
    let mut set = PhaseSet::default();
    let mut files: Vec<(ScopeName, PathBuf)> = Vec::new();
    for (visited, entry) in entries.enumerate() {
        if visited >= MAX_PHASES_DIR_ENTRIES {
            set.problems.push(PhaseFileProblem::TooManyEntries {
                dir: dir.to_path_buf(),
            });
            break;
        }
        let path = entry?.path();
        let Some(stem) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix(FILE_PREFIX))
            .map(str::to_owned)
        else {
            continue;
        };
        if path.extension().is_none_or(|e| e != FILE_EXTENSION) {
            continue;
        }
        if files.len() >= MAX_PHASE_FILES {
            set.problems.push(PhaseFileProblem::TooManyFiles {
                dir: dir.to_path_buf(),
            });
            break;
        }
        match ScopeName::new(stem) {
            Ok(name) => files.push((name, path)),
            Err(_) => set
                .problems
                .push(PhaseFileProblem::NotAScopeName { file: path }),
        }
    }
    files.sort();
    for (name, path) in files {
        match read_phase(&path, name) {
            Ok(phase) => set.phases.push(phase),
            Err(problem) => set.problems.push(problem),
        }
    }
    Ok(set)
}

/// Opens `path` for reading without waiting for a writer (`O_NONBLOCK` on Unix).
fn open_without_blocking(path: &Path) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK);
    }
    options.open(path)
}

/// Parses one phase file.
fn read_phase(path: &Path, name: ScopeName) -> Result<Phase, PhaseFileProblem> {
    let file = || path.to_path_buf();
    let unreadable = |e: io::Error| PhaseFileProblem::Unreadable {
        file: file(),
        reason: e.to_string(),
    };
    // Open first, then look at the handle: a path checked and then opened could
    // be swapped for a pipe in between, and opening a pipe for reading blocks
    // (the tick holds the cache while it reads phase files).
    let mut handle = match open_without_blocking(path) {
        Ok(handle) => handle,
        Err(e) => {
            // Windows refuses to open a directory with "access denied", which
            // would read as `Unreadable`; a path that exists and is not a regular
            // file is `NotAFile` on every platform. Only the error path looks at
            // the path, so the handle-based check above stays the primary one.
            return Err(match std::fs::metadata(path) {
                Ok(m) if !m.is_file() => PhaseFileProblem::NotAFile { file: file() },
                _ => unreadable(e),
            });
        }
    };
    let meta = handle.metadata().map_err(unreadable)?;
    if !meta.is_file() {
        return Err(PhaseFileProblem::NotAFile { file: file() });
    }
    if meta.len() > MAX_PHASE_FILE_BYTES {
        return Err(PhaseFileProblem::TooLarge {
            file: file(),
            bytes: meta.len(),
        });
    }
    // Bounded again while reading: the file may grow after the check.
    let mut text = String::new();
    (&mut handle)
        .take(MAX_PHASE_FILE_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(unreadable)?;
    if text.len() as u64 > MAX_PHASE_FILE_BYTES {
        return Err(PhaseFileProblem::TooLarge {
            file: file(),
            bytes: text.len() as u64,
        });
    }
    let parsed: PhaseFile = toml::from_str(&text).map_err(|e| PhaseFileProblem::NotAPhaseFile {
        file: file(),
        reason: e.message().to_owned(),
    })?;
    let root = BeadId::new(parsed.root.trim()).map_err(|e| PhaseFileProblem::BadRoot {
        file: file(),
        reason: e.to_string(),
    })?;
    let lead = match parsed
        .lead
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        None => None,
        Some(l) => Some(MemberName::new(l).map_err(|e| PhaseFileProblem::BadLead {
            file: file(),
            lead: e.value,
        })?),
    };
    Ok(Phase { name, root, lead })
}
