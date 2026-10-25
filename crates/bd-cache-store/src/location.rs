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
use std::sync::LazyLock;

use bd_cache_model::{CacheError, ConfigError, PhaseName, RepoConfig, ScopeId, StoreCause};
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
                source: None,
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
                source: None,
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
    /// An I/O or JSON error is kept as the source and its text in the detail
    /// (PE-RBP-F014); a file this process may not read says so, in the detail
    /// and in the fix, instead of the `bd init` advice.
    pub fn read(beads_dir: &Path) -> Result<Self, CacheError> {
        let path = beads_dir.join("metadata.json");
        let detail = |why: &dyn fmt::Display| {
            format!(
                "cannot read dolt_database/project_id from {}: {why}",
                path.display()
            )
        };
        let no_db = |why: String| CacheError::NoDatabase {
            detail: detail(&why),
            source: None,
        };
        let text = std::fs::read_to_string(&path).map_err(|e| {
            let detail = if e.kind() == io::ErrorKind::PermissionDenied {
                format!(
                    "permission denied reading {}: {e}; the file exists, so bd init will not \
                     help: make it readable by this process",
                    path.display()
                )
            } else {
                detail(&e)
            };
            CacheError::NoDatabase {
                detail,
                source: Some(StoreCause::new(e)),
            }
        })?;
        let data: Value = serde_json::from_str(&text).map_err(|e| CacheError::NoDatabase {
            detail: detail(&format_args!("not JSON: {e}")),
            source: Some(StoreCause::new(e)),
        })?;
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

/// A `$BEADS_DIR` value, validated once: never empty and always absolute
/// (lead ruling S49).
///
/// The one place the normalization rule lives (lead rulings S44, S48, S49): an
/// unset or empty value is no override (`None`), and any other value is made
/// absolute with [`std::path::absolute`] against the process's working
/// directory. What that resolves differs by platform (lead ruling S50):
///
/// - on Unix it joins a relative path with the working directory and resolves
///   nothing: no `..`, no symbolic link (it only drops `.` components and
///   repeated separators, as [`Path::components`] does);
/// - on Windows it calls `GetFullPathNameW`, which also resolves `.` and `..`
///   textually, without touching the file system.
///
/// Built once by a composition and handed, unchanged, to the store
/// ([`SqliteStore::open_with`](crate::SqliteStore::open_with),
/// [`CacheLocation::resolve_with`], [`beads_dir`]) and, as
/// [`as_os_str`](Self::as_os_str), to every `bd` child, so the two cannot
/// name different directories.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::ffi::OsStr;
/// use bd_cache_store::BeadsDirOverride;
/// assert_eq!(BeadsDirOverride::new(None)?, None);
/// assert_eq!(BeadsDirOverride::new(Some(OsStr::new("")))?, None);
/// let dir = BeadsDirOverride::new(Some(OsStr::new("rel")))?.ok_or("not empty")?;
/// assert!(dir.as_path().is_absolute() && dir.as_path().ends_with("rel"));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeadsDirOverride(PathBuf);

impl BeadsDirOverride {
    /// Validates an explicit `$BEADS_DIR` value (`None` = unset): `Ok(None)`
    /// for an unset or empty value, else the value made absolute against the
    /// process's working directory.
    ///
    /// # Errors
    ///
    /// [`BeadsDirError`] when the value cannot be made absolute (for a
    /// relative value, the working directory cannot be read); its source is
    /// the I/O error.
    pub fn new(value: Option<&OsStr>) -> Result<Option<Self>, BeadsDirError> {
        Self::new_with(value, |dir| std::path::absolute(dir))
    }

    /// [`new`](Self::new) over the process's own `$BEADS_DIR`.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn from_env() -> Result<Option<Self>, BeadsDirError> {
        Self::new(std::env::var_os(BEADS_DIR_ENV).as_deref())
    }

    /// [`new`](Self::new) with the step that makes a path absolute injected, so
    /// a test can make it fail (lead ruling S48).
    fn new_with(
        value: Option<&OsStr>,
        absolute: impl FnOnce(&Path) -> io::Result<PathBuf>,
    ) -> Result<Option<Self>, BeadsDirError> {
        let Some(value) = value.filter(|v| !v.is_empty()) else {
            return Ok(None);
        };
        let dir = Path::new(value);
        match absolute(dir) {
            Ok(abs) => Ok(Some(Self(abs))),
            Err(source) => Err(BeadsDirError::new(dir, source)),
        }
    }

    /// The absolute directory.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// The absolute directory as the value of a child's `BEADS_DIR`.
    #[must_use]
    pub fn as_os_str(&self) -> &OsStr {
        self.0.as_os_str()
    }
}

/// A `$BEADS_DIR` value could not be made absolute (lead ruling S48): the
/// working directory it is relative to cannot be read.
///
/// `Display` names the value; the I/O error is carried only as
/// [`Error::source`](std::error::Error::source).
#[derive(Debug)]
pub struct BeadsDirError {
    value: PathBuf,
    source: io::Error,
}

impl BeadsDirError {
    /// The error for `value`, which could not be made absolute because of
    /// `source`. Public so a consumer can build one in a test of its own
    /// mapping; [`BeadsDirOverride::new`] is what returns it.
    #[must_use]
    pub fn new(value: impl Into<PathBuf>, source: io::Error) -> Self {
        Self {
            value: value.into(),
            source,
        }
    }

    /// The value as given.
    #[must_use]
    pub fn value(&self) -> &Path {
        &self.value
    }

    /// The value and the I/O error, for a consumer that maps this into an
    /// error of its own.
    #[must_use]
    pub fn into_parts(self) -> (PathBuf, io::Error) {
        (self.value, self.source)
    }
}

impl fmt::Display for BeadsDirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot make BEADS_DIR {} absolute against the working directory",
            self.value.display()
        )
    }
}

impl std::error::Error for BeadsDirError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// The process's own `$BEADS_DIR` for the environment-reading paths
/// ([`CacheLocation::resolve`], [`SqliteStore::open`](crate::SqliteStore::open)):
/// a value that cannot be made absolute is `BD_CACHE.NO_DATABASE`, its detail
/// naming the value and the I/O error (lead ruling S49), before any file is
/// read.
pub(crate) fn env_beads_dir() -> Result<Option<BeadsDirOverride>, CacheError> {
    BeadsDirOverride::from_env().map_err(|e| unresolvable(&e))
}

/// What to do about an unresolvable `$BEADS_DIR` (PE-RSH-043): `NO_DATABASE`'s
/// own fix is about `bd init`, which is not the problem here.
const UNRESOLVABLE_REMEDY: &str =
    "set an absolute BEADS_DIR or run from a readable working directory";

fn unresolvable(e: &BeadsDirError) -> CacheError {
    e.to_cache_error()
}

impl BeadsDirError {
    /// The `BD_CACHE.NO_DATABASE` of this unresolvable `$BEADS_DIR`, as
    /// [`SqliteStore::open`](crate::SqliteStore::open) reports it: the detail
    /// names the value, the I/O error and the remedy. Hosts that read the
    /// variable themselves use this so their error matches the store's.
    ///
    /// The I/O error is kept as text only: as a source, a permission-denied
    /// working directory would select the fix for an unreadable `metadata.json`.
    #[must_use]
    pub fn to_cache_error(&self) -> CacheError {
        CacheError::NoDatabase {
            detail: format!("{self}: {}; {UNRESOLVABLE_REMEDY}", self.source),
            source: None,
        }
    }
}

/// bd's `.beads` directory for a repository: the override when there is one,
/// else `<repo_root>/.beads`.
///
/// Pass [`BeadsDirOverride::from_env`] for the process's own value;
/// [`CacheLocation::resolve`] does.
#[must_use]
pub fn beads_dir(repo_root: &Path, beads_dir: Option<&BeadsDirOverride>) -> PathBuf {
    beads_dir.map_or_else(
        || repo_root.join(".beads"),
        |dir| dir.as_path().to_path_buf(),
    )
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
    /// Resolves the cache file of `repo_root`, reading `$BEADS_DIR` from the
    /// environment ([`BeadsDirOverride::from_env`]).
    ///
    /// # Errors
    ///
    /// `BD_CACHE.NO_DATABASE` as for [`DatabaseIdentity::read`], and for a
    /// `$BEADS_DIR` that cannot be made absolute (the detail names the value
    /// and the I/O error; lead ruling S49).
    pub fn resolve(repo_root: &Path, config: &RepoConfig) -> Result<Self, CacheError> {
        Self::resolve_with(repo_root, config, env_beads_dir()?.as_ref())
    }

    /// Resolves the cache file with an explicit `$BEADS_DIR` override (`None` =
    /// unset); the environment is not read.
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
    /// let dir = tempfile::TempDir::new()?;
    /// let root = dir.path();
    /// std::fs::create_dir_all(root.join(".beads"))?;
    /// std::fs::write(
    ///     root.join(".beads").join("metadata.json"),
    ///     r#"{"dolt_database": "atm-dev", "project_id": "p-1"}"#,
    /// )?;
    /// let loc = CacheLocation::resolve_with(root, &RepoConfig::default(), None)?;
    /// assert!(loc.cache_path.ends_with(std::path::Path::new(".bd-cache").join("atm-dev.bd-cache.sqlite3")));
    /// # Ok(())
    /// # }
    /// ```
    pub fn resolve_with(
        repo_root: &Path,
        config: &RepoConfig,
        beads_dir_override: Option<&BeadsDirOverride>,
    ) -> Result<Self, CacheError> {
        let identity = DatabaseIdentity::read(&beads_dir(repo_root, beads_dir_override))?;
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
///
/// Exhaustive on purpose (lead ruling S38): a consumer matches every variant
/// with no wildcard arm, so a new variant fails to compile in each consumer
/// until it maps it.
#[derive(Debug)]
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

/// The fix of [`ConfigLoadError::Scope`]: it states the phase-name rule in the
/// model's one text ([`PhaseName::RULE`], RBP-F037).
static BAD_SCOPE_FIX: LazyLock<String> = LazyLock::new(|| {
    format!(
        "write each scopes entry of .beads/bd-cache.toml as a phase name ({}), with or \
         without the phase- prefix",
        PhaseName::RULE
    )
});

impl ConfigLoadError {
    /// A stable, store-owned code, one per way the file fails to load
    /// (RBP-F040; ADR-STORE-0003 amendment, lead rulings S34 and S37):
    /// `store_config_unreadable`, `store_config_invalid` (not a valid
    /// configuration: bad TOML, an unknown key, a wrong type) and
    /// `store_config_bad_scope`. These are not [`CacheError`] codes (the
    /// `BD_CACHE.*` namespace is `CacheError`'s) and not CLI-contract codes.
    /// A consumer with codes of its own maps the variants at its boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let dir = tempfile::TempDir::new()?;
    /// let root = dir.path();
    /// std::fs::create_dir_all(root.join(".beads"))?;
    /// std::fs::write(root.join(".beads").join("bd-cache.toml"), "scopes = [\"b k\"]\n")?;
    /// let err = bd_cache_store::load_repo_config(root).unwrap_err();
    /// assert_eq!(err.code(), "store_config_bad_scope");
    /// assert!(err.to_string().contains(err.fix()));
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Read { .. } => "store_config_unreadable",
            Self::Parse { .. } => "store_config_invalid",
            Self::Scope { .. } => "store_config_bad_scope",
        }
    }

    /// What to do about it: one text per way the file fails, the one
    /// [`Display`](fmt::Display) states after `fix:` (RBP-F040).
    #[must_use]
    pub fn fix(&self) -> &'static str {
        match self {
            Self::Read { .. } => {
                "make .beads/bd-cache.toml readable by this process, or remove it to use the defaults"
            }
            Self::Parse { .. } => {
                "correct the key or value of .beads/bd-cache.toml the cause names, or remove unknown keys"
            }
            Self::Scope { .. } => BAD_SCOPE_FIX.as_str(),
        }
    }
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => {
                write!(f, "cannot read {}; fix: {}", path.display(), self.fix())
            }
            Self::Parse { path, .. } => {
                write!(f, "invalid {}; fix: {}", path.display(), self.fix())
            }
            Self::Scope { path, value } => write!(
                f,
                "invalid {}: scopes entry {value:?} is not a phase name; fix: {}",
                path.display(),
                self.fix()
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
/// let dir = tempfile::TempDir::new()?;
/// let root = dir.path();
/// std::fs::create_dir_all(root.join(".beads"))?;
/// assert_eq!(bd_cache_store::load_repo_config(root)?, Default::default());
/// std::fs::write(root.join(".beads").join("bd-cache.toml"), "scopes = [\"bk\"]\n")?;
/// assert_eq!(bd_cache_store::load_repo_config(root)?.scopes, ["bk"]);
/// std::fs::write(root.join(".beads").join("bd-cache.toml"), "surprise = 1\n")?;
/// assert!(bd_cache_store::load_repo_config(root).is_err());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_or_empty_beads_dir_is_no_override() {
        assert_eq!(BeadsDirOverride::new(None).unwrap(), None);
        assert_eq!(BeadsDirOverride::new(Some(OsStr::new(""))).unwrap(), None);
        // Nothing is made absolute for them, so a failing step is never reached.
        let refuse = |_: &Path| -> io::Result<PathBuf> { Err(io::Error::other("unreached")) };
        assert_eq!(BeadsDirOverride::new_with(None, refuse).unwrap(), None);
        assert_eq!(
            BeadsDirOverride::new_with(Some(OsStr::new("")), refuse).unwrap(),
            None
        );
    }

    #[test]
    fn a_relative_beads_dir_is_made_absolute_against_the_working_directory() {
        let cwd = std::env::current_dir().unwrap();
        let dir = BeadsDirOverride::new(Some(OsStr::new("rel")))
            .unwrap()
            .unwrap();
        assert_eq!(dir.as_path(), cwd.join("rel"));
        assert_eq!(dir.as_os_str(), cwd.join("rel").as_os_str());
    }

    #[test]
    fn an_absolute_beads_dir_is_kept() {
        let abs = std::env::temp_dir().join("bd-cache-s49");
        let dir = BeadsDirOverride::new(Some(abs.as_os_str()))
            .unwrap()
            .unwrap();
        assert_eq!(dir.as_path(), abs);
        assert_eq!(beads_dir(Path::new("/repo"), Some(&dir)), abs);
        assert_eq!(
            beads_dir(Path::new("repo"), None),
            Path::new("repo").join(".beads")
        );
    }

    /// The step that makes the value absolute fails (a working directory that
    /// cannot be read): the value is refused with the I/O error as its source,
    /// never kept relative (lead ruling S48).
    #[test]
    fn a_beads_dir_that_cannot_be_made_absolute_is_an_error_with_its_cause() {
        let err = BeadsDirOverride::new_with(Some(OsStr::new("rel")), |_| {
            Err(io::Error::new(io::ErrorKind::NotFound, "getcwd failed"))
        })
        .unwrap_err();
        assert_eq!(err.value(), Path::new("rel"));
        assert!(err.to_string().contains("BEADS_DIR rel"), "{err}");
        assert!(!err.to_string().contains("getcwd"), "{err}");
        let source = std::error::Error::source(&err).unwrap();
        assert_eq!(source.to_string(), "getcwd failed");

        // The environment-reading paths report it as NO_DATABASE, naming both.
        let cache = unresolvable(&err);
        assert_eq!(cache.code(), "BD_CACHE.NO_DATABASE");
        assert!(
            cache.detail().contains("BEADS_DIR rel") && cache.detail().contains("getcwd failed"),
            "{}",
            cache.detail()
        );
        // PE-RSH-043: the detail names the real remedy, not only bd init.
        assert!(
            cache
                .detail()
                .ends_with("; set an absolute BEADS_DIR or run from a readable working directory"),
            "{}",
            cache.detail()
        );

        let (value, source) = err.into_parts();
        assert_eq!(value, Path::new("rel"));
        assert_eq!(source.kind(), io::ErrorKind::NotFound);
    }
}
