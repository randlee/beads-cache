//! Where the cache file lives, and the repository configuration (REQ-BDC-0101, 0102).
//!
//! The file's name and the project id come from bd's workspace metadata,
//! `.beads/metadata.json` (or `$BEADS_DIR/metadata.json` when `BEADS_DIR` is
//! set and not empty), so locating the file starts no `bd` process. The
//! location is `cache_path` from `.beads/bd-cache.toml` when set (relative
//! paths are taken from the repository root), else
//! `<repo>/.bd-cache/<dolt_database>.bd-cache.sqlite3`.

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use bd_cache_model::{CacheError, ConfigError, RepoConfig, ScopeId};
use serde_json::Value;

/// The environment variable that moves bd's `.beads` directory.
pub const BEADS_DIR_ENV: &str = "BEADS_DIR";
/// The default cache directory, under the repository root.
pub const DEFAULT_CACHE_DIR: &str = ".bd-cache";
/// The repository configuration file, under the repository root (for display;
/// paths are built with [`Path::join`] per component).
pub const REPO_CONFIG_FILE: &str = ".beads/bd-cache.toml";

fn repo_config_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".beads").join("bd-cache.toml")
}

/// Characters no file name may hold on some supported platform.
const UNPORTABLE_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Windows device names, refused with any extension (`nul.x` is the device too).
const DEVICE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "CONIN$",
    "CONOUT$",
];

/// Why `name` is not a file name on every supported platform, if it is not.
fn unportable(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("empty");
    }
    if name == "." || name == ".." {
        return Some("a relative directory name");
    }
    if name
        .chars()
        .any(|c| UNPORTABLE_CHARS.contains(&c) || c.is_control())
    {
        return Some("holds a path separator, a control character or one of : * ? \" < > |");
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Some("ends with a dot or a space");
    }
    let stem = name.split('.').next().unwrap_or(name);
    if DEVICE_NAMES.iter().any(|d| d.eq_ignore_ascii_case(stem)) {
        return Some("a Windows device name");
    }
    None
}

/// A validated `dolt_database` name: a file name on every supported platform.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DoltDatabase(String);

impl DoltDatabase {
    /// Validates a database name.
    ///
    /// The name becomes part of the cache file name on every platform, so it
    /// may not be empty, `.` or `..`; hold a path separator (`/`, `\`), a
    /// control character or any of `: * ? " < > |`; end with a dot or a space;
    /// or be a Windows device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`,
    /// `LPT1`-`LPT9`, `CONIN$`, `CONOUT$`, any case, with or without an
    /// extension). This is
    /// stricter than `bd_cache.py`, which refuses only `/`.
    ///
    /// # Errors
    ///
    /// `BD_CACHE.NO_DATABASE` naming the value.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_store::DoltDatabase;
    /// assert_eq!(DoltDatabase::new("atm-dev").unwrap().as_str(), "atm-dev");
    /// assert!(DoltDatabase::new("a/b").is_err());
    /// assert!(DoltDatabase::new("").is_err());
    /// assert!(DoltDatabase::new("nul").is_err());
    /// ```
    pub fn new(name: impl Into<String>) -> Result<Self, CacheError> {
        let name = name.into();
        if let Some(why) = unportable(&name) {
            return Err(CacheError::NoDatabase {
                detail: format!("invalid dolt_database {name:?}: {why}"),
            });
        }
        Ok(Self(name))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DoltDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated bd `project_id`: nonempty text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectId(String);

impl ProjectId {
    /// Validates a project id.
    ///
    /// # Errors
    ///
    /// `BD_CACHE.NO_DATABASE` for an empty or whitespace-only id.
    pub fn new(id: impl Into<String>) -> Result<Self, CacheError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(CacheError::NoDatabase {
                detail: format!("invalid project_id {id:?}"),
            });
        }
        Ok(Self(id))
    }

    /// The id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which bd database a cache file belongs to, from `metadata.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseIdentity {
    /// The dolt database: names the cache file.
    pub dolt_database: DoltDatabase,
    /// The project id: recorded in the file and checked on every open (REQ-BDC-0104).
    pub project_id: ProjectId,
}

impl DatabaseIdentity {
    /// Reads `<beads_dir>/metadata.json`.
    ///
    /// # Errors
    ///
    /// `BD_CACHE.NO_DATABASE` when the file cannot be read, is not JSON, or
    /// lacks a valid string `dolt_database` or `project_id` (REQ-BDC-0102).
    pub fn read(beads_dir: &Path) -> Result<Self, CacheError> {
        let path = beads_dir.join("metadata.json");
        let no_db = |why: String| CacheError::NoDatabase {
            detail: format!(
                "cannot read dolt_database/project_id from {}: {why}",
                path.display()
            ),
        };
        let text = std::fs::read_to_string(&path).map_err(|e| no_db(e.to_string()))?;
        let data: Value = serde_json::from_str(&text).map_err(|e| no_db(e.to_string()))?;
        let field = |key: &str| match data.get(key) {
            Some(Value::String(s)) => Ok(s.clone()),
            Some(other) => Err(no_db(format!("{key} is not a string: {other}"))),
            None => Err(no_db(format!("missing key {key}"))),
        };
        let name = field("dolt_database")?;
        let project = field("project_id")?;
        let in_file = |e: CacheError| no_db(e.detail().to_owned());
        Ok(Self {
            dolt_database: DoltDatabase::new(name).map_err(in_file)?,
            project_id: ProjectId::new(project).map_err(in_file)?,
        })
    }
}

/// bd's `.beads` directory for a repository: `beads_dir_env` when it is set
/// and not empty, else `<repo_root>/.beads`.
///
/// Pass `std::env::var_os(BEADS_DIR_ENV).as_deref()`; [`CacheLocation::resolve`]
/// does.
#[must_use]
pub fn beads_dir(repo_root: &Path, beads_dir_env: Option<&OsStr>) -> PathBuf {
    match beads_dir_env {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => repo_root.join(".beads"),
    }
}

/// The resolved cache file of a repository (REQ-BDC-0101).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheLocation {
    /// The identity read from `metadata.json`.
    pub identity: DatabaseIdentity,
    /// The cache file.
    pub cache_path: PathBuf,
}

impl CacheLocation {
    /// Resolves the cache file of `repo_root`, reading `$BEADS_DIR` from the environment.
    ///
    /// # Errors
    ///
    /// `BD_CACHE.NO_DATABASE` as for [`DatabaseIdentity::read`].
    pub fn resolve(repo_root: &Path, config: &RepoConfig) -> Result<Self, CacheError> {
        Self::resolve_with(
            repo_root,
            config,
            std::env::var_os(BEADS_DIR_ENV).as_deref(),
        )
    }

    /// Resolves the cache file with an explicit `$BEADS_DIR` value (`None` = unset).
    ///
    /// # Errors
    ///
    /// `BD_CACHE.NO_DATABASE` as for [`DatabaseIdentity::read`].
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use bd_cache_model::RepoConfig;
    /// use bd_cache_store::CacheLocation;
    /// let root = std::env::temp_dir().join(format!("bdc-doc-loc-{}", std::process::id()));
    /// std::fs::create_dir_all(root.join(".beads"))?;
    /// std::fs::write(
    ///     root.join(".beads").join("metadata.json"),
    ///     r#"{"dolt_database": "atm-dev", "project_id": "p-1"}"#,
    /// )?;
    /// let loc = CacheLocation::resolve_with(&root, &RepoConfig::default(), None)?;
    /// assert!(loc.cache_path.ends_with(std::path::Path::new(".bd-cache").join("atm-dev.bd-cache.sqlite3")));
    /// # std::fs::remove_dir_all(&root)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn resolve_with(
        repo_root: &Path,
        config: &RepoConfig,
        beads_dir_env: Option<&OsStr>,
    ) -> Result<Self, CacheError> {
        let identity = DatabaseIdentity::read(&beads_dir(repo_root, beads_dir_env))?;
        let cache_path = match &config.cache_path {
            Some(p) => repo_root.join(p),
            None => repo_root
                .join(DEFAULT_CACHE_DIR)
                .join(format!("{}.bd-cache.sqlite3", identity.dolt_database)),
        };
        Ok(Self {
            identity,
            cache_path,
        })
    }
}

/// `.beads/bd-cache.toml` could not be loaded.
///
/// `Display` names the file and the fix; the cause (the I/O or parse error)
/// is carried only as [`Error::source`](std::error::Error::source), so a
/// reporter that walks the source chain prints it once.
#[derive(Debug)]
#[non_exhaustive]
pub enum ConfigLoadError {
    /// The file exists but could not be read.
    Read {
        /// The file.
        path: PathBuf,
        /// Why.
        source: io::Error,
    },
    /// The file is not a valid configuration (bad TOML, unknown key, wrong type).
    Parse {
        /// The file.
        path: PathBuf,
        /// Why.
        source: ConfigError,
    },
    /// A `scopes` entry is not a phase name.
    Scope {
        /// The file.
        path: PathBuf,
        /// The entry as written.
        value: String,
    },
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => write!(
                f,
                "cannot read {}; fix: make the file readable, or remove it to use the defaults",
                path.display()
            ),
            Self::Parse { path, .. } => write!(
                f,
                "invalid {}; fix: correct the key or value the cause names, or remove unknown keys",
                path.display()
            ),
            Self::Scope { path, value } => write!(
                f,
                "invalid {}: scopes entry {value:?} is not a phase name; fix: use letters, \
                 digits, '-', '_' and '.', with or without the phase- prefix",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Scope { .. } => None,
        }
    }
}

/// Loads `<repo_root>/.beads/bd-cache.toml` into a [`RepoConfig`]: the one loader.
///
/// A missing file is [`RepoConfig::default`]. Every `scopes` entry must be a
/// phase name (with or without `phase-`); [`RepoConfig::cache_config`] would
/// otherwise skip it silently.
///
/// # Errors
///
/// [`ConfigLoadError`]: an unreadable file, invalid TOML, an unknown key or a
/// mistyped value, or an invalid scope.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let root = std::env::temp_dir().join(format!("bdc-doc-cfg-{}", std::process::id()));
/// std::fs::create_dir_all(root.join(".beads"))?;
/// assert_eq!(bd_cache_store::load_repo_config(&root)?, Default::default());
/// std::fs::write(root.join(".beads").join("bd-cache.toml"), "scopes = [\"bk\"]\n")?;
/// assert_eq!(bd_cache_store::load_repo_config(&root)?.scopes, ["bk"]);
/// std::fs::write(root.join(".beads").join("bd-cache.toml"), "surprise = 1\n")?;
/// assert!(bd_cache_store::load_repo_config(&root).is_err());
/// # std::fs::remove_dir_all(&root)?;
/// # Ok(())
/// # }
/// ```
pub fn load_repo_config(repo_root: &Path) -> Result<RepoConfig, ConfigLoadError> {
    let path = repo_config_path(repo_root);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(RepoConfig::default()),
        Err(source) => return Err(ConfigLoadError::Read { path, source }),
    };
    let config = RepoConfig::from_toml(&text).map_err(|source| ConfigLoadError::Parse {
        path: path.clone(),
        source,
    })?;
    if let Some(bad) = config.scopes.iter().find(|s| ScopeId::parse(s).is_none()) {
        return Err(ConfigLoadError::Scope {
            path,
            value: bad.clone(),
        });
    }
    Ok(config)
}
