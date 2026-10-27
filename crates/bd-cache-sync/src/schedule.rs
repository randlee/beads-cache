//! The sync mode: the built-in and Python rows, their flags and the switch
//! (REQ-BDC-0720, 0722, 0724, 0731; ADR-SYNC-0008).
//!
//! [`Schedules`] is the one surface front ends call: `schedule set|show|
//! enable|disable|clear`, `sync use-python|use-builtin`, `tick`, `cron run`
//! and `cron run --once` (the last two in the `cron` module). Every
//! mode-changing command decides the rows' flags from a read made under the
//! mode lock, and writes them in one sc-cron store call in one `BEGIN
//! IMMEDIATE` transaction (`replace_and_set_enabled` or `set_enabled_many`,
//! REQ-CRON-0017), so no connection ever sees both rows enabled, not even
//! while a row is created, and of two racing commands the later decides:
//!
//! - [`Schedules::set`] writes the built-in row: an existing row keeps its
//!   `enabled` flag; a new row is enabled unless the Python row is enabled
//!   (then it is written disabled); a row written enabled disables the Python
//!   row in the same call. An active row is replaced in place (generation + 1).
//! - [`Schedules::use_python`] checks its inputs, takes the lease, then reads
//!   both rows and writes the one Python row enabled and the built-in row
//!   disabled in one call, runs the row at once under that lease (its
//!   outcome recorded on the row as a worker run's is), and on any failure
//!   restores, in one call, the state it read, unless another command has
//!   replaced the Python row since.
//! - [`Schedules::use_builtin`] takes no lease and enables the built-in row
//!   and disables the Python row in one call; it always works when a
//!   built-in row exists, which makes it the recovery.
//! - [`Schedules::enable`] of the built-in row while the Python row is
//!   enabled is `SCHEDULE_INVALID` (`python_sync_enabled`).
//!
//! Every store call runs on tokio's blocking pool. The messages are neutral;
//! a front end adds its own command wording as the suggested action.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bd_cache_model::{
    CacheError, CacheStore, DrainLock, PreconditionReason, ScheduleReason, SyncRunEnd,
};
use sc_beads::{CacheBackend, Deadline, Timestamp};
use sc_cron::{
    Clock as CronClock, CronError, JitterSource, MissedTickPolicy, RunOutcome, Schedule,
    ScheduleSpec, ScheduleStore, Scheduler,
};
use serde::Serialize;

use crate::control::StopFlag;
use crate::history::{RowHistory, RunRecord};
use crate::job::{FailureLog, SyncFiles};
use crate::lease::{LeaseTaken, TickLease, hold, run_id_now, run_id_on};
use crate::mode_lock::ModeLock;
use crate::payload::{
    BuiltinPayload, EnvNames, Field, PostSyncHook, PythonPayload, SyncPayload, SyncRow,
    TrustedInterpreter, budget_invalid, invalid, schedule_name,
};
use crate::python_job::{PythonRun, PythonSyncJob, RunFiles, outcome_of};
use crate::run::{BuiltinReply, HolderView, RunContext, SharedCache, blocking, run_builtin};
use crate::text::{MAX_KEPT_STDERR_BYTES, cut};

#[doc(inline)]
pub use bd_cache_model::SyncMode;

/// An error of the on-demand tick.
#[derive(Debug)]
#[non_exhaustive]
pub enum SyncError {
    /// A typed cache error: the run's, or the table's (`STORE_FAILED`).
    Cache(CacheError),
    /// An on-demand `tick` while the Python row is enabled (REQ-BDC-0720):
    /// `PRECONDITION` with reason `python_sync_enabled`; no bd process started.
    PythonSyncEnabled,
}

impl SyncError {
    /// The `BD_CACHE.*` code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Cache(e) => e.code(),
            Self::PythonSyncEnabled => "BD_CACHE.PRECONDITION",
        }
    }

    /// The error as the cache's typed error: the one mapping every front end
    /// shares (`PRECONDITION`, [`PreconditionReason::PythonSyncEnabled`]).
    #[must_use]
    pub fn into_cache_error(self) -> CacheError {
        match self {
            Self::Cache(e) => e,
            Self::PythonSyncEnabled => CacheError::Precondition {
                detail: "the Python sync row is enabled: its script is the sync, and an \
                         on-demand tick of the built-in row's code would race it"
                    .to_owned(),
                unmet: std::collections::BTreeMap::new(),
                reason: PreconditionReason::PythonSyncEnabled,
                source: None,
            },
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cache(e) => std::fmt::Display::fmt(e, f),
            Self::PythonSyncEnabled => f.write_str("the Python sync row is enabled"),
        }
    }
}

impl std::error::Error for SyncError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cache(e) => Some(e),
            Self::PythonSyncEnabled => None,
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
    /// Reads the active rows (a blocking store call).
    ///
    /// # Errors
    ///
    /// `STORE_FAILED` when the table cannot be read.
    pub fn read<St: ScheduleStore + ?Sized>(store: &St) -> Result<Self, CacheError> {
        let mut rows = Self::default();
        for row in store.list_active().map_err(schedule_store_failed)? {
            match SyncRow::of(row.spec.name.as_str()) {
                Some(SyncRow::Builtin) => rows.builtin = Some(row),
                Some(SyncRow::Python) => rows.python = Some(row),
                None => {}
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

    fn get(&self, row: SyncRow) -> Option<&Schedule> {
        match row {
            SyncRow::Builtin => self.builtin.as_ref(),
            SyncRow::Python => self.python.as_ref(),
        }
    }

    /// The flags a write of `row` with `enabled` sets on the other row:
    /// written enabled, the other row (when it exists) is disabled in the
    /// same call; written disabled, the other row is left as it is.
    fn others(&self, row: SyncRow, enabled: bool) -> Vec<(sc_cron::ScheduleName, bool)> {
        let other = row.other();
        match self.get(other) {
            Some(_) if enabled => vec![(schedule_name(other), false)],
            _ => Vec::new(),
        }
    }
}

fn spec(
    row: SyncRow,
    expr: &str,
    timeout: Duration,
    payload: &SyncPayload,
) -> Result<ScheduleSpec, CacheError> {
    ScheduleSpec::new(row.as_str(), expr, timeout, payload.to_json()).map_err(|e| match e {
        CronError::BadExpression(why) => invalid(
            Field::Cron,
            ScheduleReason::OutOfRange,
            format!("{expr:?} is not a cron expression: {why}"),
        ),
        other => invalid(
            Field::Payload,
            ScheduleReason::OutOfRange,
            other.to_string(),
        ),
    })
}

/// `schedule set`'s inputs (REQ-BDC-0720).
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl ScheduleSet {
    /// The inputs of the contract's `cache schedule set` request.
    #[must_use]
    pub fn of(req: &bd_cache_api::request::ScheduleSet) -> Self {
        Self {
            expr: req.cron().to_owned(),
            budget_ms: req.budget_ms(),
            hook_reserve_ms: req.hook_reserve_ms(),
            post_sync: req.post_sync().map(str::to_owned),
            python: req.python().map(PathBuf::from),
            env: req.env().to_vec(),
        }
    }

    /// Validates every input before anything is written: the hook's
    /// interpreter and its import check, the budget pair, the expression.
    async fn check(&self) -> Result<ScheduleSpec, CacheError> {
        let hook = match (&self.post_sync, &self.python) {
            (None, None) if self.env.is_empty() => None,
            (None, _) => {
                return Err(invalid(
                    Field::PostSync,
                    ScheduleReason::BadModule,
                    "--python and --env name the post-sync hook's interpreter; give --post-sync too",
                ));
            }
            (Some(_), None) => {
                return Err(invalid(
                    Field::Python,
                    ScheduleReason::NotAbsolute,
                    "--post-sync needs --python <absolute interpreter>",
                ));
            }
            (Some(target), Some(python)) => {
                let hook = PostSyncHook::new(python, target, EnvNames::new(self.env.clone())?)?;
                hook.interpreter()
                    .check_import(hook.env().child_env(vec![]))
                    .await?;
                Some(hook)
            }
        };
        let budget =
            bd_cache_model::TickBudget::new(self.budget_ms, self.hook_reserve_ms, hook.is_some())
                .map_err(|e| budget_invalid(&e))?;
        let payload = SyncPayload::Builtin(BuiltinPayload { budget, hook });
        spec(SyncRow::Builtin, &self.expr, budget.row_timeout(), &payload)
    }
}

/// `sync use-python`'s inputs (REQ-BDC-0724).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsePython {
    /// `--python <absolute interpreter>`.
    pub python: PathBuf,
    /// `--module <module>`.
    pub module: String,
    /// `--cron <expr>`; default the built-in row's expression.
    pub cron: Option<String>,
    /// `--timeout-ms N` (default 60000, 1000 to `MAX_PYTHON_TIMEOUT_MS`).
    pub timeout_ms: Option<u64>,
    /// `--env NAME`, repeatable.
    pub env: Vec<String>,
}

impl UsePython {
    /// The inputs of the contract's `cache sync use-python` request.
    #[must_use]
    pub fn of(req: &bd_cache_api::request::SyncUsePython) -> Self {
        Self {
            python: PathBuf::from(req.python()),
            module: req.module().to_owned(),
            cron: req.cron().map(str::to_owned),
            timeout_ms: req.timeout_ms(),
            env: req.env().to_vec(),
        }
    }
}

/// `schedule set`'s reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScheduleSetReply {
    /// The built-in row.
    pub row: &'static str,
    /// Its flag as written.
    pub enabled: bool,
    /// Its generation after the write.
    pub generation: u32,
    /// Its expression.
    pub expr: String,
    /// Its next run.
    pub next_run_at: Option<String>,
    /// Its sc-cron timeout (`budget_ms + 10000`).
    pub timeout_ms: u64,
    /// The mode after the write.
    pub mode: SyncMode,
}

/// `schedule enable` / `disable`'s reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScheduleEnableReply {
    /// The built-in row.
    pub row: &'static str,
    /// Its flag now.
    pub enabled: bool,
    /// The mode now.
    pub mode: SyncMode,
}

/// `schedule clear`'s reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScheduleClearReply {
    /// The built-in row.
    pub row: &'static str,
    /// Whether a row was ended.
    pub cleared: bool,
    /// The mode now.
    pub mode: SyncMode,
}

/// `sync use-python`'s reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsePythonReply {
    /// The Python row.
    pub row: &'static str,
    /// The immediate run.
    pub run_id: bd_cache_model::RunId,
    /// Its exit status.
    pub exit_status: Option<i32>,
    /// Its stderr, redacted and cut to 8 KiB.
    pub stderr: String,
    /// The mode now (`python`).
    pub mode: SyncMode,
}

/// `sync use-builtin`'s reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UseBuiltinReply {
    /// The built-in row.
    pub row: &'static str,
    /// The mode now (`builtin`).
    pub mode: SyncMode,
}

/// A flag `schedule show` raises (REQ-BDC-0731).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShowFlag {
    /// The Python row exists and has never run.
    PythonNeverRun,
    /// A sync row exists and none is enabled.
    BothDisabled,
    /// The built-in row's payload does not decode.
    BuiltinPayloadInvalid,
    /// The Python row's payload does not decode.
    PythonPayloadInvalid,
}

/// What a row's payload says, in `schedule show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum PayloadView {
    /// The built-in row's budget pair.
    Builtin {
        /// `budget_ms`.
        budget_ms: u64,
        /// `hook_reserve_ms`.
        hook_reserve_ms: u64,
    },
    /// The Python row's script (names only, never a secret).
    Python {
        /// The module.
        module: String,
        /// The interpreter.
        python: String,
        /// The timeout.
        timeout_ms: u64,
    },
    /// The payload does not decode.
    Invalid {
        /// Always `true`.
        payload_invalid: bool,
    },
}

/// One sync row in `schedule show` (REQ-BDC-0731).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RowView {
    /// Which sync row.
    pub kind: SyncRow,
    /// Its name.
    pub name: String,
    /// Its flag.
    pub enabled: bool,
    /// Its expression.
    pub expr: String,
    /// Its next run.
    pub next_run_at: Option<String>,
    /// Its last run, as sc-cron recorded it.
    pub last_run_at: Option<String>,
    /// Its last outcome (`ok`, or the failure's redacted text, cut to 8 KiB).
    pub outcome: Option<String>,
    /// Its latest run, consecutive failures and first failure (typed, from its run record).
    #[serde(flatten)]
    pub history: RowHistory,
    /// Its generation.
    pub generation: u32,
    /// What its payload says.
    #[serde(flatten)]
    pub payload: PayloadView,
}

/// The built-in row's hook in `schedule show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HookView {
    /// `module:function`.
    pub module: String,
    /// Consecutive failures.
    pub consecutive_failures: u32,
    /// The first of them.
    pub first_failure_at: Option<Timestamp>,
    /// The latest error.
    pub last_error: Option<String>,
    /// The backoff's next attempt.
    pub next_attempt_at: Option<Timestamp>,
}

/// One scope in `schedule show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeView {
    /// The scope.
    pub scope: bd_cache_model::ScopeId,
    /// Its last successful sync.
    pub last_ok_at: Option<Timestamp>,
    /// Its current failure's token.
    pub failing_since: Option<bd_cache_model::FailingSince>,
}

/// Both sync rows of `schedule show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RowsView {
    /// The built-in row.
    pub builtin: Option<RowView>,
    /// The Python row.
    pub python: Option<RowView>,
}

/// `schedule show`'s reply (REQ-BDC-0731).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScheduleShow {
    /// The mode.
    pub mode: SyncMode,
    /// Both rows.
    pub rows: RowsView,
    /// The built-in row's hook, when it names one.
    pub hook: Option<HookView>,
    /// The lease's holder, or `None` when free.
    pub lease: Option<HolderView>,
    /// Each scope's sync state.
    pub scopes: Vec<ScopeView>,
    /// The flags raised.
    pub flags: Vec<ShowFlag>,
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// A row's last outcome as `schedule show` and `cache health` report it:
/// `ok`, or the failure's redacted text, cut to 8 KiB.
pub(crate) fn outcome_text(outcome: &RunOutcome) -> String {
    match outcome {
        RunOutcome::Ok => "ok".to_owned(),
        other => cut(&other.to_string(), MAX_KEPT_STDERR_BYTES).to_owned(),
    }
}

fn row_view(row: &Schedule, kind: SyncRow, history: RowHistory) -> RowView {
    let outcome = row.last_outcome.as_ref().map(outcome_text);
    let payload = match SyncPayload::decode(&row.spec.payload) {
        Ok(SyncPayload::Python(p)) => PayloadView::Python {
            module: p.interpreter().module().to_owned(),
            python: p.interpreter().path().display().to_string(),
            timeout_ms: millis(p.timeout().get()),
        },
        Ok(SyncPayload::Builtin(b)) => PayloadView::Builtin {
            budget_ms: millis(b.budget.budget()),
            hook_reserve_ms: millis(b.budget.hook_reserve()),
        },
        Err(_) => PayloadView::Invalid {
            payload_invalid: true,
        },
    };
    RowView {
        kind,
        name: row.spec.name.as_str().to_owned(),
        enabled: row.enabled,
        expr: row.spec.expr.as_str().to_owned(),
        next_run_at: row.next_run_at.map(|t| t.to_string()),
        last_run_at: row.last_run_at.map(|t| t.to_string()),
        outcome,
        history,
        generation: row.generation.get(),
        payload,
    }
}

/// One period of a row at evaluation (REQ-BDC-0727 as defined by QA97R2-016):
/// the gap between the row's next two scheduled times, its stored next run
/// and the run after it. For an irregular expression the period changes with
/// the next run; `None` for a row with no next run or no run after it.
#[must_use]
pub(crate) fn row_period(row: &Schedule) -> Option<Duration> {
    let next = row.next_run_at?;
    let after = row.next_run_after(next)?;
    (after.as_datetime() - next.as_datetime()).to_std().ok()
}

fn no_builtin_row() -> CacheError {
    invalid(
        Field::Schedule,
        ScheduleReason::NoBuiltinRow,
        "there is no built-in sync row; write one with schedule set first",
    )
}

/// The schedule, switch, tick and cron operations over one cache file's
/// schedule table (module docs): the one surface the front ends call.
///
/// Its sc-cron `Scheduler` skips missed ticks (`MissedTickPolicy::Skip`).
/// [`SystemSchedules`] is it on the system clock, over the real store.
pub struct Schedules<St, S, L, B, C, J>
where
    St: ScheduleStore,
    S: CacheStore,
    L: DrainLock,
    B: CacheBackend,
    C: CronClock,
    J: JitterSource,
{
    pub(crate) scheduler: Scheduler<St, C, J>,
    pub(crate) store: Arc<St>,
    pub(crate) cache: SharedCache<S, L, B>,
    pub(crate) files: SyncFiles,
    pub(crate) clock: Arc<C>,
    mode: ModeLock,
}

/// [`Schedules`] over the cache file's table and the real cache, on the
/// system clock (what `bdc` and `bd-cache-py` use; ADR-PY-0006).
pub type SystemSchedules = Schedules<
    bd_cache_store::CacheScheduleStore,
    bd_cache_store::SqliteStore,
    bd_cache_store::FlockDrainLock,
    sc_bd_cli::BdCli,
    sc_cron::SystemClock,
    sc_cron::SystemJitter,
>;

impl<St, S, L, B, C, J> std::fmt::Debug for Schedules<St, S, L, B, C, J>
where
    St: ScheduleStore,
    S: CacheStore,
    L: DrainLock,
    B: CacheBackend,
    C: CronClock,
    J: JitterSource,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Schedules")
            .field("files", &self.files)
            .finish_non_exhaustive()
    }
}

impl SystemSchedules {
    /// The operations over the real cache `cache` with its table `store`
    /// (from [`crate::Composed::schedule_store`]) and `files`.
    #[must_use]
    pub fn system(
        store: Arc<bd_cache_store::CacheScheduleStore>,
        cache: SharedCache<
            bd_cache_store::SqliteStore,
            bd_cache_store::FlockDrainLock,
            sc_bd_cli::BdCli,
        >,
        files: SyncFiles,
    ) -> Self {
        Self::new(
            store,
            Arc::new(sc_cron::SystemClock),
            Arc::new(sc_cron::SystemJitter),
            cache,
            files,
        )
    }
}

impl<St, S, L, B, C, J> Schedules<St, S, L, B, C, J>
where
    St: ScheduleStore + 'static,
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    /// The operations over `store` and `cache`, on `clock` and `jitter`.
    #[must_use]
    pub fn new(
        store: Arc<St>,
        clock: Arc<C>,
        jitter: Arc<J>,
        cache: SharedCache<S, L, B>,
        files: SyncFiles,
    ) -> Self {
        let scheduler = Scheduler::new(Arc::clone(&store), Arc::clone(&clock), jitter)
            .with_missed_tick_policy(MissedTickPolicy::Skip);
        Self {
            mode: ModeLock::for_cache_file(&files.cache_path),
            scheduler,
            store,
            cache,
            files,
            clock,
        }
    }

    /// The files the runs use.
    #[must_use]
    pub fn files(&self) -> &SyncFiles {
        &self.files
    }

    /// The cache the runs share, for a test that drives it between runs.
    #[doc(hidden)]
    #[must_use]
    pub fn cache_for_tests(&self) -> SharedCache<S, L, B> {
        Arc::clone(&self.cache)
    }

    /// The two rows now (a store read on the blocking pool).
    ///
    /// # Errors
    ///
    /// `STORE_FAILED` when the table cannot be read.
    pub async fn rows(&self) -> Result<Rows, CacheError> {
        let store = Arc::clone(&self.store);
        blocking(move || Rows::read(store.as_ref())).await
    }

    /// `schedule set` (REQ-BDC-0720): validates all, then, under the mode
    /// lock, writes the built-in row in one store call, its `enabled` flag kept.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` naming the field, before anything is written;
    /// `STORE_FAILED` from the table or the mode lock.
    pub async fn set(&self, req: &ScheduleSet) -> Result<ScheduleSetReply, CacheError> {
        let spec = req.check().await?;
        let _mode = self.mode.hold().await?;
        let rows = self.rows().await?;
        let python_enabled = rows.python.as_ref().is_some_and(|r| r.enabled);
        let enabled = rows.builtin.as_ref().map_or(!python_enabled, |r| r.enabled);
        let flags = rows.others(SyncRow::Builtin, enabled);
        let written = self
            .scheduler
            .replace_and_set_enabled_async(spec, enabled, flags)
            .await
            .map_err(schedule_store_failed)?;
        let mode = self.rows().await?.mode();
        Ok(ScheduleSetReply {
            row: SyncRow::Builtin.as_str(),
            enabled: written.enabled,
            generation: written.generation.get(),
            expr: written.spec.expr.as_str().to_owned(),
            next_run_at: written.next_run_at.map(|t| t.to_string()),
            timeout_ms: millis(written.spec.timeout.get()),
            mode,
        })
    }

    /// `schedule enable` / `schedule disable`: flips only the built-in row's
    /// flag (REQ-CRON-0014), under the mode lock.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID`: `no_builtin_row` with no built-in row,
    /// `python_sync_enabled` for an enable while the Python row is enabled;
    /// `STORE_FAILED`.
    pub async fn enable(&self, enabled: bool) -> Result<ScheduleEnableReply, CacheError> {
        let _mode = self.mode.hold().await?;
        let rows = self.rows().await?;
        if rows.builtin.is_none() {
            return Err(no_builtin_row());
        }
        if enabled && rows.python.as_ref().is_some_and(|r| r.enabled) {
            return Err(invalid(
                Field::Schedule,
                ScheduleReason::PythonSyncEnabled,
                "the Python sync row is enabled; switch to the built-in row instead",
            ));
        }
        self.scheduler
            .set_enabled_async(&schedule_name(SyncRow::Builtin), enabled)
            .await
            .map_err(schedule_store_failed)?;
        let mode = self.rows().await?.mode();
        Ok(ScheduleEnableReply {
            row: SyncRow::Builtin.as_str(),
            enabled,
            mode,
        })
    }

    /// `schedule clear`: ends the built-in row (its record is kept, ADR-CRON-0007).
    ///
    /// # Errors
    ///
    /// `STORE_FAILED` from the table or the mode lock.
    pub async fn clear(&self) -> Result<ScheduleClearReply, CacheError> {
        let _mode = self.mode.hold().await?;
        let rows = self.rows().await?;
        let cleared = match &rows.builtin {
            Some(row) => {
                self.scheduler
                    .stop_async(row.id, row.generation, "cleared by schedule clear")
                    .await
                    .map_err(schedule_store_failed)?;
                true
            }
            None => false,
        };
        let mode = self.rows().await?.mode();
        Ok(ScheduleClearReply {
            row: SyncRow::Builtin.as_str(),
            cleared,
            mode,
        })
    }

    /// `schedule show` (REQ-BDC-0731).
    ///
    /// # Errors
    ///
    /// `STORE_FAILED` from the table or the store.
    pub async fn show(&self) -> Result<ScheduleShow, CacheError> {
        let rows = self.rows().await?;
        let cache = Arc::clone(&self.cache);
        let (lease, hook, scopes, builtin_run, python_run) = blocking(move || {
            let guard = hold(&cache)?;
            let lease = guard.lease()?;
            let hook = guard.hook_state()?;
            let mut scopes = Vec::new();
            for scope in guard.scopes()? {
                let state = guard.sync_state(&scope.id)?;
                scopes.push(ScopeView {
                    scope: scope.id,
                    last_ok_at: state.last_ok_at,
                    failing_since: state.failing_since,
                });
            }
            let builtin_run = guard.sync_run(SyncRow::Builtin.as_str())?;
            let python_run = guard.sync_run(SyncRow::Python.as_str())?;
            Ok((lease, hook, scopes, builtin_run, python_run))
        })
        .await?;
        let view = |kind: SyncRow, record: Option<&bd_cache_model::SyncRun>| {
            rows.get(kind)
                .map(|row| row_view(row, kind, RowHistory::of(record, row, lease.as_ref())))
        };
        let hook_module =
            rows.builtin
                .as_ref()
                .and_then(|r| match SyncPayload::decode(&r.spec.payload) {
                    Ok(SyncPayload::Builtin(b)) => b.hook.map(|h| h.target()),
                    _ => None,
                });
        let mut flags = Vec::new();
        if rows
            .python
            .as_ref()
            .is_some_and(|r| r.last_run_at.is_none())
        {
            flags.push(ShowFlag::PythonNeverRun);
        }
        if (rows.builtin.is_some() || rows.python.is_some()) && rows.mode() == SyncMode::None {
            flags.push(ShowFlag::BothDisabled);
        }
        for (kind, flag) in [
            (SyncRow::Builtin, ShowFlag::BuiltinPayloadInvalid),
            (SyncRow::Python, ShowFlag::PythonPayloadInvalid),
        ] {
            if rows
                .get(kind)
                .is_some_and(|r| SyncPayload::decode(&r.spec.payload).is_err())
            {
                flags.push(flag);
            }
        }
        Ok(ScheduleShow {
            mode: rows.mode(),
            rows: RowsView {
                builtin: view(SyncRow::Builtin, builtin_run.as_ref()),
                python: view(SyncRow::Python, python_run.as_ref()),
            },
            hook: hook_module.map(|module| HookView {
                module,
                consecutive_failures: hook.consecutive_failures,
                first_failure_at: hook.first_failure_at,
                last_error: hook.last_error,
                next_attempt_at: hook.next_attempt_at,
            }),
            lease: lease.as_ref().map(HolderView::from),
            scopes,
            flags,
        })
    }

    /// `sync use-python` (REQ-BDC-0724; module docs); `kill` kills the
    /// immediate run's tree at once (a signal), which then restores.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` for a refused input (nothing written);
    /// `TICK_IN_FLIGHT` when another run holds the lease; `TICK_RUN_FAILED`
    /// after the restore when the immediate run failed, with
    /// `details.restored`; `STORE_FAILED` from the table or the mode lock.
    pub async fn use_python(
        &self,
        req: &UsePython,
        kill: StopFlag,
    ) -> Result<UsePythonReply, CacheError> {
        let interpreter = TrustedInterpreter::new(&req.python, &req.module)?;
        let env = EnvNames::new(req.env.clone())?;
        interpreter.check_import(env.child_env(vec![])).await?;
        let payload = PythonPayload::new(
            interpreter,
            self.files.cache_path.clone(),
            self.files.beads_dir.clone(),
            env,
            req.timeout_ms,
        )?;
        // The lease first (REQ-BDC-0722), then the rows (ADR-SYNC-0008).
        let lease = self.take_lease(payload.timeout().get()).await?;
        let committed = self.commit_python(req, &payload).await;
        let (before, written, new_spec) = match committed {
            Ok(parts) => parts,
            Err(e) => {
                self.release(lease).await;
                return Err(e);
            }
        };
        let job = PythonSyncJob::new(
            Arc::clone(&self.cache),
            payload,
            RunFiles {
                cache_path: self.files.cache_path.clone(),
                beads_dir: self.files.beads_dir.clone(),
                repo_root: self.files.repo_root.clone(),
            },
            Arc::clone(&self.clock),
            kill,
            lease.run_id().clone(),
            FailureLog::default(),
        );
        let record = RunRecord::new(
            Arc::clone(&self.cache),
            SyncRow::Python,
            lease.run_id().clone(),
        );
        record.start().await;
        let result = job.run_under(&lease).await;
        record
            .finish(if result.is_ok() {
                SyncRunEnd::Ok
            } else {
                SyncRunEnd::Failed
            })
            .await;
        self.record_on_row(lease.run_id(), &written, &result).await;
        let outcome = match result {
            Ok(PythonRun {
                run_id,
                exit_status,
                stderr,
            }) => Ok(UsePythonReply {
                row: SyncRow::Python.as_str(),
                run_id,
                exit_status,
                stderr,
                mode: SyncMode::Python,
            }),
            Err(failure) => {
                let restored = self
                    .restore(lease.run_id(), &before, &written, new_spec)
                    .await;
                Err(with_restored(failure, restored))
            }
        };
        self.release(lease).await;
        outcome
    }

    /// Takes the lease for an immediate run of `timeout`.
    async fn take_lease(&self, timeout: Duration) -> Result<TickLease, CacheError> {
        let cache = Arc::clone(&self.cache);
        let run_id = run_id_on(&*self.clock)?;
        let taken = blocking(move || {
            let mut guard = hold(&cache)?;
            let clock = guard.clock();
            TickLease::take(
                &cache,
                &mut guard,
                run_id,
                Deadline::after(&*clock, timeout),
            )
        })
        .await?;
        match taken {
            LeaseTaken::Taken(lease) => Ok(lease),
            LeaseTaken::Held(holder) => Err(CacheError::TickInFlight {
                detail: "another sync run holds the lease".to_owned(),
                holder,
            }),
        }
    }

    /// The mode commit of `use-python`, under the mode lock: the rows read,
    /// then the Python row written enabled and the built-in row disabled in
    /// one call. Returns the rows read, the row written and its spec.
    async fn commit_python(
        &self,
        req: &UsePython,
        payload: &PythonPayload,
    ) -> Result<(Rows, Schedule, ScheduleSpec), CacheError> {
        let _mode = self.mode.hold().await?;
        let before = self.rows().await?;
        let expr = match (&req.cron, &before.builtin) {
            (Some(expr), _) => expr.clone(),
            (None, Some(builtin)) => builtin.spec.expr.as_str().to_owned(),
            (None, None) => {
                return Err(invalid(
                    Field::Cron,
                    ScheduleReason::CronRequired,
                    "--cron is required when there is no built-in row to take the expression from",
                ));
            }
        };
        let new_spec = spec(
            SyncRow::Python,
            &expr,
            payload.timeout().get(),
            &SyncPayload::Python(payload.clone()),
        )?;
        let flags = before.others(SyncRow::Python, true);
        let written = self
            .scheduler
            .replace_and_set_enabled_async(new_spec.clone(), true, flags)
            .await
            .map_err(schedule_store_failed)?;
        Ok((before, written, new_spec))
    }

    /// Records the immediate run's outcome on the row it ran, as sc-cron
    /// records a worker run's (`last_run_at`, `last_outcome`).
    async fn record_on_row(
        &self,
        run_id: &bd_cache_model::RunId,
        written: &Schedule,
        result: &Result<PythonRun, CacheError>,
    ) {
        let store = Arc::clone(&self.store);
        let (id, generation) = (written.id, written.generation);
        let outcome = outcome_of(result);
        let at = sc_cron::Timestamp::from_datetime(self.clock.now());
        let recorded = blocking(move || {
            store
                .record_run(id, generation, at, &outcome)
                .map_err(schedule_store_failed)
        })
        .await;
        if let Err(e) = recorded {
            tracing::warn!(
                row = SyncRow::Python.as_str(),
                run_id = %run_id,
                code = e.code(),
                detail = e.detail(),
                "the immediate run's outcome could not be recorded on the Python row"
            );
        }
    }

    /// Restores `before` after a failed immediate run, under the mode lock,
    /// in one store call; `false` (logged) when the restore failed or another
    /// command replaced the Python row since `written` (its state stands).
    async fn restore(
        &self,
        run_id: &bd_cache_model::RunId,
        before: &Rows,
        written: &Schedule,
        new_spec: ScheduleSpec,
    ) -> bool {
        let restored: Result<bool, CacheError> = async {
            let _mode = self.mode.hold().await?;
            let now = self.rows().await?;
            if now.python.as_ref().map(|r| r.generation) != Some(written.generation) {
                tracing::warn!(
                    row = SyncRow::Python.as_str(),
                    run_id = %run_id,
                    "another command changed the Python row during the run; its state is kept, not restored"
                );
                return Ok(false);
            }
            let builtin_flag = match (&before.builtin, &now.builtin) {
                (Some(row), Some(_)) => vec![(schedule_name(SyncRow::Builtin), row.enabled)],
                _ => Vec::new(),
            };
            let (spec, enabled) = match &before.python {
                Some(row) => (row.spec.clone(), row.enabled),
                None => (new_spec, false),
            };
            self.scheduler
                .replace_and_set_enabled_async(spec, enabled, builtin_flag)
                .await
                .map_err(schedule_store_failed)?;
            Ok(true)
        }
        .await;
        restored.unwrap_or_else(|e| {
            tracing::error!(
                row = SyncRow::Python.as_str(),
                run_id = %run_id,
                code = e.code(),
                detail = e.detail(),
                "the state before sync use-python could not be restored"
            );
            false
        })
    }

    async fn release(&self, lease: TickLease) {
        let cache = Arc::clone(&self.cache);
        let _released = blocking(move || {
            let mut guard = hold(&cache)?;
            Ok(lease.release(&mut guard))
        })
        .await;
    }

    /// `sync use-builtin` (REQ-BDC-0724): no lease; under the mode lock, the
    /// built-in row enabled and the Python row disabled in one store call.
    ///
    /// # Errors
    ///
    /// `SCHEDULE_INVALID` (`no_builtin_row`) with no built-in row;
    /// `STORE_FAILED`.
    pub async fn use_builtin(&self) -> Result<UseBuiltinReply, CacheError> {
        let _mode = self.mode.hold().await?;
        let rows = self.rows().await?;
        if rows.builtin.is_none() {
            return Err(no_builtin_row());
        }
        let mut flags = vec![(schedule_name(SyncRow::Builtin), true)];
        flags.extend(rows.others(SyncRow::Builtin, true));
        self.scheduler
            .set_enabled_many_async(flags)
            .await
            .map_err(schedule_store_failed)?;
        Ok(UseBuiltinReply {
            row: SyncRow::Builtin.as_str(),
            mode: SyncMode::Builtin,
        })
    }

    /// An on-demand tick: the built-in row's code with its payload (or the
    /// default one when there is none), under the lease (REQ-BDC-0720); a
    /// signal (`stop`) stops it and kills its hook.
    ///
    /// Scheduled times come only from the rows: this changes none.
    ///
    /// # Errors
    ///
    /// [`SyncError::PythonSyncEnabled`] while the Python row is enabled (no
    /// bd process starts); the run's errors otherwise.
    pub async fn tick(&self, stop: StopFlag) -> Result<BuiltinReply, SyncError> {
        let rows = self.rows().await?;
        if rows.mode() == SyncMode::Python {
            return Err(SyncError::PythonSyncEnabled);
        }
        let (payload, period) = match &rows.builtin {
            Some(row) => match SyncPayload::decode(&row.spec.payload)? {
                SyncPayload::Builtin(p) => (p, row_period(row)),
                SyncPayload::Python(_) => {
                    return Err(invalid(
                        Field::Payload,
                        ScheduleReason::OutOfRange,
                        "the built-in row holds a Python payload",
                    )
                    .into());
                }
            },
            None => (BuiltinPayload::default(), None),
        };
        let ctx = RunContext {
            period,
            ..RunContext::on_demand(&self.files, stop)
        };
        let run_id = {
            let cache = Arc::clone(&self.cache);
            blocking(move || run_id_now(&*hold(&cache)?.clock())).await?
        };
        Ok(run_builtin(&self.cache, &payload, &ctx, run_id).await?)
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
                    "{detail}; the state before the command could NOT be restored: switch to the built-in row to recover"
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

#[cfg(test)]
mod tests {
    use serde_json::json;

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

    /// A row written enabled disables the other existing row in the same
    /// call; written disabled, or with no other row, it lists nothing.
    #[test]
    fn a_row_written_enabled_disables_the_other_row() {
        let rows = Rows {
            builtin: Some(row("* * * * * *", "2026-10-26T10:00:00Z")),
            python: None,
        };
        assert_eq!(rows.others(SyncRow::Python, true).len(), 1);
        assert!(rows.others(SyncRow::Python, false).is_empty());
        assert!(rows.others(SyncRow::Builtin, true).is_empty());
    }
}
