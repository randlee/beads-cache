//! The sync rows' names and payloads, and the interpreter check (ADR-SYNC-0008, REQ-BDC-0720, 0724).
//!
//! Two sc-cron rows, [`BUILTIN_ROW`] and [`PYTHON_ROW`], at most one of each
//! per cache file. Their payload is [`SyncPayload`], one serde enum tagged by
//! `target` and decoded by one constructor, [`SyncPayload::decode`], which
//! validates every value it holds: the budget pair ([`TickBudget`]), the
//! interpreter ([`TrustedInterpreter`]), the module and the `--env` names.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bd_cache_model::{CacheError, ScheduleReason, TickBudget};
use sc_cron::PythonHook;
use sc_cron::process::{ChildEnv, ChildSpec, TempCwd, supervise};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The name of the built-in sync row (a Rust job).
pub const BUILTIN_ROW: &str = "bd-cache.sync.builtin";

/// The name of the Python sync row (a consumer's script).
pub const PYTHON_ROW: &str = "bd-cache.sync.python";

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

/// A `SCHEDULE_INVALID` naming `field`.
pub(crate) fn invalid(
    field: &'static str,
    reason: ScheduleReason,
    detail: impl Into<String>,
) -> CacheError {
    CacheError::ScheduleInvalid {
        detail: detail.into(),
        field,
        reason,
    }
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
    /// that gave the module in a refusal (`module`, `post_sync`).
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` with `field: "python"` and `not_absolute` for a
    /// relative path, `not_executable` for a path that is not a regular
    /// executable file; with `field: module_field` and `bad_module` for a
    /// module that is not a dotted name.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_sync::TrustedInterpreter;
    /// let e = TrustedInterpreter::new("python3", "pkg.sync", "module").unwrap_err();
    /// assert_eq!(e.code(), "BD_CACHE.SCHEDULE_INVALID");
    /// ```
    pub fn new(
        interpreter: impl Into<PathBuf>,
        module: &str,
        module_field: &'static str,
    ) -> Result<Self, CacheError> {
        let path = interpreter.into();
        if !path.is_absolute() {
            return Err(invalid(
                "python",
                ScheduleReason::NotAbsolute,
                format!("--python {} is not an absolute path", path.display()),
            ));
        }
        if !is_executable_file(&path) {
            return Err(invalid(
                "python",
                ScheduleReason::NotExecutable,
                format!(
                    "--python {} is not a regular executable file",
                    path.display()
                ),
            ));
        }
        let hook = PythonHook::new(&path, module).map_err(|e| {
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
                "python",
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
                crate::text::cut(String::from_utf8_lossy(&captured.stderr.bytes).trim(), 512)
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct EnvNames(Vec<String>);

impl EnvNames {
    /// Checks each name: ASCII letters, digits and `_`, not starting with a
    /// digit; at most [`MAX_ENV_NAMES`]; duplicates dropped.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` (`field: "env"`, `out_of_range`).
    pub fn new(names: impl IntoIterator<Item = String>) -> Result<Self, CacheError> {
        let mut out: Vec<String> = Vec::new();
        for name in names {
            let mut chars = name.chars();
            let ok = chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !ok {
                return Err(invalid(
                    "env",
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
                "env",
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
                "post_sync",
                ScheduleReason::BadModule,
                format!("--post-sync {target:?} is not module:function"),
            )
        };
        let (module, function) = target.split_once(':').ok_or_else(bad)?;
        let mut chars = function.chars();
        let identifier = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !identifier {
            return Err(bad());
        }
        let interpreter = TrustedInterpreter::new(interpreter, module, "post_sync")?;
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinPayload {
    /// The budget pair.
    pub budget: TickBudget,
    /// The post-sync hook, when one is named.
    pub hook: Option<PostSyncHook>,
}

impl BuiltinPayload {
    /// The payload of an on-demand tick with no built-in row: the default
    /// budget (`budget_ms` 25000, `hook_reserve_ms` 5000) and no hook.
    ///
    /// # Errors
    ///
    /// Never in practice: the defaults are in range; a refusal is
    /// `SCHEDULE_INVALID`.
    pub fn defaults() -> Result<Self, CacheError> {
        Ok(Self {
            budget: TickBudget::new(None, None, false).map_err(|e| budget_invalid(&e))?,
            hook: None,
        })
    }
}

/// The Python row's payload: the script and what it is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonPayload {
    /// The interpreter and the module (`-I -m <module>`).
    pub interpreter: TrustedInterpreter,
    /// The cache file the script opens.
    pub cache_path: PathBuf,
    /// The beads directory, when one is set.
    pub beads_dir: Option<PathBuf>,
    /// The `--env` names.
    pub env: EnvNames,
    /// The row's timeout.
    pub timeout: Duration,
}

impl PythonPayload {
    /// Checks the timeout (1000–600000 ms) and builds the payload.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` (`timeout_ms`, `out_of_range`).
    pub fn new(
        interpreter: TrustedInterpreter,
        cache_path: PathBuf,
        beads_dir: Option<PathBuf>,
        env: EnvNames,
        timeout_ms: Option<u64>,
    ) -> Result<Self, CacheError> {
        let ms = timeout_ms.unwrap_or(DEFAULT_PYTHON_TIMEOUT_MS);
        if !(MIN_PYTHON_TIMEOUT_MS..=MAX_PYTHON_TIMEOUT_MS).contains(&ms) {
            return Err(invalid(
                "timeout_ms",
                ScheduleReason::OutOfRange,
                format!(
                    "--timeout-ms {ms} is outside {MIN_PYTHON_TIMEOUT_MS}..={MAX_PYTHON_TIMEOUT_MS}"
                ),
            ));
        }
        Ok(Self {
            interpreter,
            cache_path,
            beads_dir,
            env,
            timeout: Duration::from_millis(ms),
        })
    }

    /// The `payload` member a run of the script gets: the row's payload plus `run_id`.
    #[must_use]
    pub fn run_payload(&self, run_id: &str) -> Value {
        let mut v = SyncPayload::Python(self.clone()).to_json();
        if let Value::Object(m) = &mut v {
            m.insert("run_id".to_owned(), Value::String(run_id.to_owned()));
        }
        v
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
#[derive(Debug, Serialize, Deserialize)]
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
        // Added to the run's copy of the payload only.
        #[serde(default, skip_serializing)]
        run_id: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireHook {
    interpreter: PathBuf,
    function: String,
    #[serde(default)]
    env: Vec<String>,
}

impl SyncPayload {
    /// Decodes and checks a stored payload.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` naming the value at fault, or (`field: "payload"`)
    /// a payload that is not one of the two shapes.
    pub fn decode(v: &Value) -> Result<Self, CacheError> {
        let wire: Wire = serde_json::from_value(v.clone()).map_err(|e| {
            invalid(
                "payload",
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
                let budget = TickBudget::new(Some(budget_ms), Some(hook_reserve_ms), hook.is_some())
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
                run_id: _,
            } => Self::Python(PythonPayload::new(
                TrustedInterpreter::new(interpreter, &module, "module")?,
                cache_path,
                beads_dir,
                EnvNames::new(env)?,
                Some(timeout_ms),
            )?),
        })
    }

    /// The stored JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let wire = match self {
            Self::Builtin(b) => Wire::Builtin {
                budget_ms: millis(b.budget.budget()),
                hook_reserve_ms: millis(b.budget.hook_reserve()),
                hook: b.hook.as_ref().map(|h| WireHook {
                    interpreter: h.interpreter.path().to_path_buf(),
                    function: h.target(),
                    env: h.env.0.clone(),
                }),
            },
            Self::Python(p) => Wire::Python {
                interpreter: p.interpreter.path().to_path_buf(),
                module: p.interpreter.module().to_owned(),
                cache_path: p.cache_path.clone(),
                beads_dir: p.beads_dir.clone(),
                env: p.env.0.clone(),
                timeout_ms: millis(p.timeout),
                run_id: None,
            },
        };
        serde_json::to_value(wire).unwrap_or(Value::Null)
    }
}

/// A budget refusal as `SCHEDULE_INVALID` naming the field.
pub(crate) fn budget_invalid(e: &sc_beads::InvalidValue) -> CacheError {
    let field = if e.kind == "budget_ms" {
        "budget_ms"
    } else {
        "hook_reserve_ms"
    };
    invalid(field, ScheduleReason::OutOfRange, e.to_string())
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(all(test, unix))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, reason = "test code: a failed unwrap is a test failure")]

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
        let hook = PostSyncHook::new(&python, "pkg.hooks:after", EnvNames::new(["TOKEN".to_owned()]).unwrap()).unwrap();
        let builtin = SyncPayload::Builtin(BuiltinPayload {
            budget: TickBudget::new(Some(20_000), Some(4_000), true).unwrap(),
            hook: Some(hook),
        });
        assert_eq!(SyncPayload::decode(&builtin.to_json()).unwrap(), builtin);
        let py = SyncPayload::Python(
            PythonPayload::new(
                TrustedInterpreter::new(&python, "pkg.sync", "module").unwrap(),
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
        assert_eq!(SyncPayload::decode(&bad).unwrap_err().code(), "BD_CACHE.SCHEDULE_INVALID");
        assert!(SyncPayload::decode(&serde_json::json!({"target": "other"})).is_err());
    }

    #[test]
    fn the_interpreter_must_be_an_absolute_regular_executable_file() {
        let dir = tempfile::tempdir().unwrap();
        let python = interpreter(dir.path());
        let field = |e: CacheError| match e {
            CacheError::ScheduleInvalid { reason, .. } => reason,
            other => panic!("{other:?}"),
        };
        assert_eq!(field(TrustedInterpreter::new("python3", "m", "module").unwrap_err()), ScheduleReason::NotAbsolute);
        assert_eq!(field(TrustedInterpreter::new(dir.path(), "m", "module").unwrap_err()), ScheduleReason::NotExecutable);
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(field(TrustedInterpreter::new(&python, "m", "module").unwrap_err()), ScheduleReason::NotExecutable);
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(field(TrustedInterpreter::new(&python, "-c", "module").unwrap_err()), ScheduleReason::BadModule);
        assert!(TrustedInterpreter::new(&python, "pkg.mod", "module").is_ok());
        for bad in ["pkg", "pkg:", "pkg:1x", ":f"] {
            assert!(PostSyncHook::new(&python, bad, EnvNames::default()).is_err(), "{bad}");
        }
    }

    #[tokio::test]
    async fn the_import_check_refuses_an_interpreter_that_cannot_import_the_package() {
        let dir = tempfile::tempdir().unwrap();
        let python = dir.path().join("python");
        // A fake interpreter that exits 1 on -c (acceptance 2).
        std::fs::write(&python, "#!/bin/sh\n[ \"$2\" = -c ] && exit 1\nexit 0\n").unwrap();
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).unwrap();
        let t = TrustedInterpreter::new(&python, "m", "module").unwrap();
        let e = t.check_import(EnvNames::default().child_env(vec![])).await.unwrap_err();
        assert!(matches!(e, CacheError::ScheduleInvalid { reason: ScheduleReason::ImportFailed, .. }), "{e:?}");
        // Planted negative: one that exits 0 passes.
        std::fs::write(&python, "#!/bin/sh\nexit 0\n").unwrap();
        assert!(t.check_import(EnvNames::default().child_env(vec![])).await.is_ok());
    }
}
