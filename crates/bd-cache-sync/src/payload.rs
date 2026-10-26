//! The sync rows' names and payloads, and the interpreter check (ADR-SYNC-0008, REQ-BDC-0720, 0724).
//!
//! Two sc-cron rows, [`BUILTIN_ROW`] and [`PYTHON_ROW`], at most one of each
//! per cache file. Their payload is [`SyncPayload`], one serde enum tagged by
//! `target` and decoded by one constructor, [`SyncPayload::decode`], which
//! validates every value it holds: the budget pair ([`TickBudget`]), the
//! interpreter ([`TrustedInterpreter`]), the module, the timeout and the
//! `--env` names. Every path a payload holds is valid UTF-8 (checked when the
//! value is built), so a payload can always be stored as JSON.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use bd_cache_model::{CacheError, ScheduleReason, TickBudget};
use sc_cron::process::{ChildEnv, ChildSpec, TempCwd, supervise};
use sc_cron::{PythonHook, RunTimeout, ScheduleName};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::text::cut;

/// The name of the built-in sync row (a Rust job).
pub const BUILTIN_ROW: &str = "bd-cache.sync.builtin";

/// The name of the Python sync row (a consumer's script).
pub const PYTHON_ROW: &str = "bd-cache.sync.python";

/// [`BUILTIN_ROW`] as sc-cron's name, validated once.
static BUILTIN_NAME: LazyLock<Result<ScheduleName, sc_cron::CronError>> =
    LazyLock::new(|| ScheduleName::new(BUILTIN_ROW));

/// [`PYTHON_ROW`] as sc-cron's name, validated once.
static PYTHON_NAME: LazyLock<Result<ScheduleName, sc_cron::CronError>> =
    LazyLock::new(|| ScheduleName::new(PYTHON_ROW));

/// One of the two sync rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRow {
    /// [`BUILTIN_ROW`].
    Builtin,
    /// [`PYTHON_ROW`].
    Python,
}

impl SyncRow {
    /// The row's name in the table.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => BUILTIN_ROW,
            Self::Python => PYTHON_ROW,
        }
    }

    /// The sync row named `name`, if it is one.
    #[must_use]
    pub fn of(name: &str) -> Option<Self> {
        match name {
            BUILTIN_ROW => Some(Self::Builtin),
            PYTHON_ROW => Some(Self::Python),
            _ => None,
        }
    }

    /// The other sync row.
    #[must_use]
    pub fn other(self) -> Self {
        match self {
            Self::Builtin => Self::Python,
            Self::Python => Self::Builtin,
        }
    }

    /// The row's name as sc-cron's [`ScheduleName`], built once.
    ///
    /// # Errors
    ///
    /// Never for these two literals (a unit test checks both); a refusal
    /// would be `STORE_FAILED`, never a silently skipped write.
    pub fn name(self) -> Result<ScheduleName, CacheError> {
        let built = match self {
            Self::Builtin => &*BUILTIN_NAME,
            Self::Python => &*PYTHON_NAME,
        };
        built
            .clone()
            .map_err(crate::schedule::schedule_store_failed)
    }
}

/// The Python row's timeout when `--timeout-ms` is not given (REQ-BDC-0724).
pub const DEFAULT_PYTHON_TIMEOUT_MS: u64 = 60_000;

/// The smallest `--timeout-ms` of the Python row (REQ-BDC-0724).
pub const MIN_PYTHON_TIMEOUT_MS: u64 = 1_000;

/// The largest `--timeout-ms` of the Python row (REQ-BDC-0724).
pub const MAX_PYTHON_TIMEOUT_MS: u64 = 600_000;

/// How long `-I -c 'import bd_cache_rs'` may take before the interpreter is
/// refused (REQ-BDC-0724).
pub const IMPORT_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// The most `--env` names a row may carry: far more than a hook needs, and
/// it bounds the payload.
pub const MAX_ENV_NAMES: usize = 64;

/// Variables every child keeps from this process's environment, besides the
/// `LC_*` family and the row's `--env` names (REQ-BDC-0723).
const BASE_ENV: [&str; 3] = ["PATH", "HOME", "LANG"];

/// The option or payload member a `SCHEDULE_INVALID` names (its `field`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    /// `--python`.
    Python,
    /// `--module`.
    Module,
    /// `--post-sync`.
    PostSync,
    /// `--env`.
    Env,
    /// `--timeout-ms`.
    TimeoutMs,
    /// `--budget-ms`.
    BudgetMs,
    /// `--hook-reserve-ms`.
    HookReserveMs,
    /// The cron expression.
    Cron,
    /// The stored payload, or a path in it.
    Payload,
    /// The schedule as a whole (a mode rule).
    Schedule,
}

impl Field {
    /// The `field` text.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Module => "module",
            Self::PostSync => "post_sync",
            Self::Env => "env",
            Self::TimeoutMs => "timeout_ms",
            Self::BudgetMs => "budget_ms",
            Self::HookReserveMs => "hook_reserve_ms",
            Self::Cron => "cron",
            Self::Payload => "payload",
            Self::Schedule => "schedule",
        }
    }
}

/// A `SCHEDULE_INVALID` naming `field`.
pub(crate) fn invalid(
    field: Field,
    reason: ScheduleReason,
    detail: impl Into<String>,
) -> CacheError {
    CacheError::ScheduleInvalid {
        detail: detail.into(),
        field: field.as_str(),
        reason,
    }
}

/// True for a portable identifier: ASCII letters, digits and `_`, not
/// starting with a digit (an `--env` name, a hook's function name).
pub(crate) fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `path` as text, or `SCHEDULE_INVALID` naming `field`: a path that is not
/// valid UTF-8 cannot be stored in a row's JSON payload.
fn utf8(field: Field, what: &str, path: &Path) -> Result<String, CacheError> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        invalid(
            field,
            ScheduleReason::OutOfRange,
            format!(
                "{what} {} is not valid UTF-8 and cannot be stored in the row",
                path.display()
            ),
        )
    })
}

/// An absolute path to a regular executable file, with the module it runs.
///
/// Built only by [`TrustedInterpreter::new`], which then calls
/// [`PythonHook::new`], so the module rule has one parser (ADR-SYNC-0008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedInterpreter {
    hook: PythonHook,
}

impl TrustedInterpreter {
    /// Checks `interpreter` and `module`; `module_field` names the option
    /// that gave the module in a refusal.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` with `field: "python"` and `not_absolute` for a
    /// relative path, `not_executable` for a path that is not a regular
    /// executable file, `out_of_range` for a path that is not valid UTF-8;
    /// with the module's field and `bad_module` for a module that is not a
    /// dotted name.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_sync::TrustedInterpreter;
    /// let e = TrustedInterpreter::new("python3", "pkg.sync").unwrap_err();
    /// assert_eq!(e.code(), "BD_CACHE.SCHEDULE_INVALID");
    /// ```
    pub fn new(interpreter: impl Into<PathBuf>, module: &str) -> Result<Self, CacheError> {
        Self::checked(&interpreter.into(), module, Field::Module)
    }

    fn checked(path: &Path, module: &str, module_field: Field) -> Result<Self, CacheError> {
        if !path.is_absolute() {
            return Err(invalid(
                Field::Python,
                ScheduleReason::NotAbsolute,
                format!("--python {} is not an absolute path", path.display()),
            ));
        }
        utf8(Field::Python, "--python", path)?;
        if !is_executable_file(path) {
            return Err(invalid(
                Field::Python,
                ScheduleReason::NotExecutable,
                format!(
                    "--python {} is not a regular executable file",
                    path.display()
                ),
            ));
        }
        let hook = PythonHook::new(path, module).map_err(|e| {
            invalid(
                module_field,
                ScheduleReason::BadModule,
                format!("{module:?} is not a dotted module name: {e}"),
            )
        })?;
        Ok(Self { hook })
    }

    /// The interpreter.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.hook.interpreter()
    }

    /// The module.
    #[must_use]
    pub fn module(&self) -> &str {
        self.hook.module()
    }

    /// sc-cron's hook for this interpreter and module, with `env`.
    #[must_use]
    pub fn python_hook(&self, env: ChildEnv) -> PythonHook {
        self.hook.clone().with_env(env)
    }

    /// Runs `<interpreter> -I -c 'import bd_cache_rs'` through sc-cron's
    /// supervisor with `env`, killed with its tree after
    /// [`IMPORT_CHECK_TIMEOUT`] (REQ-BDC-0724, ADR-SYNC-0008).
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` (`field: "python"`, `import_failed`) when the
    /// interpreter cannot start, exits non-zero or does not finish in time.
    pub async fn check_import(&self, env: ChildEnv) -> Result<(), CacheError> {
        let failed = |why: String| {
            invalid(
                Field::Python,
                ScheduleReason::ImportFailed,
                format!(
                    "{} cannot import bd_cache_rs within {} s: {why}",
                    self.path().display(),
                    IMPORT_CHECK_TIMEOUT.as_secs()
                ),
            )
        };
        let cwd = TempCwd::new().map_err(|e| failed(format!("no temporary directory: {e}")))?;
        let spec = ChildSpec {
            program: self.path(),
            args: vec!["-I".into(), "-c".into(), "import bd_cache_rs".into()],
            cwd: cwd.path(),
            input: b"",
            env,
        };
        let captured = supervise(&spec, 8 * 1024, tokio::time::sleep(IMPORT_CHECK_TIMEOUT))
            .await
            .map_err(|e| failed(format!("cannot start it: {e}")))?;
        match captured.status {
            Some(status) if status.success() => Ok(()),
            Some(status) => Err(failed(format!(
                "it exited with {status}: {}",
                cut(String::from_utf8_lossy(&captured.stderr.bytes).trim(), 512)
            ))),
            None => Err(failed("it was killed at the timeout".to_owned())),
        }
    }
}

/// A regular file with an execute bit (Unix), or any regular file (Windows).
fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Validated `--env` names: each a portable variable name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvNames(Vec<String>);

impl EnvNames {
    /// Checks each name ([`is_identifier`]); at most [`MAX_ENV_NAMES`];
    /// duplicates dropped.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` (`field: "env"`, `out_of_range`).
    pub fn new(names: impl IntoIterator<Item = String>) -> Result<Self, CacheError> {
        let mut out: Vec<String> = Vec::new();
        for name in names {
            if !is_identifier(&name) {
                return Err(invalid(
                    Field::Env,
                    ScheduleReason::OutOfRange,
                    format!("--env {name:?} is not a variable name"),
                ));
            }
            if !out.contains(&name) {
                out.push(name);
            }
        }
        if out.len() > MAX_ENV_NAMES {
            return Err(invalid(
                Field::Env,
                ScheduleReason::OutOfRange,
                format!("at most {MAX_ENV_NAMES} --env names"),
            ));
        }
        Ok(Self(out))
    }

    /// The names.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.0
    }

    /// The environment of a child: cleared except `PATH`, `HOME`, the
    /// locale variables (`LANG` and every `LC_*` name in this process's
    /// environment at this call) and these names, plus `set`
    /// (REQ-BDC-0723, ADR-SYNC-0008).
    #[must_use]
    pub fn child_env(&self, set: Vec<(std::ffi::OsString, std::ffi::OsString)>) -> ChildEnv {
        let mut keep: Vec<std::ffi::OsString> = BASE_ENV.iter().map(Into::into).collect();
        keep.extend(
            std::env::vars_os()
                .map(|(k, _)| k)
                .filter(|k| k.to_str().is_some_and(|k| k.starts_with("LC_"))),
        );
        keep.extend(self.0.iter().map(Into::into));
        ChildEnv::Only { keep, set }
    }
}

/// The post-sync hook named in the built-in row (REQ-BDC-0723).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostSyncHook {
    interpreter: TrustedInterpreter,
    function: String,
    env: EnvNames,
}

impl PostSyncHook {
    /// Checks `interpreter` and `module:function`.
    ///
    /// # Errors
    ///
    /// As [`TrustedInterpreter::new`] (the module field is `post_sync`), and
    /// `SCHEDULE_INVALID` (`post_sync`, `bad_module`) for a value that is not
    /// `module:function` with an identifier after the colon.
    pub fn new(interpreter: &Path, target: &str, env: EnvNames) -> Result<Self, CacheError> {
        let bad = || {
            invalid(
                Field::PostSync,
                ScheduleReason::BadModule,
                format!("--post-sync {target:?} is not module:function"),
            )
        };
        let (module, function) = target.split_once(':').ok_or_else(bad)?;
        if !is_identifier(function) {
            return Err(bad());
        }
        let interpreter = TrustedInterpreter::checked(interpreter, module, Field::PostSync)?;
        Ok(Self {
            interpreter,
            function: function.to_owned(),
            env,
        })
    }

    /// The interpreter.
    #[must_use]
    pub fn interpreter(&self) -> &TrustedInterpreter {
        &self.interpreter
    }

    /// `module:function`.
    #[must_use]
    pub fn target(&self) -> String {
        format!("{}:{}", self.interpreter.module(), self.function)
    }

    /// The `--env` names.
    #[must_use]
    pub fn env(&self) -> &EnvNames {
        &self.env
    }
}

/// The built-in row's payload: its budget and its optional hook.
///
/// The default is an on-demand tick's with no built-in row: the default
/// budget (`budget_ms` 25000, `hook_reserve_ms` 5000) and no hook.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuiltinPayload {
    /// The budget pair.
    pub budget: TickBudget,
    /// The post-sync hook, when one is named.
    pub hook: Option<PostSyncHook>,
}

/// The Python row's payload: the script and what it is given.
///
/// Its fields are checked once, by [`PythonPayload::new`], and read through
/// the accessors, so a payload out of range cannot be held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonPayload {
    interpreter: TrustedInterpreter,
    cache_path: PathBuf,
    beads_dir: Option<PathBuf>,
    env: EnvNames,
    timeout: RunTimeout,
}

impl PythonPayload {
    /// Checks the timeout (1000–600000 ms) and the paths (UTF-8), and builds the payload.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID`: `timeout_ms` (`out_of_range`) for a timeout out
    /// of range; `payload` (`out_of_range`) for a cache file or beads
    /// directory whose path is not valid UTF-8.
    pub fn new(
        interpreter: TrustedInterpreter,
        cache_path: PathBuf,
        beads_dir: Option<PathBuf>,
        env: EnvNames,
        timeout_ms: Option<u64>,
    ) -> Result<Self, CacheError> {
        let ms = timeout_ms.unwrap_or(DEFAULT_PYTHON_TIMEOUT_MS);
        let out_of_range = || {
            invalid(
                Field::TimeoutMs,
                ScheduleReason::OutOfRange,
                format!(
                    "--timeout-ms {ms} is outside {MIN_PYTHON_TIMEOUT_MS}..={MAX_PYTHON_TIMEOUT_MS}"
                ),
            )
        };
        if !(MIN_PYTHON_TIMEOUT_MS..=MAX_PYTHON_TIMEOUT_MS).contains(&ms) {
            return Err(out_of_range());
        }
        let timeout = RunTimeout::new(Duration::from_millis(ms))
            .map_err(|_in_range_already| out_of_range())?;
        utf8(Field::Payload, "the cache file", &cache_path)?;
        if let Some(dir) = &beads_dir {
            utf8(Field::Payload, "the beads directory", dir)?;
        }
        Ok(Self {
            interpreter,
            cache_path,
            beads_dir,
            env,
            timeout,
        })
    }

    /// The interpreter and the module (`-I -m <module>`).
    #[must_use]
    pub fn interpreter(&self) -> &TrustedInterpreter {
        &self.interpreter
    }

    /// The cache file the row was written for. A run never trusts it: the
    /// trust check and the script's `BD_CACHE_PATH` use the file the worker
    /// actually opened (REQ-BDC-0723).
    #[must_use]
    pub fn cache_path(&self) -> &Path {
        &self.cache_path
    }

    /// The beads directory, when one is set.
    #[must_use]
    pub fn beads_dir(&self) -> Option<&Path> {
        self.beads_dir.as_deref()
    }

    /// The `--env` names.
    #[must_use]
    pub fn env(&self) -> &EnvNames {
        &self.env
    }

    /// The row's timeout (also its sc-cron timeout).
    #[must_use]
    pub fn timeout(&self) -> RunTimeout {
        self.timeout
    }

    /// The `payload` member a run of the script gets: the row's payload with
    /// `cache_path` the file the run opened (`opened`, never the stored
    /// one), plus `run_id` and `repo_root`.
    #[must_use]
    pub fn run_payload(
        &self,
        run_id: &bd_cache_model::RunId,
        opened: &Path,
        repo_root: &Path,
    ) -> Value {
        let mut v = self.to_json();
        if let Value::Object(m) = &mut v {
            m.insert(
                "cache_path".to_owned(),
                Value::String(opened.to_string_lossy().into_owned()),
            );
            m.insert("run_id".to_owned(), Value::String(run_id.to_string()));
            m.insert(
                "repo_root".to_owned(),
                Value::String(repo_root.to_string_lossy().into_owned()),
            );
        }
        v
    }

    fn to_json(&self) -> Value {
        // Every path was checked to be UTF-8 when the payload was built.
        json!({
            "target": "python",
            "interpreter": self.interpreter.path().to_string_lossy(),
            "module": self.interpreter.module(),
            "cache_path": self.cache_path.to_string_lossy(),
            "beads_dir": self.beads_dir.as_ref().map(|d| d.to_string_lossy()),
            "env": self.env.names(),
            "timeout_ms": millis(self.timeout.get()),
        })
    }
}

/// A sync row's payload (ADR-SYNC-0008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncPayload {
    /// The built-in row.
    Builtin(BuiltinPayload),
    /// The Python row.
    Python(PythonPayload),
}

/// The wire form; every value is checked again on decode.
#[derive(Debug, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
enum Wire {
    Builtin {
        budget_ms: u64,
        hook_reserve_ms: u64,
        #[serde(default)]
        hook: Option<WireHook>,
    },
    Python {
        interpreter: PathBuf,
        module: String,
        cache_path: PathBuf,
        #[serde(default)]
        beads_dir: Option<PathBuf>,
        #[serde(default)]
        env: Vec<String>,
        timeout_ms: u64,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireHook {
    interpreter: PathBuf,
    function: String,
    #[serde(default)]
    env: Vec<String>,
}

impl SyncPayload {
    /// Decodes and checks a stored payload; the value is read in place, never copied.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` naming the value at fault, or (`field: "payload"`)
    /// a payload that is not one of the two shapes.
    pub fn decode(v: &Value) -> Result<Self, CacheError> {
        let wire = Wire::deserialize(v).map_err(|e| {
            invalid(
                Field::Payload,
                ScheduleReason::OutOfRange,
                format!("not a sync row payload: {e}"),
            )
        })?;
        Ok(match wire {
            Wire::Builtin {
                budget_ms,
                hook_reserve_ms,
                hook,
            } => {
                let budget =
                    TickBudget::new(Some(budget_ms), Some(hook_reserve_ms), hook.is_some())
                        .map_err(|e| budget_invalid(&e))?;
                let hook = hook
                    .map(|h| PostSyncHook::new(&h.interpreter, &h.function, EnvNames::new(h.env)?))
                    .transpose()?;
                Self::Builtin(BuiltinPayload { budget, hook })
            }
            Wire::Python {
                interpreter,
                module,
                cache_path,
                beads_dir,
                env,
                timeout_ms,
            } => Self::Python(PythonPayload::new(
                TrustedInterpreter::new(interpreter, &module)?,
                cache_path,
                beads_dir,
                EnvNames::new(env)?,
                Some(timeout_ms),
            )?),
        })
    }

    /// The stored JSON. Built from checked values only (every path UTF-8),
    /// so it cannot fail and is never a placeholder.
    #[must_use]
    pub fn to_json(&self) -> Value {
        match self {
            Self::Builtin(b) => {
                let mut v = json!({
                    "target": "builtin",
                    "budget_ms": millis(b.budget.budget()),
                    "hook_reserve_ms": millis(b.budget.hook_reserve()),
                });
                if let (Some(h), Value::Object(m)) = (&b.hook, &mut v) {
                    m.insert(
                        "hook".to_owned(),
                        json!({
                            "interpreter": h.interpreter.path().to_string_lossy(),
                            "function": h.target(),
                            "env": h.env.names(),
                        }),
                    );
                }
                v
            }
            Self::Python(p) => p.to_json(),
        }
    }
}

/// A budget refusal as `SCHEDULE_INVALID` naming the field.
pub(crate) fn budget_invalid(e: &sc_beads::InvalidValue) -> CacheError {
    let field = match e.kind {
        "budget_ms" => Field::BudgetMs,
        _ => Field::HookReserveMs,
    };
    invalid(field, ScheduleReason::OutOfRange, e.to_string())
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    #[test]
    fn both_row_names_are_valid_sc_cron_names() {
        for row in [super::SyncRow::Builtin, super::SyncRow::Python] {
            assert_eq!(
                row.name().ok().map(|n| n.as_str().to_owned()),
                Some(row.as_str().to_owned())
            );
            assert_eq!(super::SyncRow::of(row.as_str()), Some(row));
            assert_eq!(row.other().other(), row);
        }
        assert_eq!(super::SyncRow::of("other"), None);
    }
}

#[cfg(all(test, unix))]
mod unix_tests {
    #![expect(
        clippy::unwrap_used,
        reason = "test code: a failed unwrap is a test failure"
    )]

    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    fn interpreter(dir: &Path) -> PathBuf {
        let path = dir.join("python");
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn a_payload_round_trips_and_every_value_is_checked_on_decode() {
        let dir = tempfile::tempdir().unwrap();
        let python = interpreter(dir.path());
        let hook = PostSyncHook::new(
            &python,
            "pkg.hooks:after",
            EnvNames::new(["TOKEN".to_owned()]).unwrap(),
        )
        .unwrap();
        let builtin = SyncPayload::Builtin(BuiltinPayload {
            budget: TickBudget::new(Some(20_000), Some(4_000), true).unwrap(),
            hook: Some(hook),
        });
        assert_eq!(SyncPayload::decode(&builtin.to_json()).unwrap(), builtin);
        let py = SyncPayload::Python(
            PythonPayload::new(
                TrustedInterpreter::new(&python, "pkg.sync").unwrap(),
                dir.path().join("c.sqlite3"),
                None,
                EnvNames::default(),
                None,
            )
            .unwrap(),
        );
        assert_eq!(SyncPayload::decode(&py.to_json()).unwrap(), py);
        let mut bad = builtin.to_json();
        bad["hook_reserve_ms"] = 500.into();
        assert_eq!(
            SyncPayload::decode(&bad).unwrap_err().code(),
            "BD_CACHE.SCHEDULE_INVALID"
        );
        assert!(SyncPayload::decode(&serde_json::json!({"target": "other"})).is_err());
        assert_eq!(BuiltinPayload::default().budget, TickBudget::default());
    }

    /// A path that is not UTF-8 cannot be held, so a payload always stores
    /// as JSON (B8Q1-023); planted negative: the same file under a UTF-8
    /// name is accepted.
    #[test]
    fn a_path_that_is_not_utf8_is_refused_before_it_can_be_stored() {
        use std::os::unix::ffi::OsStrExt as _;
        let dir = tempfile::tempdir().unwrap();
        let python = interpreter(dir.path());
        let t = TrustedInterpreter::new(&python, "pkg.sync").unwrap();
        let bad = dir
            .path()
            .join(std::ffi::OsStr::from_bytes(b"cache-\xff.sqlite3"));
        let e = PythonPayload::new(t.clone(), bad, None, EnvNames::default(), None).unwrap_err();
        assert!(
            matches!(
                &e,
                CacheError::ScheduleInvalid {
                    field: "payload",
                    ..
                }
            ),
            "{e:?}"
        );
        // Refused before the file is looked at (macOS cannot even create it).
        let odd = dir.path().join(std::ffi::OsStr::from_bytes(b"py-\xff"));
        let e = TrustedInterpreter::new(&odd, "pkg.sync").unwrap_err();
        assert!(
            matches!(
                &e,
                CacheError::ScheduleInvalid {
                    field: "python",
                    ..
                }
            ),
            "{e:?}"
        );
        assert!(
            PythonPayload::new(
                t,
                dir.path().join("c.sqlite3"),
                None,
                EnvNames::default(),
                None
            )
            .is_ok()
        );
    }

    #[test]
    fn the_interpreter_must_be_an_absolute_regular_executable_file() {
        let dir = tempfile::tempdir().unwrap();
        let python = interpreter(dir.path());
        let field = |e: CacheError| match e {
            CacheError::ScheduleInvalid { reason, .. } => reason,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            field(TrustedInterpreter::new("python3", "m").unwrap_err()),
            ScheduleReason::NotAbsolute
        );
        assert_eq!(
            field(TrustedInterpreter::new(dir.path(), "m").unwrap_err()),
            ScheduleReason::NotExecutable
        );
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            field(TrustedInterpreter::new(&python, "m").unwrap_err()),
            ScheduleReason::NotExecutable
        );
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            field(TrustedInterpreter::new(&python, "-c").unwrap_err()),
            ScheduleReason::BadModule
        );
        assert!(TrustedInterpreter::new(&python, "pkg.mod").is_ok());
        for bad in ["pkg", "pkg:", "pkg:1x", ":f"] {
            assert!(
                PostSyncHook::new(&python, bad, EnvNames::default()).is_err(),
                "{bad}"
            );
        }
    }

    #[tokio::test]
    async fn the_import_check_refuses_an_interpreter_that_cannot_import_the_package() {
        let dir = tempfile::tempdir().unwrap();
        let python = dir.path().join("python");
        // A fake interpreter that exits 1 on -c (acceptance 2).
        crate::test_script::write(&python, "#!/bin/sh\n[ \"$2\" = -c ] && exit 1\nexit 0\n");
        let t = TrustedInterpreter::new(&python, "m").unwrap();
        let e = t
            .check_import(EnvNames::default().child_env(vec![]))
            .await
            .unwrap_err();
        assert!(
            matches!(
                e,
                CacheError::ScheduleInvalid {
                    reason: ScheduleReason::ImportFailed,
                    ..
                }
            ),
            "{e:?}"
        );
        // Planted negative: one that exits 0 passes.
        let ok = dir.path().join("python-ok");
        crate::test_script::write(&ok, "#!/bin/sh\nexit 0\n");
        let t = TrustedInterpreter::new(&ok, "m").unwrap();
        assert!(
            t.check_import(EnvNames::default().child_env(vec![]))
                .await
                .is_ok()
        );
    }
}
