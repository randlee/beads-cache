//! The sync mode: the built-in and Python rows, their flags and the switch
//! (REQ-BDC-0720, 0722, 0724, 0731; ADR-SYNC-0008).
//!
//! Every write of a row or of the rows' flags is one sc-cron store call in
//! one `BEGIN IMMEDIATE` transaction (`replace_and_set_enabled` or
//! `set_enabled_many`, REQ-CRON-0017), so no connection ever sees both rows
//! enabled, not even while a row is created:
//!
//! - [`schedule_set`] writes the built-in row: an existing row keeps its
//!   `enabled` flag; a new row is enabled unless the Python row is enabled
//!   (then it is written disabled); a row written enabled disables the Python
//!   row in the same call. An active row is replaced in place (generation + 1).
//! - [`sync_use_python`] checks its inputs, takes the lease, writes the one
//!   Python row enabled and the built-in row disabled in one call, runs the
//!   row at once under that lease, and on any failure restores, in one call,
//!   the state held before the command.
//! - [`sync_use_builtin`] takes no lease and enables the built-in row and
//!   disables the Python row in one call; it always works when a built-in
//!   row exists, which makes it the recovery.
//! - [`schedule_enable`] of the built-in row while the Python row is enabled
//!   is `SCHEDULE_INVALID` (`python_sync_enabled`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bd_cache_model::{CacheError, CacheStore, DrainLock, ScheduleReason, TickBudget};
use sc_beads::CacheBackend;
use sc_cron::{
    Clock as CronClock, CronError, JitterSource, RunOutcome, Schedule, ScheduleName, ScheduleSpec,
    ScheduleStore, Scheduler,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::history::RowHistory;
use crate::job::SyncContext;
use crate::lease::{LeaseTaken, TickLease, new_run_id};
use crate::payload::{
    BUILTIN_ROW, BuiltinPayload, EnvNames, PYTHON_ROW, PostSyncHook, PythonPayload, SyncPayload,
    TrustedInterpreter, budget_invalid, invalid,
};
use crate::python_job::PythonSyncJob;
use crate::run::{BuiltinReply, RunContext, SharedCache, blocking, hold, holder_json, run_builtin};

/// The sync mode (REQ-BDC-0731).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    /// The built-in row is enabled.
    Builtin,
    /// The Python row is enabled.
    Python,
    /// Neither is.
    None,
}

/// An error of the schedule, switch and tick operations.
#[derive(Debug)]
#[non_exhaustive]
pub enum SyncError {
    /// A typed cache error: `SCHEDULE_INVALID`, `TICK_RUN_FAILED`,
    /// `TICK_IN_FLIGHT`, `STORE_FAILED` (a schedule table that cannot be
    /// read, ADR-BDC-0014) and the store's other codes.
    Cache(CacheError),
    /// `PRECONDITION` with `details.reason: "python_sync_enabled"`: an
    /// on-demand `tick` while the Python row is enabled (REQ-BDC-0720).
    PythonSyncEnabled(String),
}

impl SyncError {
    /// The `BD_CACHE.*` code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Cache(e) => e.code(),
            Self::PythonSyncEnabled(_) => "BD_CACHE.PRECONDITION",
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cache(e) => std::fmt::Display::fmt(e, f),
            Self::PythonSyncEnabled(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for SyncError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cache(e) => Some(e),
            Self::PythonSyncEnabled(_) => None,
        }
    }
}

impl From<CacheError> for SyncError {
    fn from(e: CacheError) -> Self {
        Self::Cache(e)
    }
}

/// A schedule-table failure as `STORE_FAILED` (ADR-BDC-0014).
pub(crate) fn schedule_store_failed(e: CronError) -> CacheError {
    CacheError::store_failed(format!("schedule store (sc_cron_schedule): {e}"), e)
}

/// The two rows, as the table holds them now.
#[derive(Debug, Clone, Default)]
pub struct Rows {
    /// The active built-in row.
    pub builtin: Option<Schedule>,
    /// The active Python row.
    pub python: Option<Schedule>,
}

impl Rows {
    /// Reads the active rows.
    ///
    /// # Errors
    ///
    /// `STORE_FAILED` when the table cannot be read.
    pub fn read<St: ScheduleStore + ?Sized>(store: &St) -> Result<Self, CacheError> {
        let mut rows = Self::default();
        for row in store.list_active().map_err(schedule_store_failed)? {
            match row.spec.name.as_str() {
                BUILTIN_ROW => rows.builtin = Some(row),
                PYTHON_ROW => rows.python = Some(row),
                _ => {}
            }
        }
        Ok(rows)
    }

    /// The sync mode the flags give.
    #[must_use]
    pub fn mode(&self) -> SyncMode {
        if self.python.as_ref().is_some_and(|r| r.enabled) {
            SyncMode::Python
        } else if self.builtin.as_ref().is_some_and(|r| r.enabled) {
            SyncMode::Builtin
        } else {
            SyncMode::None
        }
    }
}

fn name(row: &str) -> Result<ScheduleName, CacheError> {
    ScheduleName::new(row).map_err(schedule_store_failed)
}

fn spec(
    row: &str,
    expr: &str,
    timeout: Duration,
    payload: Value,
) -> Result<ScheduleSpec, CacheError> {
    ScheduleSpec::new(row, expr, timeout, payload).map_err(|e| match e {
        CronError::BadExpression(why) => invalid(
            "cron",
            ScheduleReason::OutOfRange,
            format!("{expr:?} is not a cron expression: {why}"),
        ),
        other => invalid("payload", ScheduleReason::OutOfRange, other.to_string()),
    })
}

/// `schedule set`'s inputs (REQ-BDC-0720).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScheduleSet {
    /// The cron expression.
    pub expr: String,
    /// `--budget-ms` (default 25000).
    pub budget_ms: Option<u64>,
    /// `--hook-reserve-ms` (default 5000).
    pub hook_reserve_ms: Option<u64>,
    /// `--post-sync <module:function>`.
    pub post_sync: Option<String>,
    /// `--python <interpreter>` for the hook.
    pub python: Option<PathBuf>,
    /// `--env NAME`, repeatable.
    pub env: Vec<String>,
}

/// Validates every input of `schedule set` before anything is written.
///
/// # Errors
///
/// `SCHEDULE_INVALID` naming the field.
pub async fn check_schedule_set(
    req: &ScheduleSet,
) -> Result<(BuiltinPayload, ScheduleSpec), CacheError> {
    let hook = match (&req.post_sync, &req.python) {
        (None, None) if req.env.is_empty() => None,
        (None, _) => {
            return Err(invalid(
                "post_sync",
                ScheduleReason::BadModule,
                "--python and --env name the post-sync hook's interpreter; give --post-sync too",
            ));
        }
        (Some(_), None) => {
            return Err(invalid(
                "python",
                ScheduleReason::NotAbsolute,
                "--post-sync needs --python <absolute interpreter>",
            ));
        }
        (Some(target), Some(python)) => {
            let hook = PostSyncHook::new(python, target, EnvNames::new(req.env.clone())?)?;
            hook.interpreter()
                .check_import(hook.env().child_env(vec![]))
                .await?;
            Some(hook)
        }
    };
    let budget = TickBudget::new(req.budget_ms, req.hook_reserve_ms, hook.is_some())
        .map_err(|e| budget_invalid(&e))?;
    let payload = BuiltinPayload { budget, hook };
    let spec = spec(
        BUILTIN_ROW,
        &req.expr,
        budget.row_timeout(),
        SyncPayload::Builtin(payload.clone()).to_json(),
    )?;
    Ok((payload, spec))
}

/// `schedule set` (REQ-BDC-0720): validates all, then writes the built-in
/// row in one store call, its `enabled` flag kept.
///
/// # Errors
///
/// `SCHEDULE_INVALID` naming the field, before anything is written;
/// `STORE_FAILED` from the table.
pub async fn schedule_set<St, C, J>(
    scheduler: &Scheduler<St, C, J>,
    req: &ScheduleSet,
) -> Result<Value, CacheError>
where
    St: ScheduleStore + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    let (_payload, spec) = check_schedule_set(req).await?;
    let rows = Rows::read(scheduler.store().as_ref())?;
    let python_enabled = rows.python.as_ref().is_some_and(|r| r.enabled);
    let enabled = match &rows.builtin {
        Some(row) => row.enabled,
        None => !python_enabled,
    };
    let flags = match &rows.python {
        Some(_) if enabled => vec![(name(PYTHON_ROW)?, false)],
        _ => Vec::new(),
    };
    let written = scheduler
        .replace_and_set_enabled_async(spec, enabled, flags)
        .await
        .map_err(schedule_store_failed)?;
    let mode = Rows::read(scheduler.store().as_ref())?.mode();
    Ok(json!({
        "row": BUILTIN_ROW,
        "enabled": written.enabled,
        "generation": written.generation.get(),
        "expr": written.spec.expr.as_str(),
        "next_run_at": written.next_run_at.map(|t| t.to_string()),
        "timeout_ms": u64::try_from(written.spec.timeout.get().as_millis()).unwrap_or(u64::MAX),
        "mode": mode,
    }))
}

/// `schedule enable` / `schedule disable`: flips only the built-in row's flag (REQ-CRON-0014).
///
/// # Errors
///
/// `SCHEDULE_INVALID`: `no_builtin_row` with no built-in row,
/// `python_sync_enabled` for an enable while the Python row is enabled.
pub async fn schedule_enable<St, C, J>(
    scheduler: &Scheduler<St, C, J>,
    enabled: bool,
) -> Result<Value, CacheError>
where
    St: ScheduleStore + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    let rows = Rows::read(scheduler.store().as_ref())?;
    if rows.builtin.is_none() {
        return Err(no_builtin_row());
    }
    if enabled && rows.python.as_ref().is_some_and(|r| r.enabled) {
        return Err(invalid(
            "schedule",
            ScheduleReason::PythonSyncEnabled,
            "the Python sync row is enabled; switch with bdc cache sync use-builtin",
        ));
    }
    scheduler
        .set_enabled_async(&name(BUILTIN_ROW)?, enabled)
        .await
        .map_err(schedule_store_failed)?;
    let mode = Rows::read(scheduler.store().as_ref())?.mode();
    Ok(json!({"row": BUILTIN_ROW, "enabled": enabled, "mode": mode}))
}

/// `schedule clear`: ends the built-in row (its record is kept, ADR-CRON-0007).
///
/// # Errors
///
/// `STORE_FAILED` from the table.
pub async fn schedule_clear<St, C, J>(scheduler: &Scheduler<St, C, J>) -> Result<Value, CacheError>
where
    St: ScheduleStore + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    let rows = Rows::read(scheduler.store().as_ref())?;
    let cleared = match &rows.builtin {
        Some(row) => {
            scheduler
                .stop_async(
                    row.id,
                    row.generation,
                    "cleared by bdc cache schedule clear",
                )
                .await
                .map_err(schedule_store_failed)?;
            true
        }
        None => false,
    };
    let mode = Rows::read(scheduler.store().as_ref())?.mode();
    Ok(json!({"row": BUILTIN_ROW, "cleared": cleared, "mode": mode}))
}

fn no_builtin_row() -> CacheError {
    invalid(
        "schedule",
        ScheduleReason::NoBuiltinRow,
        "there is no built-in sync row; create it with bdc cache schedule set <cron-expr>",
    )
}

/// One row in `schedule show`.
fn row_json(row: Option<&Schedule>, kind: &str) -> Value {
    let Some(row) = row else {
        return Value::Null;
    };
    let history = RowHistory::of(row.last_outcome.as_ref());
    let outcome = row.last_outcome.as_ref().map(|o| match o {
        RunOutcome::Ok => "ok".to_owned(),
        other => {
            crate::text::cut(&other.to_string(), crate::text::MAX_KEPT_STDERR_BYTES).to_owned()
        }
    });
    let mut v = json!({
        "kind": kind,
        "name": row.spec.name.as_str(),
        "enabled": row.enabled,
        "expr": row.spec.expr.as_str(),
        "next_run_at": row.next_run_at.map(|t| t.to_string()),
        "last_run_at": row.last_run_at.map(|t| t.to_string()),
        "outcome": outcome,
        "run_id": history.run_id,
        "consecutive_failures": history.failures,
        "first_failure_at": history.since,
        "generation": row.generation.get(),
    });
    // The payload, for the module of the Python row; never a secret (names only).
    if let (Value::Object(m), Ok(payload)) = (&mut v, SyncPayload::decode(&row.spec.payload)) {
        match payload {
            SyncPayload::Python(p) => {
                m.insert("module".into(), json!(p.interpreter.module()));
                m.insert(
                    "python".into(),
                    json!(p.interpreter.path().display().to_string()),
                );
                m.insert(
                    "timeout_ms".into(),
                    json!(u64::try_from(p.timeout.as_millis()).unwrap_or(u64::MAX)),
                );
            }
            SyncPayload::Builtin(b) => {
                m.insert(
                    "budget_ms".into(),
                    json!(u64::try_from(b.budget.budget().as_millis()).unwrap_or(u64::MAX)),
                );
                m.insert(
                    "hook_reserve_ms".into(),
                    json!(u64::try_from(b.budget.hook_reserve().as_millis()).unwrap_or(u64::MAX)),
                );
            }
        }
    } else if let Value::Object(m) = &mut v {
        m.insert("payload_invalid".into(), Value::Bool(true));
    }
    v
}

/// `schedule show` (REQ-BDC-0731).
///
/// # Errors
///
/// `STORE_FAILED` from the table or the store.
pub async fn schedule_show<St, S, L, B>(
    store: &Arc<St>,
    cache: &SharedCache<S, L, B>,
) -> Result<Value, CacheError>
where
    St: ScheduleStore + 'static,
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    let rows = {
        let store = Arc::clone(store);
        blocking(move || Rows::read(store.as_ref())).await?
    };
    let hook_module =
        rows.builtin
            .as_ref()
            .and_then(|r| match SyncPayload::decode(&r.spec.payload) {
                Ok(SyncPayload::Builtin(b)) => b.hook.map(|h| h.target()),
                _ => None,
            });
    let cache = Arc::clone(cache);
    let (lease, hook, scopes) = blocking(move || {
        let guard = hold(&cache)?;
        let lease = guard.lease()?;
        let hook = guard.hook_state()?;
        let mut scopes = Vec::new();
        for scope in guard.scopes()? {
            let state = guard.sync_state(&scope.id)?;
            scopes.push(json!({
                "scope": scope.id,
                "last_ok_at": state.last_ok_at.as_ref().map(sc_beads::Timestamp::as_str),
                "failing_since": state.failing_since,
            }));
        }
        Ok((lease, hook, scopes))
    })
    .await?;
    let mode = rows.mode();
    let mut flags = Vec::new();
    if rows
        .python
        .as_ref()
        .is_some_and(|r| r.last_run_at.is_none())
    {
        flags.push("python_never_run");
    }
    let both_disabled = rows.builtin.as_ref().is_none_or(|r| !r.enabled)
        && rows.python.as_ref().is_none_or(|r| !r.enabled)
        && (rows.builtin.is_some() || rows.python.is_some());
    if both_disabled {
        flags.push("both_disabled");
    }
    for row in [&rows.builtin, &rows.python].into_iter().flatten() {
        if SyncPayload::decode(&row.spec.payload).is_err() {
            flags.push(if row.spec.name.as_str() == BUILTIN_ROW {
                "builtin_payload_invalid"
            } else {
                "python_payload_invalid"
            });
        }
    }
    Ok(json!({
        "mode": mode,
        "rows": {
            "builtin": row_json(rows.builtin.as_ref(), "builtin"),
            "python": row_json(rows.python.as_ref(), "python"),
        },
        "hook": hook_module.map(|module| json!({
            "module": module,
            "consecutive_failures": hook.consecutive_failures,
            "first_failure_at": hook.first_failure_at.as_ref().map(sc_beads::Timestamp::as_str),
            "last_error": hook.last_error,
            "next_attempt_at": hook.next_attempt_at.as_ref().map(sc_beads::Timestamp::as_str),
        })),
        "lease": lease.as_ref().map(holder_json),
        "scopes": scopes,
        "flags": flags,
    }))
}

/// `sync use-python`'s inputs (REQ-BDC-0724).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsePython {
    /// `--python <absolute interpreter>`.
    pub python: PathBuf,
    /// `--module <module>`.
    pub module: String,
    /// `--cron <expr>`; default the built-in row's expression.
    pub cron: Option<String>,
    /// `--timeout-ms N` (default 60000, 1000–600000).
    pub timeout_ms: Option<u64>,
    /// `--env NAME`, repeatable.
    pub env: Vec<String>,
}

/// `sync use-python` (REQ-BDC-0724; module docs).
///
/// # Errors
///
/// `SCHEDULE_INVALID` for a refused input (nothing written);
/// `TICK_IN_FLIGHT` when another run holds the lease; `TICK_RUN_FAILED`
/// after the restore when the immediate run failed, with
/// `details.restored`; `STORE_FAILED` from the table.
pub async fn sync_use_python<St, S, L, B, C, J>(
    scheduler: &Scheduler<St, C, J>,
    ctx: &SyncContext<S, L, B, C>,
    req: &UsePython,
) -> Result<Value, CacheError>
where
    St: ScheduleStore + 'static,
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    let interpreter = TrustedInterpreter::new(&req.python, &req.module, "module")?;
    let env = EnvNames::new(req.env.clone())?;
    interpreter.check_import(env.child_env(vec![])).await?;
    let payload = PythonPayload::new(
        interpreter,
        ctx.cache_path.clone(),
        ctx.beads_dir.clone(),
        env,
        req.timeout_ms,
    )?;
    let rows = Rows::read(scheduler.store().as_ref())?;
    let expr = match (&req.cron, &rows.builtin) {
        (Some(expr), _) => expr.clone(),
        (None, Some(builtin)) => builtin.spec.expr.as_str().to_owned(),
        (None, None) => {
            return Err(invalid(
                "cron",
                ScheduleReason::CronRequired,
                "--cron is required when there is no built-in row to take the expression from",
            ));
        }
    };
    let new_spec = spec(
        PYTHON_ROW,
        &expr,
        payload.timeout,
        SyncPayload::Python(payload.clone()).to_json(),
    )?;
    // The lease, before the mode commit (REQ-BDC-0722).
    let lease = {
        let cache = Arc::clone(&ctx.cache);
        let timeout = payload.timeout;
        blocking(move || {
            let mut guard = hold(&cache)?;
            let clock = guard.clock();
            let run_id = new_run_id(&*clock);
            TickLease::take(
                &mut guard,
                run_id,
                sc_beads::Deadline::after(&*clock, timeout),
            )
        })
        .await?
    };
    let lease = match lease {
        LeaseTaken::Taken(lease) => lease,
        LeaseTaken::Held(holder) => {
            return Err(CacheError::TickInFlight {
                detail: "another sync run holds the lease".to_owned(),
                holder,
            });
        }
    };
    let builtin_flag = match &rows.builtin {
        Some(_) => vec![(name(BUILTIN_ROW)?, false)],
        None => Vec::new(),
    };
    let committed = scheduler
        .replace_and_set_enabled_async(new_spec.clone(), true, builtin_flag)
        .await
        .map_err(schedule_store_failed);
    let result = match committed {
        Ok(_) => {
            let job = PythonSyncJob::new(
                Arc::clone(&ctx.cache),
                payload,
                Arc::clone(&ctx.clock),
                ctx.kill.clone(),
                RowHistory::default(),
            );
            job.run_under(&lease).await
        }
        Err(e) => {
            release(&ctx.cache, lease).await;
            return Err(e);
        }
    };
    let outcome = match result {
        Ok(run) => Ok(json!({
            "row": PYTHON_ROW,
            "run_id": run.run_id,
            "exit_status": run.exit_status,
            "stderr": run.stderr,
            "mode": SyncMode::Python,
        })),
        Err(failure) => {
            let restored = restore(scheduler, &rows, new_spec).await;
            Err(with_restored(failure, restored))
        }
    };
    release(&ctx.cache, lease).await;
    outcome
}

/// Restores the state held before `use-python`, in one store call.
async fn restore<St, C, J>(
    scheduler: &Scheduler<St, C, J>,
    before: &Rows,
    new_spec: ScheduleSpec,
) -> bool
where
    St: ScheduleStore + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    let builtin_flag = match (&before.builtin, name(BUILTIN_ROW)) {
        (Some(row), Ok(n)) => vec![(n, row.enabled)],
        _ => Vec::new(),
    };
    let (spec, enabled) = match &before.python {
        Some(row) => (row.spec.clone(), row.enabled),
        None => (new_spec, false),
    };
    match scheduler
        .replace_and_set_enabled_async(spec, enabled, builtin_flag)
        .await
    {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(code = e.code(), error = %e,
                "the state before sync use-python could not be restored; run bdc cache sync use-builtin");
            false
        }
    }
}

fn with_restored(e: CacheError, ok: bool) -> CacheError {
    match e {
        CacheError::TickRunFailed {
            detail,
            row,
            run_id,
            reason,
            exit_status,
            stderr,
            ..
        } => {
            let detail = if ok {
                format!("{detail}; the state before the command was restored")
            } else {
                format!(
                    "{detail}; the state before the command could NOT be restored: run bdc cache sync use-builtin"
                )
            };
            CacheError::TickRunFailed {
                detail,
                row,
                run_id,
                reason,
                exit_status,
                stderr,
                restored: Some(ok),
            }
        }
        other => other,
    }
}

async fn release<S, L, B>(cache: &SharedCache<S, L, B>, lease: TickLease)
where
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    let cache = Arc::clone(cache);
    let _released = blocking(move || {
        let mut guard = hold(&cache)?;
        Ok(lease.release(&mut guard))
    })
    .await;
}

/// `sync use-builtin` (REQ-BDC-0724): no lease; the built-in row enabled and
/// the Python row disabled in one store call.
///
/// # Errors
///
/// `SCHEDULE_INVALID` (`no_builtin_row`) with no built-in row; `STORE_FAILED`.
pub async fn sync_use_builtin<St, C, J>(
    scheduler: &Scheduler<St, C, J>,
) -> Result<Value, CacheError>
where
    St: ScheduleStore + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    let rows = Rows::read(scheduler.store().as_ref())?;
    if rows.builtin.is_none() {
        return Err(no_builtin_row());
    }
    let mut flags = vec![(name(BUILTIN_ROW)?, true)];
    if rows.python.is_some() {
        flags.push((name(PYTHON_ROW)?, false));
    }
    scheduler
        .set_enabled_many_async(flags)
        .await
        .map_err(schedule_store_failed)?;
    Ok(json!({"mode": SyncMode::Builtin, "row": BUILTIN_ROW}))
}

/// An on-demand tick: the built-in row's code with its payload (or the
/// defaults when there is none), under the lease (REQ-BDC-0720).
///
/// Scheduled times come only from the rows: this changes none.
///
/// # Errors
///
/// [`SyncError::PythonSyncEnabled`] while the Python row is enabled (no bd
/// process starts); the run's errors otherwise.
pub async fn tick<St, S, L, B>(
    store: &Arc<St>,
    cache: &SharedCache<S, L, B>,
    ctx: &RunContext,
) -> Result<BuiltinReply, SyncError>
where
    St: ScheduleStore + 'static,
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
{
    let rows = {
        let store = Arc::clone(store);
        blocking(move || Rows::read(store.as_ref())).await?
    };
    if rows.mode() == SyncMode::Python {
        return Err(SyncError::PythonSyncEnabled(
            "the Python sync row is enabled: its script is the sync; switch with bdc cache sync use-builtin"
                .to_owned(),
        ));
    }
    let (payload, period) = match &rows.builtin {
        Some(row) => match SyncPayload::decode(&row.spec.payload)? {
            SyncPayload::Builtin(p) => (p, row_period(row)),
            SyncPayload::Python(_) => {
                return Err(invalid(
                    "payload",
                    ScheduleReason::OutOfRange,
                    "the built-in row holds a Python payload",
                )
                .into());
            }
        },
        None => (BuiltinPayload::defaults()?, None),
    };
    let ctx = RunContext {
        period,
        ..ctx.clone()
    };
    Ok(run_builtin(cache, &payload, &ctx).await?)
}

/// One period of a row at evaluation (REQ-BDC-0727 as defined by QA97R2-016):
/// the gap between the row's next two scheduled times, its stored next run
/// and the run after it. For an irregular expression the period changes with
/// the next run; `None` for a row with no next run or no run after it.
#[must_use]
pub fn row_period(row: &Schedule) -> Option<Duration> {
    let next = row.next_run_at?;
    let after = row.next_run_after(next)?;
    (after.as_datetime() - next.as_datetime()).to_std().ok()
}

/// The schedule and switch operations over the cache file's table on the
/// system clock, for a front end with no sc-cron edge of its own
/// (`bd-cache-py`, ADR-PY-0006): it holds sc-cron's `Scheduler` here, so
/// bd-cache-sync re-exports none of sc-cron's API.
pub struct SystemSchedules {
    store: Arc<bd_cache_store::CacheScheduleStore>,
    scheduler:
        Scheduler<bd_cache_store::CacheScheduleStore, sc_cron::SystemClock, sc_cron::SystemJitter>,
}

impl std::fmt::Debug for SystemSchedules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemSchedules").finish_non_exhaustive()
    }
}

impl SystemSchedules {
    /// The operations over `store` (from [`crate::Composed::schedule_store`]).
    #[must_use]
    pub fn new(store: Arc<bd_cache_store::CacheScheduleStore>) -> Self {
        let scheduler = Scheduler::new(
            Arc::clone(&store),
            Arc::new(sc_cron::SystemClock),
            Arc::new(sc_cron::SystemJitter),
        );
        Self { store, scheduler }
    }

    /// [`schedule_set`].
    ///
    /// # Errors
    ///
    /// As [`schedule_set`].
    pub async fn set(&self, req: &ScheduleSet) -> Result<Value, CacheError> {
        schedule_set(&self.scheduler, req).await
    }

    /// [`schedule_show`].
    ///
    /// # Errors
    ///
    /// As [`schedule_show`].
    pub async fn show<S, L, B>(&self, cache: &SharedCache<S, L, B>) -> Result<Value, CacheError>
    where
        S: CacheStore + 'static,
        L: DrainLock + 'static,
        B: CacheBackend + 'static,
    {
        schedule_show(&self.store, cache).await
    }

    /// [`schedule_enable`].
    ///
    /// # Errors
    ///
    /// As [`schedule_enable`].
    pub async fn enable(&self, enabled: bool) -> Result<Value, CacheError> {
        schedule_enable(&self.scheduler, enabled).await
    }

    /// [`schedule_clear`].
    ///
    /// # Errors
    ///
    /// As [`schedule_clear`].
    pub async fn clear(&self) -> Result<Value, CacheError> {
        schedule_clear(&self.scheduler).await
    }

    /// [`sync_use_python`], its immediate run killed when `kill` is set.
    ///
    /// # Errors
    ///
    /// As [`sync_use_python`].
    pub async fn use_python<S, L, B>(
        &self,
        cache: &SharedCache<S, L, B>,
        cache_path: PathBuf,
        beads_dir: Option<PathBuf>,
        kill: crate::StopFlag,
        req: &UsePython,
    ) -> Result<Value, CacheError>
    where
        S: CacheStore + 'static,
        L: DrainLock + 'static,
        B: CacheBackend + 'static,
    {
        let ctx = SyncContext {
            cache: Arc::clone(cache),
            clock: Arc::new(sc_cron::SystemClock),
            cache_path,
            beads_dir,
            stop: kill.clone(),
            kill,
        };
        sync_use_python(&self.scheduler, &ctx, req).await
    }

    /// [`sync_use_builtin`].
    ///
    /// # Errors
    ///
    /// As [`sync_use_builtin`].
    pub async fn use_builtin(&self) -> Result<Value, CacheError> {
        sync_use_builtin(&self.scheduler).await
    }

    /// [`tick`].
    ///
    /// # Errors
    ///
    /// As [`tick`].
    pub async fn tick<S, L, B>(
        &self,
        cache: &SharedCache<S, L, B>,
        ctx: &RunContext,
    ) -> Result<BuiltinReply, SyncError>
    where
        S: CacheStore + 'static,
        L: DrainLock + 'static,
        B: CacheBackend + 'static,
    {
        tick(&self.store, cache, ctx).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "test code: a failed unwrap is a test failure"
    )]

    use super::*;

    fn row(expr: &str, next: &str) -> Schedule {
        let spec = ScheduleSpec::new("s", expr, Duration::from_secs(5), json!({})).unwrap();
        let at: sc_cron::Timestamp = next.parse().unwrap();
        Schedule {
            id: sc_cron::ScheduleId::new(1),
            generation: sc_cron::Generation::FIRST,
            spec,
            jitter: Duration::ZERO,
            created_at: at,
            next_run_at: Some(at),
            last_run_at: None,
            last_outcome: None,
            ended_at: None,
            stop_reason: None,
            enabled: true,
        }
    }

    /// The period of a row is the gap between its next two scheduled times
    /// at evaluation (QA97R2-016): constant for a regular expression, and
    /// for an irregular one it follows the next run. Planted negative: a row
    /// with no next run has none.
    #[test]
    fn the_period_of_a_row_is_the_gap_between_its_next_two_runs() {
        assert_eq!(
            row_period(&row("*/30 * * * * *", "2026-10-26T10:00:30Z")),
            Some(Duration::from_secs(30))
        );
        let irregular = "0 0,5 * * * *";
        assert_eq!(
            row_period(&row(irregular, "2026-10-26T10:00:00Z")),
            Some(Duration::from_mins(5))
        );
        assert_eq!(
            row_period(&row(irregular, "2026-10-26T10:05:00Z")),
            Some(Duration::from_mins(55))
        );
        let mut none = row(irregular, "2026-10-26T10:05:00Z");
        none.next_run_at = None;
        assert_eq!(row_period(&none), None);
    }
}
