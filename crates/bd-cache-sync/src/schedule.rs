//! The rows of a cache file's cron table (REQ-BDC-0701, ANC-002, ANC-003, ANC-008).
//!
//! The cron table of a cache file is an `sc-cron` store in its own SQLite
//! file beside the cache file, [`cron_db_path`] (`<cache file>.cron.sqlite3`;
//! `sc-cron` owns its file, NFR-BDC-0007, and the cache file gains no table,
//! ADR-STORE-0001). `sc-cron` stays a general scheduler (ANC-005): any row
//! may be in the table. Two kinds of row are run by [`SyncJob`](crate::SyncJob),
//! told apart by their payload's `kind`:
//!
//! - the **built-in sync row**, `{"kind": "bd-cache.sync", "post_sync_hook":
//!   {"interpreter", "module", "timeout_ms"} | null}` ([`RowPayload::BuiltIn`]).
//!   There is at most one per repository: [`register_builtin`] refuses a
//!   second ([`ScheduleError::SecondBuiltIn`]). Its timeout is the run's
//!   budget (REQ-BDC-0703).
//! - a **Python row**, `{"kind": "python", "name", "interpreter", "module", ...}`
//!   ([`RowPayload::Python`]): a consumer's own sync method, run through
//!   `sc-cron`'s Python runner as `<interpreter> -I -m <module>` with the
//!   row's payload on stdin (`cron.tick`). It feeds the cache through
//!   `ingest_scope` (REQ-BDC-0708 in the register amendments, REQ-BDC-0007,
//!   0008, 0502).
//!
//! A consumer that replaces the built-in sync disables the built-in row
//! ([`set_builtin_enabled`]: only the `enabled` column changes, ANC-006) and
//! starts its own Python row ([`python_spec`]).

use std::path::{Path, PathBuf};
use std::time::Duration;

use sc_cron::{
    Clock, CronError, JitterSource, PythonHook, Schedule, ScheduleName, ScheduleSpec,
    ScheduleStore, Scheduler,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::run::DEFAULT_BUDGET;

/// The `kind` of the built-in sync row's payload.
pub const BUILTIN_KIND: &str = "bd-cache.sync";
/// The `kind` of a consumer's Python row's payload.
pub const PYTHON_KIND: &str = "python";
/// The name of the built-in sync row.
pub const BUILTIN_NAME: &str = "bd-cache.sync";
/// The built-in row's default cron expression: every 30 s.
///
/// The interval is the retry interval of everything a run leaves for later
/// (ADR-BDC-0005).
pub const DEFAULT_EXPR: &str = "*/30 * * * * *";
/// The default timeout of a post-sync hook (NFR-BDC-0004).
pub const DEFAULT_HOOK_TIMEOUT: Duration = Duration::from_secs(5);

/// The cron table of the cache file at `cache_path`: `<cache file>.cron.sqlite3`.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// let p = bd_cache_sync::cron_db_path(Path::new("/r/.bd-cache/db.bd-cache.sqlite3"));
/// assert_eq!(p, Path::new("/r/.bd-cache/db.bd-cache.sqlite3.cron.sqlite3"));
/// ```
#[must_use]
pub fn cron_db_path(cache_path: &Path) -> PathBuf {
    let mut name = cache_path.as_os_str().to_owned();
    name.push(".cron.sqlite3");
    PathBuf::from(name)
}

/// A Python module named by a row: the post-sync hook, or a Python row's method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookSpec {
    /// An absolute path, or a bare command name looked up on `PATH`.
    pub interpreter: PathBuf,
    /// A dotted module path (`pkg.sub.mod`).
    pub module: String,
    /// The hook's timeout in milliseconds; it is killed then, or at the
    /// run's deadline when that is sooner (NFR-BDC-0004).
    #[serde(default = "default_hook_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_hook_timeout_ms() -> u64 {
    u64::try_from(DEFAULT_HOOK_TIMEOUT.as_millis()).unwrap_or(u64::MAX)
}

impl HookSpec {
    /// The hook's timeout.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    /// The validated runner (`sc_cron::PythonHook::new`).
    ///
    /// # Errors
    ///
    /// [`CronError::InvalidSpec`] for an interpreter or module `sc-cron` refuses.
    pub fn hook(&self) -> Result<PythonHook, CronError> {
        PythonHook::new(&self.interpreter, &self.module)
    }
}

/// What a row of the cron table is to [`SyncJob`](crate::SyncJob).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowPayload {
    /// The built-in sync, with its optional post-sync hook.
    BuiltIn {
        /// Called after each run with the run's results and errors.
        post_sync_hook: Option<HookSpec>,
    },
    /// A consumer's Python sync method: run as is, the whole payload on stdin.
    Python {
        /// The module and its interpreter.
        hook: PythonHook,
        /// The row's name, as the module is told it (`schedule` on stdin).
        name: ScheduleName,
    },
}

/// The built-in payload as stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuiltInPayload {
    kind: String,
    #[serde(default)]
    post_sync_hook: Option<HookSpec>,
}

impl RowPayload {
    /// Reads a row's payload; `None` for a row of another kind (other rows
    /// of the table are someone else's, ANC-005).
    ///
    /// # Errors
    ///
    /// [`ScheduleError::BadPayload`] for a payload of one of the two kinds
    /// that does not have their shape.
    pub fn of(payload: &Value) -> Result<Option<Self>, ScheduleError> {
        let bad = |why: String| ScheduleError::BadPayload(why);
        match payload.get("kind").and_then(Value::as_str) {
            Some(BUILTIN_KIND) => {
                let p: BuiltInPayload = serde_json::from_value(payload.clone())
                    .map_err(|e| bad(format!("the built-in sync row's payload: {e}")))?;
                if let Some(h) = &p.post_sync_hook {
                    h.hook().map_err(|e| bad(format!("post_sync_hook: {e}")))?;
                }
                Ok(Some(Self::BuiltIn {
                    post_sync_hook: p.post_sync_hook,
                }))
            }
            Some(PYTHON_KIND) => {
                let text = |key: &str| {
                    payload
                        .get(key)
                        .and_then(Value::as_str)
                        .ok_or_else(|| bad(format!("a python row needs a string {key:?}")))
                };
                let hook = PythonHook::new(text("interpreter")?, text("module")?)
                    .map_err(|e| bad(format!("a python row: {e}")))?;
                let name = ScheduleName::new(text("name")?)
                    .map_err(|e| bad(format!("a python row: {e}")))?;
                Ok(Some(Self::Python { hook, name }))
            }
            _ => Ok(None),
        }
    }
}

/// A cron-table operation could not be done.
#[derive(Debug)]
#[non_exhaustive]
pub enum ScheduleError {
    /// A built-in sync row already exists (REQ-BDC-0701, ANC-008).
    SecondBuiltIn {
        /// The name of the row that exists.
        existing: String,
    },
    /// There is no built-in sync row.
    NoBuiltIn,
    /// A row's payload has a kind this crate runs but not its shape.
    BadPayload(String),
    /// The post-sync hook's timeout leaves the sync no time of the row's.
    HookTimeout {
        /// The hook's timeout.
        hook: Duration,
        /// The row's timeout.
        row: Duration,
    },
    /// `sc-cron` refused the operation.
    Cron(CronError),
}

impl ScheduleError {
    /// A stable code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::SecondBuiltIn { .. } => "schedule_second_builtin",
            Self::NoBuiltIn => "schedule_no_builtin",
            Self::BadPayload(_) => "schedule_bad_payload",
            Self::HookTimeout { .. } => "schedule_hook_timeout",
            Self::Cron(_) => "schedule_cron_refused",
        }
    }

    /// What to do about it.
    #[must_use]
    pub fn fix(&self) -> &'static str {
        match self {
            Self::SecondBuiltIn { .. } => {
                "change the existing built-in row (bdc cache schedule set), or clear it first"
            }
            Self::NoBuiltIn => "create it with bdc cache schedule set",
            Self::BadPayload(_) => "rewrite the row's payload in the shape its kind names",
            Self::HookTimeout { .. } => {
                "give the post-sync hook a timeout shorter than the row's timeout"
            }
            Self::Cron(_) => "use a cron expression and timeout sc-cron accepts",
        }
    }
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SecondBuiltIn { existing } => write!(
                f,
                "the repository already has a built-in sync row ({existing}); there is at most one"
            ),
            Self::NoBuiltIn => f.write_str("the repository has no built-in sync row"),
            Self::BadPayload(why) => write!(f, "invalid row payload: {why}"),
            Self::HookTimeout { hook, row } => write!(
                f,
                "the post-sync hook's timeout ({} ms) is not shorter than the row's ({} ms)",
                hook.as_millis(),
                row.as_millis()
            ),
            Self::Cron(_) => f.write_str("sc-cron refused the schedule"),
        }
    }
}

impl std::error::Error for ScheduleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cron(e) => Some(e),
            _ => None,
        }
    }
}

impl From<CronError> for ScheduleError {
    fn from(e: CronError) -> Self {
        Self::Cron(e)
    }
}

/// The built-in sync row's specification: `expr` (default [`DEFAULT_EXPR`]),
/// `timeout` (the run's budget, default [`DEFAULT_BUDGET`]) and the optional
/// post-sync hook.
///
/// # Errors
///
/// [`ScheduleError::HookTimeout`] when the hook's timeout is not shorter than
/// the row's; [`ScheduleError::BadPayload`] for a hook `sc-cron` refuses;
/// [`ScheduleError::Cron`] for an expression or timeout `sc-cron` refuses.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::{BUILTIN_NAME, builtin_spec};
/// let spec = builtin_spec(None, None, None).unwrap();
/// assert_eq!(spec.name.as_str(), BUILTIN_NAME);
/// assert_eq!(spec.payload["kind"], "bd-cache.sync");
/// ```
pub fn builtin_spec(
    expr: Option<&str>,
    timeout: Option<Duration>,
    post_sync_hook: Option<HookSpec>,
) -> Result<ScheduleSpec, ScheduleError> {
    let timeout = timeout.unwrap_or(DEFAULT_BUDGET);
    if let Some(h) = &post_sync_hook {
        if h.timeout() >= timeout || h.timeout().is_zero() {
            return Err(ScheduleError::HookTimeout {
                hook: h.timeout(),
                row: timeout,
            });
        }
        h.hook()
            .map_err(|e| ScheduleError::BadPayload(format!("post_sync_hook: {e}")))?;
    }
    let payload = json!({ "kind": BUILTIN_KIND, "post_sync_hook": post_sync_hook });
    Ok(ScheduleSpec::new(
        BUILTIN_NAME,
        expr.unwrap_or(DEFAULT_EXPR),
        timeout,
        payload,
    )?)
}

/// A consumer's Python row: `name`, `expr`, `timeout`, the module that is its
/// sync method, and the consumer's own `extra` payload keys (ANC-003).
///
/// # Errors
///
/// [`ScheduleError::BadPayload`] for an interpreter or module `sc-cron`
/// refuses, or `extra` that sets `kind`, `name`, `interpreter` or `module`;
/// [`ScheduleError::Cron`] for a spec `sc-cron` refuses.
pub fn python_spec(
    name: &str,
    expr: &str,
    timeout: Duration,
    hook: &HookSpec,
    extra: Map<String, Value>,
) -> Result<ScheduleSpec, ScheduleError> {
    hook.hook()
        .map_err(|e| ScheduleError::BadPayload(format!("a python row: {e}")))?;
    if let Some(k) = ["kind", "name", "interpreter", "module"]
        .into_iter()
        .find(|k| extra.contains_key(*k))
    {
        return Err(ScheduleError::BadPayload(format!(
            "{k:?} is set by the row itself"
        )));
    }
    let mut payload = extra;
    payload.insert("kind".into(), PYTHON_KIND.into());
    payload.insert("name".into(), name.into());
    payload.insert(
        "interpreter".into(),
        hook.interpreter.to_string_lossy().into_owned().into(),
    );
    payload.insert("module".into(), hook.module.clone().into());
    Ok(ScheduleSpec::new(
        name,
        expr,
        timeout,
        Value::Object(payload),
    )?)
}

/// The active built-in sync row of `store`, if any.
///
/// # Errors
///
/// [`ScheduleError::Cron`] when the store cannot be read.
pub fn find_builtin<S: ScheduleStore + ?Sized>(store: &S) -> Result<Option<Schedule>, ScheduleError> {
    Ok(store.list_active()?.into_iter().find(is_builtin))
}

/// True for a row whose payload is the built-in sync's.
fn is_builtin(s: &Schedule) -> bool {
    s.spec.payload.get("kind").and_then(Value::as_str) == Some(BUILTIN_KIND)
}

/// Starts the built-in sync row `spec`, refusing a second one (REQ-BDC-0701,
/// ANC-008). Other rows are never refused here.
///
/// # Errors
///
/// [`ScheduleError::SecondBuiltIn`] when an active built-in row exists;
/// [`ScheduleError::BadPayload`] for a `spec` that is not a built-in row;
/// [`ScheduleError::Cron`] when `sc-cron` refuses the row.
pub fn register_builtin<S, C, J>(
    scheduler: &Scheduler<S, C, J>,
    spec: ScheduleSpec,
) -> Result<Schedule, ScheduleError>
where
    S: ScheduleStore + 'static,
    C: Clock + 'static,
    J: JitterSource + 'static,
{
    if !matches!(RowPayload::of(&spec.payload)?, Some(RowPayload::BuiltIn { .. })) {
        return Err(ScheduleError::BadPayload(
            "the row's payload is not the built-in sync's".into(),
        ));
    }
    if let Some(existing) = find_builtin(scheduler.store().as_ref())? {
        return Err(ScheduleError::SecondBuiltIn {
            existing: existing.spec.name.as_str().to_owned(),
        });
    }
    Ok(scheduler.start(spec)?)
}

/// Creates the built-in row, or replaces the one there is with `spec`; a
/// row that was disabled stays disabled (`bdc cache schedule set`).
///
/// # Errors
///
/// As [`register_builtin`], and [`ScheduleError::Cron`] when the old row
/// cannot be ended.
pub fn set_builtin<S, C, J>(
    scheduler: &Scheduler<S, C, J>,
    spec: ScheduleSpec,
) -> Result<Schedule, ScheduleError>
where
    S: ScheduleStore + 'static,
    C: Clock + 'static,
    J: JitterSource + 'static,
{
    let previous = find_builtin(scheduler.store().as_ref())?;
    let enabled = previous.as_ref().is_none_or(|p| p.enabled);
    if let Some(p) = previous {
        scheduler.stop(p.id, p.generation, "replaced by bdc cache schedule set")?;
    }
    let mut row = register_builtin(scheduler, spec)?;
    if !enabled {
        scheduler.set_enabled(&row.spec.name, false)?;
        row.enabled = false;
    }
    Ok(row)
}

/// Enables or disables the built-in row: only `sc-cron`'s `enabled` column
/// changes (ANC-006). A consumer disables it before enabling its own Python row.
///
/// # Errors
///
/// [`ScheduleError::NoBuiltIn`] when there is none; [`ScheduleError::Cron`].
pub fn set_builtin_enabled<S, C, J>(
    scheduler: &Scheduler<S, C, J>,
    enabled: bool,
) -> Result<Schedule, ScheduleError>
where
    S: ScheduleStore + 'static,
    C: Clock + 'static,
    J: JitterSource + 'static,
{
    let mut row = find_builtin(scheduler.store().as_ref())?.ok_or(ScheduleError::NoBuiltIn)?;
    scheduler.set_enabled(&row.spec.name, enabled)?;
    row.enabled = enabled;
    Ok(row)
}

/// Ends the built-in row (`bdc cache schedule clear`); `None` when there was none.
///
/// # Errors
///
/// [`ScheduleError::Cron`] when it cannot be ended.
pub fn clear_builtin<S, C, J>(
    scheduler: &Scheduler<S, C, J>,
) -> Result<Option<Schedule>, ScheduleError>
where
    S: ScheduleStore + 'static,
    C: Clock + 'static,
    J: JitterSource + 'static,
{
    let Some(row) = find_builtin(scheduler.store().as_ref())? else {
        return Ok(None);
    };
    scheduler.stop(row.id, row.generation, "cleared by bdc cache schedule clear")?;
    Ok(Some(row))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook(timeout_ms: u64) -> HookSpec {
        HookSpec {
            interpreter: "python3".into(),
            module: "consumer.after_sync".into(),
            timeout_ms,
        }
    }

    /// A post-sync hook gets a timeout shorter than the row's, so the sync
    /// keeps part of the budget (REQ-BDC-0703, NFR-BDC-0004).
    #[test]
    fn the_hook_timeout_must_be_shorter_than_the_rows() {
        assert!(builtin_spec(None, Some(Duration::from_secs(25)), Some(hook(5_000))).is_ok());
        for bad in [25_000, 30_000, 0] {
            let e = builtin_spec(None, Some(Duration::from_secs(25)), Some(hook(bad))).unwrap_err();
            assert_eq!(e.code(), "schedule_hook_timeout", "{bad}");
        }
    }

    /// The payload says what a row is; rows of other kinds are not this crate's.
    #[test]
    fn payloads_are_told_apart_by_kind() {
        let spec = builtin_spec(None, None, Some(hook(1_000))).unwrap();
        assert_eq!(
            RowPayload::of(&spec.payload).unwrap(),
            Some(RowPayload::BuiltIn {
                post_sync_hook: Some(hook(1_000))
            })
        );
        let py = python_spec(
            "consumer-sync",
            "*/30 * * * * *",
            Duration::from_secs(20),
            &hook(0),
            Map::from_iter([("repo".to_owned(), json!("/r"))]),
        )
        .unwrap();
        assert_eq!(py.payload["repo"], "/r");
        assert!(matches!(
            RowPayload::of(&py.payload).unwrap(),
            Some(RowPayload::Python { hook, name })
                if hook.module() == "consumer.after_sync" && name.as_str() == "consumer-sync"
        ));
        assert_eq!(RowPayload::of(&json!({"kind": "other"})).unwrap(), None);
        assert_eq!(RowPayload::of(&json!({})).unwrap(), None);
        assert!(RowPayload::of(&json!({"kind": "python", "module": "m"})).is_err());
        assert!(
            RowPayload::of(&json!({"kind": BUILTIN_KIND, "surprise": 1})).is_err(),
            "the built-in payload is strict"
        );
        assert!(
            python_spec(
                "x",
                "*/30 * * * * *",
                Duration::from_secs(20),
                &hook(0),
                Map::from_iter([("kind".to_owned(), json!("bd-cache.sync"))]),
            )
            .is_err(),
            "a python row cannot pose as the built-in row"
        );
    }
}
