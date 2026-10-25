//! A cache file's cron table, for front ends (`bdc cache schedule`, `bdc cache
//! tick`; REQ-BDC-0701, 0703; ANC-002, ANC-008).
//!
//! [`CronTable`] opens the `sc-cron` store of a cache file
//! ([`cron_db_path`]) on the system clock and manages its one built-in sync
//! row; [`tick_once`] runs that row once, on demand, with the row's budget
//! and post-sync hook (the schedule stays in the table). Front ends depend
//! on this crate only, never on `sc-cron` itself.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sc_beads::Deadline;
use sc_cron::{Schedule, Scheduler, SqliteScheduleStore, SystemClock, SystemJitter};
use serde::Serialize;

use crate::compose::Composed;
use crate::control::StopFlag;
use crate::hook::HookRun;
use crate::job::{BuiltInRun, JobRun, SyncJob};
use crate::report::SyncRun;
use crate::run::DEFAULT_BUDGET;
use crate::schedule::{
    HookSpec, RowPayload, ScheduleError, builtin_spec, clear_builtin, cron_db_path, find_builtin,
    register_builtin, set_builtin, set_builtin_enabled,
};

/// The built-in sync row as a front end shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuiltInRow {
    /// The row's name.
    pub name: String,
    /// Its cron expression.
    pub cron: String,
    /// Its timeout, the run's budget (REQ-BDC-0703).
    pub timeout_ms: u64,
    /// Whether it fires (ANC-006).
    pub enabled: bool,
    /// The post-sync hook it names.
    pub post_sync_hook: Option<HookSpec>,
    /// When it next runs.
    pub next_run_at: Option<String>,
    /// When its last run finished.
    pub last_run_at: Option<String>,
    /// How its last run went: `ok`, `timed_out`, or `refused: ...` / `failed: ...`.
    pub last_outcome: Option<String>,
}

impl BuiltInRow {
    fn of(row: &Schedule) -> Self {
        let post_sync_hook = match RowPayload::of(&row.spec.payload) {
            Ok(Some(RowPayload::BuiltIn { post_sync_hook })) => post_sync_hook,
            _ => None,
        };
        Self {
            name: row.spec.name.as_str().to_owned(),
            cron: row.spec.expr.as_str().to_owned(),
            timeout_ms: u64::try_from(row.spec.timeout.get().as_millis()).unwrap_or(u64::MAX),
            enabled: row.enabled,
            post_sync_hook,
            next_run_at: row.next_run_at.as_ref().map(ToString::to_string),
            last_run_at: row.last_run_at.as_ref().map(ToString::to_string),
            last_outcome: row.last_outcome.as_ref().map(|o| match o {
                sc_cron::RunOutcome::Ok => "ok".to_owned(),
                sc_cron::RunOutcome::TimedOut => "timed_out".to_owned(),
                sc_cron::RunOutcome::Refused(m) => format!("refused: {m}"),
                sc_cron::RunOutcome::Failed(m) => format!("failed: {m}"),
            }),
        }
    }
}

/// The cron table of one cache file, on the system clock.
pub struct CronTable {
    scheduler: Scheduler<SqliteScheduleStore, SystemClock, SystemJitter>,
}

impl std::fmt::Debug for CronTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CronTable").finish_non_exhaustive()
    }
}

impl CronTable {
    /// Opens (creating it when missing) the cron table of the cache file at `cache_path`.
    ///
    /// # Errors
    ///
    /// [`ScheduleError::Cron`] when the store cannot be opened.
    pub fn open(cache_path: &Path) -> Result<Self, ScheduleError> {
        let store = SqliteScheduleStore::open(cron_db_path(cache_path))?;
        Ok(Self {
            scheduler: Scheduler::new(Arc::new(store), Arc::new(SystemClock), Arc::new(SystemJitter)),
        })
    }

    /// Opens the cron table when the file exists; `None` otherwise (nothing is created).
    ///
    /// # Errors
    ///
    /// As [`CronTable::open`].
    pub fn open_existing(cache_path: &Path) -> Result<Option<Self>, ScheduleError> {
        if cron_db_path(cache_path).exists() {
            Self::open(cache_path).map(Some)
        } else {
            Ok(None)
        }
    }

    /// The built-in sync row, if any.
    ///
    /// # Errors
    ///
    /// [`ScheduleError::Cron`] when the store cannot be read.
    pub fn builtin(&self) -> Result<Option<BuiltInRow>, ScheduleError> {
        Ok(find_builtin(self.scheduler.store().as_ref())?.map(|r| BuiltInRow::of(&r)))
    }

    /// Creates the built-in row; with `replace`, replaces the one there is
    /// (a disabled row stays disabled). Without it, a second built-in row is
    /// refused (REQ-BDC-0701, ANC-008).
    ///
    /// # Errors
    ///
    /// [`ScheduleError::SecondBuiltIn`] without `replace` when one exists;
    /// the errors of [`builtin_spec`].
    pub fn set_builtin(
        &self,
        cron: Option<&str>,
        timeout: Option<Duration>,
        post_sync_hook: Option<HookSpec>,
        replace: bool,
    ) -> Result<BuiltInRow, ScheduleError> {
        let spec = builtin_spec(cron, timeout, post_sync_hook)?;
        let row = if replace {
            set_builtin(&self.scheduler, spec)?
        } else {
            register_builtin(&self.scheduler, spec)?
        };
        Ok(BuiltInRow::of(&row))
    }

    /// Flips only the `enabled` column of the built-in row (ANC-006).
    ///
    /// # Errors
    ///
    /// [`ScheduleError::NoBuiltIn`] when there is none.
    pub fn set_builtin_enabled(&self, enabled: bool) -> Result<BuiltInRow, ScheduleError> {
        set_builtin_enabled(&self.scheduler, enabled).map(|r| BuiltInRow::of(&r))
    }

    /// Ends the built-in row; `None` when there was none.
    ///
    /// # Errors
    ///
    /// [`ScheduleError::Cron`] when it cannot be ended.
    pub fn clear_builtin(&self) -> Result<Option<BuiltInRow>, ScheduleError> {
        Ok(clear_builtin(&self.scheduler)?.map(|r| BuiltInRow::of(&r)))
    }
}

/// What one on-demand run of the built-in sync did (`bdc cache tick`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TickReport {
    /// The budget the run had: the given one, else the row's timeout, else
    /// [`DEFAULT_BUDGET`] (REQ-BDC-0703).
    pub budget_ms: u64,
    /// The built-in row the run used, when the table has one.
    pub schedule: Option<BuiltInRow>,
    /// The run: each scope's result and error.
    pub run: SyncRun,
    /// The post-sync hook's run, when the row names a hook.
    pub hook: Option<HookRun>,
}

/// Runs the built-in sync of `composed` once, on demand (`bdc cache tick`).
///
/// The budget is `budget`, else the built-in row's timeout, else
/// [`DEFAULT_BUDGET`]; the row's post-sync hook runs after the sync with its
/// timeout capped by the time left (NFR-BDC-0004). The schedule stays in the
/// table, and a disabled row is run too: a run on demand is the operator's.
/// `stop` ends the run early (a signal).
///
/// # Errors
///
/// [`TickError::Schedule`] when the cron table cannot be read;
/// [`TickError::Cache`] for a store or cache error of the run;
/// [`TickError::Runtime`] when the run's runtime cannot be built.
pub fn tick_once(
    composed: Composed,
    budget: Option<Duration>,
    stop: StopFlag,
) -> Result<TickReport, TickError> {
    let table = CronTable::open_existing(&composed.cache_path).map_err(TickError::Schedule)?;
    let schedule = match &table {
        Some(t) => t.builtin().map_err(TickError::Schedule)?,
        None => None,
    };
    let budget = budget
        .or_else(|| schedule.as_ref().map(|s| Duration::from_millis(s.timeout_ms)))
        .unwrap_or(DEFAULT_BUDGET);
    let hook = schedule.as_ref().and_then(|s| s.post_sync_hook.clone());
    let clock = composed.cache.clock();
    let deadline = Deadline::after(&*clock, budget).instant();
    let job = SyncJob::new(composed.cache, composed.repo_root, Arc::new(SystemClock)).with_stop(stop);
    let BuiltInRun { sync, hook: hook_run } = job
        .run_builtin_blocking(hook.as_ref(), deadline)
        .map_err(TickError::Runtime)?;
    let run = match sync {
        JobRun::Done(run) => *run,
        JobRun::Failed { error, .. } => return Err(TickError::Cache(error)),
        JobRun::Lost { error, .. } => return Err(TickError::Lost(error)),
    };
    Ok(TickReport {
        budget_ms: u64::try_from(budget.as_millis()).unwrap_or(u64::MAX),
        schedule,
        run,
        hook: hook_run,
    })
}

/// An on-demand run could not be done.
#[derive(Debug)]
#[non_exhaustive]
pub enum TickError {
    /// The cron table could not be read.
    Schedule(ScheduleError),
    /// The run failed with a store or cache error.
    Cache(bd_cache_model::CacheError),
    /// The cache was lost (a panic; never on an on-demand run's fresh cache).
    Lost(crate::job::CacheLost),
    /// The runtime of the run could not be built.
    Runtime(std::io::Error),
}

impl std::fmt::Display for TickError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schedule(e) => std::fmt::Display::fmt(e, f),
            Self::Cache(_) => f.write_str("the sync run failed"),
            Self::Lost(e) => std::fmt::Display::fmt(e, f),
            Self::Runtime(_) => f.write_str("the sync run's runtime could not be built"),
        }
    }
}

impl std::error::Error for TickError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Schedule(e) => Some(e),
            Self::Cache(e) => Some(e),
            Self::Lost(e) => Some(e),
            Self::Runtime(e) => Some(e),
        }
    }
}
