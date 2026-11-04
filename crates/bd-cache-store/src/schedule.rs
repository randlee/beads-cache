//! [`CacheScheduleStore`]: sc-cron's [`ScheduleStore`] over the cache file's
//! one connection (ADR-BDC-0014, NFR-BDC-0007 as amended, REQ-CRON-0017).
//!
//! sc-cron's standalone store owns its own file, connection and
//! `user_version`. The sync rows must live in the cache file instead, which
//! stays at `user_version` 3 with one connection per process. So this store
//! keeps sc-cron's `sc_cron_schedule` table (its version-2 columns and checks,
//! created on open as an additive extension, ADR-STORE-0006) in the cache
//! file, and runs on the connection [`SqliteStore`](crate::SqliteStore) holds:
//! the two share one mutex-guarded object that owns the connection and the
//! process's claim on the file, so the claim is released only when the last
//! of them drops. The version of sc-cron's columns in the file is the `meta`
//! row `sc_cron_schedule_version`, checked when the view is made, never
//! `user_version`.
//!
//! The SQL is sc-cron's own: every method behaves as `SqliteScheduleStore`'s
//! does, and every write runs in one `BEGIN IMMEDIATE` transaction. The
//! store's contract tests (`tests/schedule_store.rs`) check the cases of
//! REQ-CRON-0006, 0007, 0008, 0014 and 0017 against it.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Row, ToSql, Transaction, TransactionBehavior, params,
};
use sc_beads::logging::tracing;
use sc_cron::{
    CronError, CronExpr, Generation, HookEnvironment, HookTarget, MAX_PAYLOAD_BYTES, NewSchedule,
    RunOutcome, RunTimeout, Schedule, ScheduleId, ScheduleLimits, ScheduleName, ScheduleSpec,
    ScheduleStore, StoreError, StoreErrorKind, Timestamp,
};

use crate::schema::{SC_CRON_SCHEDULE_VERSION, SC_CRON_SCHEDULE_VERSION_KEY};
use crate::store::Shared;

const TABLE: &str = "sc_cron_schedule";

const COLUMNS: &str = "id, generation, name, expr, timeout_ms, jitter_ms, payload, created_at, \
     next_run_at, last_run_at, last_outcome, ended_at, stop_reason, enabled";

/// sc-cron's [`ScheduleStore`] inside the cache file, on the cache's one connection.
///
/// Made by [`SqliteStore::schedule_store`](crate::SqliteStore::schedule_store);
/// `Send + Sync`, so a `sc_cron::Scheduler` can hold it in an `Arc` and call
/// it from its worker. Each call locks the shared connection for its one
/// statement or transaction, as every `SqliteStore` call does. The longest
/// such hold is a full-size `ingest_scope` (about 1.3 s for 50 000 rows on a
/// debug build, `tests/phase_b_store.rs`); a schedule call waits for it on the
/// mutex, which sc-cron's worker runs on its blocking pool.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use bd_cache_model::RepoConfig;
/// use bd_cache_store::SqliteStore;
/// use sc_cron::ScheduleStore;
///
/// let dir = tempfile::TempDir::new()?;
/// let root = dir.path();
/// std::fs::create_dir_all(root.join(".beads"))?;
/// std::fs::write(
///     root.join(".beads").join("metadata.json"),
///     r#"{"dolt_database": "dev", "project_id": "p-1"}"#,
/// )?;
/// let store = SqliteStore::open_with(root, &RepoConfig::default(), None)?;
/// let schedules = store.schedule_store()?;
/// assert!(schedules.list_active()?.is_empty());
/// // The view keeps the file's claim: a second open in this process is refused.
/// drop(store);
/// let again = SqliteStore::open_with(root, &RepoConfig::default(), None);
/// assert_eq!(again.unwrap_err().code(), "BD_CACHE.STORE_IN_USE");
/// # drop(schedules);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct CacheScheduleStore {
    shared: Arc<Shared>,
    limits: ScheduleLimits,
    shutdown: Option<crate::ScheduleShutdownControl>,
    // Row ids already warned about as corrupt, so a bad row is reported once
    // per store rather than on every worker reload.
    warned: Mutex<HashSet<i64>>,
}

impl CacheScheduleStore {
    /// Makes a worker-only view sharing this connection and its file claim.
    ///
    /// The returned control bounds this view's calls during shutdown without
    /// changing public schedule reads, health scopes or normal cache sessions.
    #[must_use]
    pub fn worker_view(&self) -> (Self, crate::ScheduleShutdownControl) {
        let control = crate::ScheduleShutdownControl::over(Arc::as_ptr(&self.shared) as usize);
        (
            Self {
                shared: Arc::clone(&self.shared),
                limits: self.limits,
                shutdown: Some(control.clone()),
                warned: Mutex::new(HashSet::new()),
            },
            control,
        )
    }

    fn worker_scope(&self) -> Option<crate::schedule_budget::Scope> {
        self.shutdown
            .as_ref()
            .and_then(|control| control.enter(Arc::as_ptr(&self.shared) as usize))
    }

    /// The view over `shared`, after checking the file's
    /// `sc_cron_schedule_version` row (ADR-BDC-0014).
    pub(crate) fn over(shared: Arc<Shared>) -> Result<Self, CronError> {
        let version: Option<String> = shared
            .session_lock()
            .map_err(bounded_session_error)?
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [SC_CRON_SCHEDULE_VERSION_KEY],
                |r| r.get(0),
            )
            .optional()
            .map_err(|error| store_error("failed to read the schedule table version", error))?;
        let expected = SC_CRON_SCHEDULE_VERSION.to_string();
        if version.as_deref() != Some(expected.as_str()) {
            return Err(StoreError::new(
                StoreErrorKind::SchemaMismatch,
                format!(
                    "{}: meta {SC_CRON_SCHEDULE_VERSION_KEY} is {}, this code serves version \
                     {expected}",
                    shared.path().display(),
                    version.as_deref().unwrap_or("absent")
                ),
            )
            .into());
        }
        Ok(Self {
            shared,
            limits: ScheduleLimits::with_max_schedule_rows(std::num::NonZeroUsize::new(1024)),
            shutdown: None,
            warned: Mutex::new(HashSet::new()),
        })
    }

    /// Warns about a skipped corrupt row, once per row id for this store.
    fn warn_corrupt(&self, id: Option<i64>, name: Option<&str>, error: &CronError) {
        if let Some(id) = id {
            let mut warned = self.warned.lock().unwrap_or_else(PoisonError::into_inner);
            if !warned.insert(id) {
                return;
            }
        }
        tracing::warn!(subsystem = "bd_cache_store", action = "skip_corrupt_row", id = ?id,
            name = ?name, %error, "skipping corrupt schedule row; other schedules keep running");
    }

    /// Runs one guarded `UPDATE` and reports whether it changed the row.
    fn guarded_update(
        &self,
        sql: &str,
        id: ScheduleId,
        generation: Generation,
        values: &[&dyn ToSql],
    ) -> Result<bool, CronError> {
        let Ok(id) = i64::try_from(id.get()) else {
            // No stored row can carry an id above i64::MAX.
            return Ok(false);
        };
        let generation = generation.get();
        let mut all: Vec<&dyn ToSql> = vec![&id, &generation];
        all.extend_from_slice(values);
        let changed = self
            .shared
            .session_lock()
            .map_err(bounded_session_error)?
            .conn
            .execute(sql, all.as_slice())
            .map_err(|error| store_error(&format!("failed to update {TABLE} id {id}"), error))?;
        Ok(changed == 1)
    }
}

/// `BEGIN IMMEDIATE` on the shared connection.
fn begin(connection: &mut Connection) -> Result<Transaction<'_>, CronError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| store_error("failed to begin transaction", error))
}

impl ScheduleStore for CacheScheduleStore {
    fn limits(&self) -> ScheduleLimits {
        self.limits
    }
    fn create(&self, schedule: &NewSchedule, created_at: Timestamp) -> Result<Schedule, CronError> {
        let _worker_scope = self.worker_scope();
        crate::health::reject_mutation(self.shared.health_deadline())
            .map_err(bounded_session_error)?;
        let columns = Columns::new(schedule, created_at)?;
        let mut inner = self.shared.session_lock().map_err(bounded_session_error)?;
        let transaction = begin(&mut inner.conn)?;
        let existing = transaction
            .query_row(
                "SELECT id, generation, ended_at IS NULL FROM sc_cron_schedule WHERE name = ?1",
                [schedule.spec.name.as_str()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| store_error(&format!("failed to read {TABLE}"), error))?;
        let id = match existing {
            Some((_, _, true)) => {
                return Err(CronError::AlreadyActive(schedule.spec.name.clone()));
            }
            Some((id, generation, false)) => {
                check_limit(&transaction, self.limits)?;
                columns.rewrite(&transaction, id, generation, true)?;
                id
            }
            None => {
                check_limit(&transaction, self.limits)?;
                columns.insert(&transaction, schedule.spec.name.as_str(), true)?
            }
        };
        let row = select_one(&transaction, id)?.ok_or_else(|| missing(id, "create"))?;
        transaction
            .commit()
            .map_err(|error| store_error("failed to commit", error))?;
        Ok(row)
    }

    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError> {
        let _worker_scope = self.worker_scope();
        let Ok(id) = i64::try_from(id.get()) else {
            return Ok(None);
        };
        select_one(
            &self
                .shared
                .session_lock()
                .map_err(bounded_session_error)?
                .conn,
            id,
        )
    }

    fn list_active(&self) -> Result<Vec<Schedule>, CronError> {
        let _worker_scope = self.worker_scope();
        let rows = {
            let inner = self.shared.session_lock().map_err(bounded_session_error)?;
            let mut statement = inner
                .conn
                .prepare(&format!(
                    "SELECT {COLUMNS} FROM sc_cron_schedule WHERE ended_at IS NULL
                     ORDER BY created_at, id"
                ))
                .map_err(|error| store_error(&format!("failed to read {TABLE}"), error))?;
            statement
                .query_map([], |row| {
                    // Keep the id even when the rest of the row cannot be read.
                    Ok(RawRow::read(row).map_err(|error| (row.get::<_, i64>(0).ok(), error)))
                })
                .and_then(Iterator::collect::<Result<Vec<_>, _>>)
                .map_err(|error| store_error(&format!("failed to read {TABLE}"), error))?
        };
        let mut schedules = Vec::with_capacity(rows.len());
        for row in rows {
            crate::health::check_shutdown(
                crate::schedule_budget::current(Arc::as_ptr(&self.shared) as usize).as_ref(),
            )
            .map_err(bounded_session_error)?;
            crate::health::check_decode().map_err(bounded_session_error)?;
            crate::health::check(self.shared.health_deadline()).map_err(bounded_session_error)?;
            let (id, name, error) = match row {
                Ok(raw) => {
                    let (id, name) = (raw.id, raw.name.clone());
                    match raw.decode() {
                        Ok(schedule) => {
                            schedules.push(schedule);
                            continue;
                        }
                        Err(error) => (Some(id), Some(name), error),
                    }
                }
                Err((id, error)) => (
                    id,
                    None,
                    store_error(&format!("failed to read {TABLE}"), error),
                ),
            };
            // A row that cannot be decoded must not stop the others: skip it.
            // Any other failure (busy, I/O) still fails the whole call.
            match &error {
                CronError::Store(store) if store.kind() == StoreErrorKind::Corrupt => {
                    self.warn_corrupt(id, name.as_deref(), &error);
                }
                _ => return Err(error),
            }
        }
        Ok(schedules)
    }

    fn record_run(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        outcome: &RunOutcome,
    ) -> Result<bool, CronError> {
        let _worker_scope = self.worker_scope();
        crate::health::reject_mutation(self.shared.health_deadline())
            .map_err(bounded_session_error)?;
        self.guarded_update(
            "UPDATE sc_cron_schedule SET last_run_at = ?3, last_outcome = ?4
             WHERE id = ?1 AND generation = ?2 AND ended_at IS NULL",
            id,
            generation,
            &[&at.to_string(), &outcome.to_string()],
        )
    }

    fn set_next(
        &self,
        id: ScheduleId,
        generation: Generation,
        next_run_at: Timestamp,
    ) -> Result<bool, CronError> {
        let _worker_scope = self.worker_scope();
        crate::health::reject_mutation(self.shared.health_deadline())
            .map_err(bounded_session_error)?;
        self.guarded_update(
            "UPDATE sc_cron_schedule SET next_run_at = ?3
             WHERE id = ?1 AND generation = ?2 AND ended_at IS NULL AND enabled = 1",
            id,
            generation,
            &[&next_run_at.to_string()],
        )
    }

    fn stop(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        reason: &str,
    ) -> Result<bool, CronError> {
        let _worker_scope = self.worker_scope();
        crate::health::reject_mutation(self.shared.health_deadline())
            .map_err(bounded_session_error)?;
        self.guarded_update(
            "UPDATE sc_cron_schedule SET ended_at = ?3, stop_reason = ?4, next_run_at = NULL
             WHERE id = ?1 AND generation = ?2 AND ended_at IS NULL",
            id,
            generation,
            &[&at.to_string(), &reason],
        )
    }

    fn set_enabled(&self, name: &ScheduleName, enabled: bool) -> Result<bool, CronError> {
        let _worker_scope = self.worker_scope();
        crate::health::reject_mutation(self.shared.health_deadline())
            .map_err(bounded_session_error)?;
        // Flips only the `enabled` column, in place (REQ-CRON-0014).
        let changed = self
            .shared
            .session_lock()
            .map_err(bounded_session_error)?
            .conn
            .execute(
                "UPDATE sc_cron_schedule SET enabled = ?2
                 WHERE name = ?1 AND ended_at IS NULL",
                params![name.as_str(), enabled],
            )
            .map_err(|error| store_error(&format!("failed to update {TABLE} {name}"), error))?;
        Ok(changed == 1)
    }

    fn disable(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        reason: &str,
    ) -> Result<bool, CronError> {
        let _worker_scope = self.worker_scope();
        crate::health::reject_mutation(self.shared.health_deadline())
            .map_err(bounded_session_error)?;
        self.guarded_update(
            "UPDATE sc_cron_schedule SET enabled = 0, last_run_at = ?3, last_outcome = ?4
             WHERE id = ?1 AND generation = ?2 AND ended_at IS NULL",
            id,
            generation,
            &[
                &at.to_string(),
                &RunOutcome::Refused(reason.to_owned()).to_string(),
            ],
        )
    }

    fn set_enabled_many(&self, flags: &[(ScheduleName, bool)]) -> Result<bool, CronError> {
        let _worker_scope = self.worker_scope();
        crate::health::reject_mutation(self.shared.health_deadline())
            .map_err(bounded_session_error)?;
        let mut inner = self.shared.session_lock().map_err(bounded_session_error)?;
        let transaction = begin(&mut inner.conn)?;
        // A missing name returns before the commit: the transaction rolls
        // back on drop, so no flag of the list changes (REQ-CRON-0017).
        if !set_flags(&transaction, flags)? {
            return Ok(false);
        }
        transaction
            .commit()
            .map_err(|error| store_error("failed to commit", error))?;
        Ok(true)
    }

    fn replace_and_set_enabled(
        &self,
        schedule: &NewSchedule,
        enabled: bool,
        created_at: Timestamp,
        flags: &[(ScheduleName, bool)],
    ) -> Result<Option<Schedule>, CronError> {
        let _worker_scope = self.worker_scope();
        crate::health::reject_mutation(self.shared.health_deadline())
            .map_err(bounded_session_error)?;
        let name = &schedule.spec.name;
        if flags.iter().any(|(listed, _)| listed == name) {
            return Err(CronError::InvalidSpec(format!(
                "the flags to set name the written schedule {name}"
            )));
        }
        let columns = Columns::new(schedule, created_at)?;
        let mut inner = self.shared.session_lock().map_err(bounded_session_error)?;
        let transaction = begin(&mut inner.conn)?;
        let existing = transaction
            .query_row(
                "SELECT id, generation FROM sc_cron_schedule WHERE name = ?1",
                [name.as_str()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(|error| store_error(&format!("failed to read {TABLE}"), error))?;
        // The row is written with its flag directly; it never passes through
        // another enabled state (REQ-CRON-0017).
        let id = if let Some((id, generation)) = existing {
            let active: bool = transaction
                .query_row(
                    "SELECT ended_at IS NULL FROM sc_cron_schedule WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )
                .map_err(|e| store_error("checking active row", e))?;
            if !active {
                check_limit(&transaction, self.limits)?;
            }
            columns.rewrite(&transaction, id, generation, enabled)?;
            id
        } else {
            check_limit(&transaction, self.limits)?;
            columns.insert(&transaction, name.as_str(), enabled)?
        };
        if !set_flags(&transaction, flags)? {
            return Ok(None);
        }
        let row = select_one(&transaction, id)?.ok_or_else(|| missing(id, "write"))?;
        transaction
            .commit()
            .map_err(|error| store_error("failed to commit", error))?;
        Ok(Some(row))
    }
}

fn bounded_session_error(error: bd_cache_model::CacheError) -> CronError {
    StoreError::new(StoreErrorKind::Other, "bounded cache schedule session")
        .with_source(error)
        .into()
}

fn check_limit(transaction: &Transaction<'_>, limits: ScheduleLimits) -> Result<(), CronError> {
    if let Some(limit) = limits.max_schedule_rows {
        let count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM sc_cron_schedule WHERE ended_at IS NULL",
                [],
                |r| r.get(0),
            )
            .map_err(|e| store_error("counting schedule rows", e))?;
        if count >= i64::try_from(limit.get()).unwrap_or(i64::MAX) {
            return Err(CronError::InvalidSpec(format!(
                "schedule row limit {} reached: current count {count}",
                limit.get()
            )));
        }
    }
    Ok(())
}

/// Sets each listed active schedule's flag inside `transaction`; `false` as
/// soon as a name has no active schedule (the caller then rolls back).
fn set_flags(
    transaction: &Transaction<'_>,
    flags: &[(ScheduleName, bool)],
) -> Result<bool, CronError> {
    for (name, enabled) in flags {
        let changed = transaction
            .execute(
                "UPDATE sc_cron_schedule SET enabled = ?2
                 WHERE name = ?1 AND ended_at IS NULL",
                params![name.as_str(), enabled],
            )
            .map_err(|error| store_error(&format!("failed to update {TABLE} {name}"), error))?;
        if changed != 1 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn missing(id: i64, after: &str) -> CronError {
    StoreError::new(
        StoreErrorKind::Other,
        format!("{TABLE} id {id}: missing after {after}"),
    )
    .into()
}

/// One schedule's column values, converted before any write.
struct Columns {
    expr: String,
    timeout_ms: i64,
    jitter_ms: i64,
    payload: String,
    created_at: String,
    next_run_at: String,
}

impl Columns {
    fn new(schedule: &NewSchedule, created_at: Timestamp) -> Result<Self, CronError> {
        let spec = &schedule.spec;
        if !matches!(spec.hook, HookTarget::Rust) || !spec.environment.is_empty() {
            return Err(CronError::InvalidSpec("cache schedule adapter supports Rust jobs with empty upstream hook environment; Python modules remain cache-owned supervised jobs".into()));
        }
        let payload = spec.payload.to_string();
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(CronError::InvalidSpec(format!(
                "payload is {} bytes; the limit is {MAX_PAYLOAD_BYTES}",
                payload.len()
            )));
        }
        Ok(Self {
            expr: spec.expr.as_str().to_owned(),
            timeout_ms: millis("timeout", spec.timeout.get())?,
            jitter_ms: millis("jitter", schedule.jitter)?,
            payload,
            created_at: created_at.to_string(),
            next_run_at: schedule.next_run_at.to_string(),
        })
    }

    fn insert(
        &self,
        transaction: &Transaction<'_>,
        name: &str,
        enabled: bool,
    ) -> Result<i64, CronError> {
        transaction
            .execute(
                "INSERT INTO sc_cron_schedule
                    (generation, name, expr, timeout_ms, jitter_ms, payload,
                     created_at, next_run_at, enabled)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    name,
                    self.expr,
                    self.timeout_ms,
                    self.jitter_ms,
                    self.payload,
                    self.created_at,
                    self.next_run_at,
                    enabled,
                ],
            )
            .map_err(|error| store_error(&format!("failed to insert {TABLE}"), error))?;
        Ok(transaction.last_insert_rowid())
    }

    /// Rewrites row `id` in place: generation plus one, every field from the
    /// schedule, run and end state cleared, and `enabled` as given.
    fn rewrite(
        &self,
        transaction: &Transaction<'_>,
        id: i64,
        generation: i64,
        enabled: bool,
    ) -> Result<(), CronError> {
        let next = u32::try_from(generation)
            .ok()
            .and_then(Generation::new)
            .and_then(Generation::next)
            .ok_or_else(|| {
                StoreError::new(
                    StoreErrorKind::Other,
                    format!("{TABLE} id {id}: generation exhausted"),
                )
            })?;
        transaction
            .execute(
                "UPDATE sc_cron_schedule SET generation = ?2, expr = ?3, timeout_ms = ?4,
                    jitter_ms = ?5, payload = ?6, created_at = ?7, next_run_at = ?8,
                    last_run_at = NULL, last_outcome = NULL, ended_at = NULL,
                    stop_reason = NULL, enabled = ?9
                 WHERE id = ?1",
                params![
                    id,
                    next.get(),
                    self.expr,
                    self.timeout_ms,
                    self.jitter_ms,
                    self.payload,
                    self.created_at,
                    self.next_run_at,
                    enabled,
                ],
            )
            .map_err(|error| store_error(&format!("failed to rewrite {TABLE} id {id}"), error))?;
        Ok(())
    }
}

fn millis(field: &str, value: Duration) -> Result<i64, CronError> {
    i64::try_from(value.as_millis())
        .map_err(|_overflow| CronError::InvalidSpec(format!("{field} is too large to store")))
}

/// Classifies a rusqlite error so callers can tell busy from corrupt, as
/// sc-cron's own store does.
fn store_error(context: &str, error: rusqlite::Error) -> CronError {
    let kind = match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => StoreErrorKind::Busy,
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => StoreErrorKind::Corrupt,
        Some(
            ErrorCode::CannotOpen
            | ErrorCode::SystemIoFailure
            | ErrorCode::DiskFull
            | ErrorCode::ReadOnly
            | ErrorCode::PermissionDenied,
        ) => StoreErrorKind::Unavailable,
        _ => match error {
            rusqlite::Error::FromSqlConversionFailure(..)
            | rusqlite::Error::InvalidColumnType(..)
            | rusqlite::Error::IntegralValueOutOfRange(..) => StoreErrorKind::Corrupt,
            _ => StoreErrorKind::Other,
        },
    };
    StoreError::new(kind, context).with_source(error).into()
}

fn select_one(connection: &Connection, id: i64) -> Result<Option<Schedule>, CronError> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM sc_cron_schedule WHERE id = ?1"),
            [id],
            RawRow::read,
        )
        .optional()
        .map_err(|error| store_error(&format!("failed to read {TABLE} id {id}"), error))?
        .map(RawRow::decode)
        .transpose()
}

/// Decodes `last_outcome` text written by `RunOutcome`'s `Display`.
fn decode_outcome(text: &str) -> Option<RunOutcome> {
    match text {
        "ok" => Some(RunOutcome::Ok),
        "timed_out" => Some(RunOutcome::TimedOut),
        _ => text
            .strip_prefix("refused: ")
            .map(|message| RunOutcome::Refused(message.to_owned()))
            .or_else(|| {
                text.strip_prefix("failed: ")
                    .map(|message| RunOutcome::Failed(message.to_owned()))
            }),
    }
}

/// One row as stored, before typed decoding.
struct RawRow {
    id: i64,
    generation: i64,
    name: String,
    expr: String,
    timeout_ms: i64,
    jitter_ms: i64,
    payload: String,
    created_at: String,
    next_run_at: Option<String>,
    last_run_at: Option<String>,
    last_outcome: Option<String>,
    ended_at: Option<String>,
    stop_reason: Option<String>,
    enabled: i64,
}

impl RawRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            generation: row.get(1)?,
            name: row.get(2)?,
            expr: row.get(3)?,
            timeout_ms: row.get(4)?,
            jitter_ms: row.get(5)?,
            payload: row.get(6)?,
            created_at: row.get(7)?,
            next_run_at: row.get(8)?,
            last_run_at: row.get(9)?,
            last_outcome: row.get(10)?,
            ended_at: row.get(11)?,
            stop_reason: row.get(12)?,
            enabled: row.get(13)?,
        })
    }

    /// Decodes every column; any malformed value is a `Corrupt` error naming the row.
    fn decode(self) -> Result<Schedule, CronError> {
        let id = self.id;
        let corrupt = |what: String| -> CronError {
            StoreError::new(StoreErrorKind::Corrupt, format!("{TABLE} id {id}: {what}")).into()
        };
        let field = |name: &str, error: CronError| corrupt(format!("{name}: {error}"));
        let stamp = |name: &str, text: &str| -> Result<Timestamp, CronError> {
            text.parse().map_err(|error| field(name, error))
        };
        let optional_stamp =
            |name: &str, text: Option<&str>| text.map(|text| stamp(name, text)).transpose();
        let duration = |value: i64, name: &str| {
            u64::try_from(value)
                .map(Duration::from_millis)
                .map_err(|_negative| corrupt(format!("{name} is negative")))
        };
        let payload = serde_json::from_str(&self.payload)
            .map_err(|error| corrupt(format!("payload is not JSON: {error}")))?;
        let last_outcome = self
            .last_outcome
            .as_deref()
            .map(|text| {
                decode_outcome(text).ok_or_else(|| corrupt(format!("unknown outcome {text:?}")))
            })
            .transpose()?;
        let timeout = duration(self.timeout_ms, "timeout_ms")?;
        Ok(Schedule {
            id: ScheduleId::new(
                u64::try_from(id).map_err(|_negative| corrupt("id is negative".into()))?,
            ),
            generation: u32::try_from(self.generation)
                .ok()
                .and_then(Generation::new)
                .ok_or_else(|| corrupt("generation is out of range".into()))?,
            spec: ScheduleSpec {
                name: ScheduleName::new(self.name).map_err(|error| field("name", error))?,
                expr: CronExpr::parse(&self.expr).map_err(|error| field("expr", error))?,
                timeout: RunTimeout::new(timeout).map_err(|error| field("timeout_ms", error))?,
                payload,
                hook: HookTarget::Rust,
                environment: HookEnvironment::new(),
            },
            jitter: duration(self.jitter_ms, "jitter_ms")?,
            created_at: stamp("created_at", &self.created_at)?,
            next_run_at: optional_stamp("next_run_at", self.next_run_at.as_deref())?,
            last_run_at: optional_stamp("last_run_at", self.last_run_at.as_deref())?,
            last_outcome,
            ended_at: optional_stamp("ended_at", self.ended_at.as_deref())?,
            stop_reason: self.stop_reason,
            enabled: match self.enabled {
                0 => false,
                1 => true,
                other => return Err(corrupt(format!("enabled is {other}, not 0 or 1"))),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use bd_cache_model::{CacheStore, ScopeId};
    use std::time::Instant;

    use super::*;
    use crate::location::{DatabaseIdentity, DoltDatabase, ProjectId};
    use crate::store::SqliteStore;

    mod protocol {
        include!("../tests/support/schedule_protocol.rs");
    }

    fn shutdown_store() -> (tempfile::TempDir, SqliteStore) {
        shutdown_store_at(Instant::now())
    }

    fn shutdown_store_at(started: Instant) -> (tempfile::TempDir, SqliteStore) {
        store_fixture_phase(started, "scratch-create-start");
        let dir = tempfile::tempdir().unwrap();
        store_fixture_phase(started, "store-open-start");
        let store = SqliteStore::open_at(
            &dir.path().join("shutdown.sqlite3"),
            &DatabaseIdentity {
                dolt_database: DoltDatabase::new("shutdown").unwrap(),
                project_id: ProjectId::new("shutdown").unwrap(),
            },
        )
        .unwrap();
        store_fixture_phase(started, "store-open-returned");
        (dir, store)
    }

    // Diagnostic receipts describe completed phases only; they never satisfy a
    // result assertion, alter a deadline or authorize skipping the real call.
    fn store_fixture_phase(started: Instant, phase: &str) {
        protocol::record(started, "main", phase, &serde_json::Value::Null).unwrap();
        eprintln!("RQA9 store phase={phase} elapsed={:?}", started.elapsed());
    }

    /// A deadline installed after a real mutex wait started must end that wait
    /// before the holder releases it. The error is not a dropped blocking task.
    #[test]
    fn shutdown_interrupts_an_already_contended_worker_read_only() {
        let Some(stall) = store_fixture_mode(
            "schedule::tests::shutdown_interrupts_an_already_contended_worker_read_only",
        )
        .unwrap() else {
            return;
        };
        let started = Instant::now();
        store_fixture_phase(started, "fixture-body-entered");
        let (_dir, store) = shutdown_store_at(started);
        store_fixture_phase(started, "schedule-view-start");
        let public = store.schedule_store().unwrap();
        store_fixture_phase(started, "schedule-view-returned");
        store_fixture_phase(started, "schedule-setup-complete");
        let (worker, control) = public.worker_view();
        let guard = store.inner();
        let (waiting, observed) = std::sync::mpsc::channel();
        control.observe_wait(waiting);
        let (completed, result) = std::sync::mpsc::channel();
        store_fixture_phase(started, "worker-spawn-start");
        let task = std::thread::spawn(move || {
            protocol::worker_started(started);
            let result = worker.list_active();
            protocol::worker_returned(started, &result, stall);
            stall_store_cleanup(stall, started);
            completed.send(result).unwrap();
        });
        let entered = observed.recv_timeout(Duration::from_secs(2));
        protocol::wait_result(started, entered);
        protocol::cancel(started, &control);
        store_fixture_phase(started, "cancellation-installed");
        let bounded = result.recv_timeout(Duration::from_millis(500));
        protocol::record(started, "main", "completion-result", &serde_json::json!({"timeout": matches!(bounded, Err(std::sync::mpsc::RecvTimeoutError::Timeout))})).unwrap();
        eprintln!(
            "RQA9 store phase=completion-receipt elapsed={:?} result={bounded:?}",
            started.elapsed()
        );
        if bounded.is_err() {
            eprintln!("store cleanup observation expired after 500ms");
        }
        // Always release and join before an assertion can unwind the test.
        drop(guard);
        store_fixture_phase(started, "worker-join-start");
        task.join().unwrap();
        store_fixture_phase(started, "worker-join-returned");
        entered.unwrap();
        assert!(
            bounded
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("shutdown")
        );
        assert!(public.list_active().unwrap().is_empty());
        assert!(
            store
                .health_read_control()
                .within(Instant::now() + Duration::from_secs(1), || public
                    .list_active()
                    .map_err(|error| bd_cache_model::CacheError::store_failed(
                        "schedule read",
                        error
                    )))
                .unwrap()
                .is_empty()
        );
    }

    /// A store write already in SQLite's busy handler observes the new deadline,
    /// rolls back, and restores connection settings for normal writes/health.
    #[test]
    fn shutdown_interrupts_busy_persistence_and_restores_the_shared_session() {
        let Some(stall) = store_fixture_mode(
            "schedule::tests::shutdown_interrupts_busy_persistence_and_restores_the_shared_session",
        )
        .unwrap() else {
            return;
        };
        let started = Instant::now();
        store_fixture_phase(started, "fixture-body-entered");
        let (dir, store) = shutdown_store_at(started);
        store_fixture_phase(started, "schedule-view-start");
        let public = store.schedule_store().unwrap();
        store_fixture_phase(started, "schedule-view-returned");
        let spec = ScheduleSpec::new(
            "shutdown",
            "@hourly",
            Duration::from_secs(1),
            serde_json::json!({}),
        )
        .unwrap();
        let now = chrono_time();
        store_fixture_phase(started, "schedule-create-start");
        let row = public
            .create(
                &NewSchedule {
                    spec,
                    jitter: Duration::ZERO,
                    next_run_at: now,
                },
                now,
            )
            .unwrap();
        store_fixture_phase(started, "schedule-setup-complete");
        let (worker, control) = public.worker_view();
        store_fixture_phase(started, "contender-open-start");
        let other = Connection::open(dir.path().join("shutdown.sqlite3")).unwrap();
        store_fixture_phase(started, "contender-lock-start");
        other.execute_batch("BEGIN IMMEDIATE").unwrap();
        store_fixture_phase(started, "contender-lock-acquired");
        let (waiting, observed) = std::sync::mpsc::channel();
        control.observe_wait(waiting);
        let (completed, result) = std::sync::mpsc::channel();
        let (id, generation) = (row.id, row.generation);
        store_fixture_phase(started, "worker-spawn-start");
        let task = std::thread::spawn(move || {
            protocol::worker_started(started);
            let result = worker.record_run(id, generation, now, &RunOutcome::Ok);
            protocol::worker_returned(started, &result, stall);
            stall_store_cleanup(stall, started);
            completed.send(result).unwrap();
        });
        let entered = observed.recv_timeout(Duration::from_secs(2));
        protocol::wait_result(started, entered);
        protocol::cancel(started, &control);
        store_fixture_phase(started, "cancellation-installed");
        let bounded = result.recv_timeout(Duration::from_millis(500));
        protocol::record(started, "main", "completion-result", &serde_json::json!({"timeout": matches!(bounded, Err(std::sync::mpsc::RecvTimeoutError::Timeout))})).unwrap();
        eprintln!(
            "RQA9 store phase=completion-receipt elapsed={:?} result={bounded:?}",
            started.elapsed()
        );
        if bounded.is_err() {
            eprintln!("store cleanup observation expired after 500ms");
        }
        store_fixture_phase(started, "contender-rollback-start");
        other.execute_batch("ROLLBACK").unwrap();
        store_fixture_phase(started, "contender-rollback-returned");
        store_fixture_phase(started, "worker-join-start");
        task.join().unwrap();
        store_fixture_phase(started, "worker-join-returned");
        entered.unwrap();
        assert!(bounded.unwrap().is_err());
        assert_eq!(
            public.get(id).unwrap().unwrap().last_outcome,
            None,
            "failed persistence wrote nothing"
        );
        assert!(
            public
                .record_run(id, generation, now, &RunOutcome::Ok)
                .unwrap()
        );
        assert_normal_session_settings(&store);
        assert_eq!(
            store
                .health_read_control()
                .within(Instant::now() + Duration::from_secs(1), || public
                    .list_active()
                    .map_err(|error| bd_cache_model::CacheError::store_failed(
                        "schedule read",
                        error
                    )))
                .unwrap()
                .len(),
            1
        );
    }

    fn assert_normal_session_settings(store: &SqliteStore) {
        let inner = store.inner();
        let busy: i64 = inner
            .conn
            .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            busy,
            i64::try_from(crate::BUSY_TIMEOUT.as_millis()).unwrap()
        );
        assert!(
            !inner
                .conn
                .query_row("PRAGMA query_only", [], |r| r.get::<_, bool>(0))
                .unwrap()
        );
    }

    /// The controller reaches SQL already executing, and a late signal cannot
    /// interrupt a subsequent ordinary writer after the worker session restores.
    #[test]
    fn shutdown_interrupts_a_running_sql_statement_and_restores_progress() {
        let Some(stall) = store_fixture_mode(
            "schedule::tests::shutdown_interrupts_a_running_sql_statement_and_restores_progress",
        )
        .unwrap() else {
            return;
        };
        let started = Instant::now();
        store_fixture_phase(started, "fixture-body-entered");
        let (_dir, store) = shutdown_store_at(started);
        store_fixture_phase(started, "schedule-view-start");
        let public = store.schedule_store().unwrap();
        store_fixture_phase(started, "schedule-view-returned");
        let now = chrono_time();
        let spec = ScheduleSpec::new(
            "shutdown",
            "@hourly",
            Duration::from_secs(1),
            serde_json::json!({}),
        )
        .unwrap();
        store_fixture_phase(started, "schedule-create-start");
        let row = public
            .create(
                &NewSchedule {
                    spec,
                    jitter: Duration::ZERO,
                    next_run_at: now,
                },
                now,
            )
            .unwrap();
        let interrupt = {
            let inner = store.inner();
            inner.conn.execute_batch("CREATE TRIGGER slow_worker BEFORE UPDATE ON sc_cron_schedule BEGIN
                SELECT (WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT sum(x) FROM n);
                END;").unwrap();
            inner.conn.get_interrupt_handle()
        };
        store_fixture_phase(started, "schedule-setup-complete");
        let (worker, control) = public.worker_view();
        let (waiting, observed) = std::sync::mpsc::channel();
        control.observe_wait(waiting);
        let (completed, result) = std::sync::mpsc::channel();
        let (id, generation) = (row.id, row.generation);
        store_fixture_phase(started, "worker-spawn-start");
        let task = std::thread::spawn(move || {
            protocol::worker_started(started);
            let result = worker.record_run(id, generation, now, &RunOutcome::Ok);
            protocol::worker_returned(started, &result, stall);
            stall_store_cleanup(stall, started);
            completed.send(result).unwrap();
        });
        let entered = observed.recv_timeout(Duration::from_secs(2));
        protocol::wait_result(started, entered);
        protocol::cancel(started, &control);
        store_fixture_phase(started, "cancellation-installed");
        let bounded = result.recv_timeout(Duration::from_millis(500));
        protocol::record(started, "main", "completion-result", &serde_json::json!({"timeout": matches!(bounded, Err(std::sync::mpsc::RecvTimeoutError::Timeout))})).unwrap();
        eprintln!(
            "RQA9 store phase=completion-receipt elapsed={:?} result={bounded:?}",
            started.elapsed()
        );
        if bounded.is_err() {
            eprintln!("store cleanup observation expired after 500ms");
        }
        // If a planted mutation omits cancellation, own and interrupt the same
        // connection before joining; the test still fails its bounded receipt.
        if bounded.is_err() {
            interrupt.interrupt();
        }
        store_fixture_phase(started, "worker-join-start");
        task.join().unwrap();
        store_fixture_phase(started, "worker-join-returned");
        entered.unwrap();
        assert!(bounded.unwrap().is_err());
        store
            .inner()
            .conn
            .execute_batch("DROP TRIGGER slow_worker")
            .unwrap();
        assert_eq!(public.get(id).unwrap().unwrap().last_outcome, None);
        assert!(
            public
                .record_run(id, generation, now, &RunOutcome::Ok)
                .unwrap()
        );
    }

    const STORE_FIXTURE_MODE: &str = "BD_CACHE_SCHEDULE_FIXTURE";
    const STORE_FIXTURES: [&str; 3] = [
        "schedule::tests::shutdown_interrupts_an_already_contended_worker_read_only",
        "schedule::tests::shutdown_interrupts_busy_persistence_and_restores_the_shared_session",
        "schedule::tests::shutdown_interrupts_a_running_sql_statement_and_restores_progress",
    ];

    fn store_fixture_mode(test: &str) -> std::io::Result<Option<protocol::Mode>> {
        if let Some(mode) = std::env::var_os(STORE_FIXTURE_MODE) {
            return protocol::Mode::parse(&mode).map(Some);
        }
        let mut child =
            crate::fixture_child::FixtureChild::spawn(test, STORE_FIXTURE_MODE, "normal")?;
        crate::fixture_child::assert_passed(&mut child)?;
        Ok(None)
    }

    fn stall_store_cleanup(stall: protocol::Mode, started: Instant) {
        if stall != protocol::Mode::Normal {
            eprintln!("stalled store cleanup after real worker call");
            protocol::record(
                started,
                "worker",
                "stall-marker-written",
                &serde_json::Value::Null,
            )
            .unwrap();
            loop {
                std::thread::park();
            }
        }
    }

    #[test]
    fn shutdown_fixture_supervisor_kills_and_reaps_each_stalled_cleanup() {
        for test in STORE_FIXTURES {
            let observation = protocol::observe(test, STORE_FIXTURE_MODE, "stalled").unwrap();
            observation.qualify();
        }
    }

    #[test]
    fn shutdown_fixture_classifies_post_return_marker_gap_as_failed_qualification() {
        let observation =
            protocol::observe(STORE_FIXTURES[1], STORE_FIXTURE_MODE, "post-return-gap").unwrap();
        let failure =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observation.qualify()));
        assert!(
            failure.is_err(),
            "planted missing marker must fail normal qualification"
        );
        assert_eq!(
            observation.classification(),
            "worker_returned_required_marker_missing"
        );
        observation.assert_real_return_and_reap();
        observation.assert_planted_return();
        // The direct negative command retains actual libtest exit101. The
        // ordinary outer regression passes only by proving that owned failure.
        if std::env::var_os("BD_CACHE_EXPOSE_PHASE_QUALIFICATION_FAILURE").is_some() {
            std::panic::resume_unwind(failure.unwrap_err());
        }
    }

    #[test]
    fn shutdown_fixture_supervisor_reaps_during_assertion_unwind() {
        use std::sync::atomic::Ordering;
        let child = crate::fixture_child::FixtureChild::spawn(
            STORE_FIXTURES[0],
            STORE_FIXTURE_MODE,
            "stalled",
        )
        .unwrap();
        assert!(
            child
                .wait_for_output(
                    "stalled store cleanup after real worker call",
                    Duration::from_secs(2)
                )
                .unwrap()
        );
        let receipt = child.cleanup_receipt();
        let started = std::time::Instant::now();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owner = child;
            panic!("injected parent assertion");
        }));
        assert!(failure.is_err());
        assert!(
            receipt.load(Ordering::SeqCst),
            "unwind did not reap the owned child"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        eprintln!(
            "FTQ-009 assertion-unwind child cleanup in {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn shutdown_fixture_observer_error_and_unwind_reap_owned_root() {
        use std::sync::atomic::Ordering;
        for panic in [false, true] {
            let mut child = crate::fixture_child::FixtureChild::spawn(
                STORE_FIXTURES[0],
                STORE_FIXTURE_MODE,
                "stalled",
            )
            .unwrap();
            assert!(
                child
                    .wait_for_output(
                        "stalled store cleanup after real worker call",
                        Duration::from_secs(2)
                    )
                    .unwrap()
            );
            let receipt = child.cleanup_receipt();
            let pid = child.process.id();
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                child.finish_observing(Duration::from_secs(5), |_, _, _, _| {
                    assert!(!panic, "injected phase observer panic");
                    Err(std::io::Error::other("injected phase observer error"))
                })
            }));
            if panic {
                assert!(failure.is_err());
            } else {
                assert_eq!(
                    failure.unwrap().unwrap_err().to_string(),
                    "injected phase observer error"
                );
            }
            assert!(
                receipt.load(Ordering::SeqCst),
                "observer failure did not reap root"
            );
            eprintln!(
                "RQA9_OWNER_CONTROL observer_panic={panic} root_pid={pid} actual_kill_wait_receipt=true before_driver_fallback"
            );
        }
    }

    #[test]
    fn shutdown_fixture_cleanup_failure_is_not_retried_or_a_successful_receipt() {
        use std::sync::atomic::Ordering;
        let mut child = crate::fixture_child::FixtureChild::spawn(
            STORE_FIXTURES[0],
            STORE_FIXTURE_MODE,
            "stalled",
        )
        .unwrap();
        assert!(
            child
                .wait_for_output(
                    "stalled store cleanup after real worker call",
                    Duration::from_secs(2)
                )
                .unwrap()
        );
        std::fs::write(
            child.scratch.path().join("owned-child-invalid.pid"),
            "invalid",
        )
        .unwrap();
        assert!(child.finish(Duration::ZERO).is_err());
        assert!(
            child.process.try_wait().unwrap().is_some(),
            "root was not reaped after receipt error"
        );
        assert!(!child.cleanup_receipt().load(Ordering::SeqCst));
        assert!(
            !child.cleanup_on_drop,
            "Drop must not retry incomplete cleanup"
        );
        assert!(
            child
                .finish(Duration::ZERO)
                .unwrap_err()
                .to_string()
                .contains("already attempted")
        );
        eprintln!(
            "RQA9_OWNER_CONTROL cleanup_error root_pid={} actual_root_wait=true cleanup_receipt=false repeat_attempt_refused=true before_driver_fallback",
            child.process.id()
        );
    }

    /// Management policy belongs to exactly one view/connection and is restored
    /// before returning an error or unwinding. It never overrides health policy.
    #[test]
    fn management_scope_is_view_specific_restores_on_error_and_unwind_and_keeps_health_read_only() {
        let (_dir, store) = shutdown_store();
        let public = store.schedule_store().unwrap();
        let (worker, control) = public.worker_view();
        let (other_view, other_control) = public.worker_view();
        let (_other_dir, other_store) = shutdown_store();
        let (other_connection, other_connection_control) =
            other_store.schedule_store().unwrap().worker_view();
        for control in [&control, &other_control, &other_connection_control] {
            control.finish_by(Instant::now());
        }
        {
            let _management = control.management_call();
            assert!(worker.list_active().unwrap().is_empty());
            assert!(
                other_view.list_active().is_err(),
                "same connection, different view still expires"
            );
            assert!(
                other_connection.list_active().is_err(),
                "other connection still expires"
            );
            let result: Result<_, bd_cache_model::CacheError> =
                store
                    .health_read_control()
                    .within(Instant::now() + Duration::from_secs(1), || {
                        worker
                            .set_enabled(&ScheduleName::new("absent").unwrap(), true)
                            .map_err(|error| {
                                bd_cache_model::CacheError::store_failed(
                                    "managed health write",
                                    error,
                                )
                            })
                    });
            let error = result.unwrap_err();
            assert!(format!("{error:?}").contains("read-only"), "{error:?}");
            assert!(
                store
                    .health_read_control()
                    .within(Instant::now(), || Ok::<_, bd_cache_model::CacheError>(()))
                    .is_err()
            );
        }
        assert!(
            worker.list_active().is_err(),
            "management error must not leave a worker exemption"
        );
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _management = control.management_call();
            assert!(worker.list_active().unwrap().is_empty());
            panic!("injected management panic");
        }));
        assert!(unwind.is_err());
        assert!(
            worker.list_active().is_err(),
            "unwind must not leave a worker exemption"
        );
        assert!(public.list_active().unwrap().is_empty());
        let inner = store.inner();
        assert!(
            !inner
                .conn
                .query_row("PRAGMA query_only", [], |r| r.get::<_, bool>(0))
                .unwrap()
        );
        assert_eq!(
            inner
                .conn
                .query_row("PRAGMA busy_timeout", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            i64::try_from(crate::BUSY_TIMEOUT.as_millis()).unwrap()
        );
    }

    fn chrono_time() -> sc_cron::Timestamp {
        "2026-11-03T10:00:00Z".parse().unwrap()
    }

    /// The schedule view and the store share the cache's one connection
    /// (ADR-BDC-0014, AC7): inside one transaction opened on the store's
    /// connection, a write through the store is seen through the view, and a
    /// write through the view is seen through the store, while a second
    /// connection sees neither. Planted negative: give the view a connection
    /// of its own, and neither uncommitted write is seen through the other.
    #[test]
    fn the_view_and_the_store_share_one_connection() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("one.sqlite3");
        let identity = DatabaseIdentity {
            dolt_database: DoltDatabase::new("db").unwrap(),
            project_id: ProjectId::new("p").unwrap(),
        };
        let store = SqliteStore::open_at(&path, &identity).unwrap();
        let view = store.schedule_store().unwrap();
        store
            .inner()
            .conn
            .execute_batch(
                "BEGIN; INSERT INTO sc_cron_schedule (generation, name, expr, timeout_ms, \
                 jitter_ms, payload, created_at, next_run_at) VALUES (1, 'seen', '@hourly', \
                 1000, 0, '{}', '2026-10-23T10:00:00.000000Z', '2026-10-23T11:00:00.000000Z');",
            )
            .unwrap();
        let through_view = view.list_active().unwrap();
        assert_eq!(
            through_view
                .iter()
                .map(|s| s.spec.name.as_str())
                .collect::<Vec<_>>(),
            ["seen"],
            "the store's uncommitted write is seen through the view"
        );
        view.shared
            .lock()
            .conn
            .execute("INSERT INTO scope(name, kind) VALUES ('lane', 'phase')", [])
            .unwrap();
        let lane = ScopeId::new("lane").unwrap();
        assert!(
            store.scope(&lane).unwrap().is_some(),
            "the view's uncommitted write is seen through the store"
        );
        let other = Connection::open(&path).unwrap();
        let rows: i64 = other
            .query_row("SELECT COUNT(*) FROM sc_cron_schedule", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0, "another connection sees neither write");
        drop(other);
        store.inner().conn.execute_batch("ROLLBACK").unwrap();
        assert!(view.list_active().unwrap().is_empty());
        assert!(store.scope(&lane).unwrap().is_none());
    }

    #[test]
    fn outcomes_round_trip_through_their_stored_text() {
        for outcome in [
            RunOutcome::Ok,
            RunOutcome::TimedOut,
            RunOutcome::Refused("quota: full".into()),
            RunOutcome::Failed(String::new()),
        ] {
            assert_eq!(decode_outcome(&outcome.to_string()), Some(outcome));
        }
        assert_eq!(decode_outcome("exit"), None);
    }
    #[test]
    fn explicit_row_limits_count_disabled_rows_and_refuse_restarts_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let identity = DatabaseIdentity {
            dolt_database: DoltDatabase::new("db").unwrap(),
            project_id: ProjectId::new("p").unwrap(),
        };
        let store = SqliteStore::open_at(&dir.path().join("limits.db"), &identity).unwrap();
        let mut view = store.schedule_store().unwrap();
        view.limits = ScheduleLimits::with_max_schedule_rows(std::num::NonZeroUsize::new(2));
        let at = "2026-11-01T00:00:00Z".parse().unwrap();
        let new = |name| NewSchedule {
            spec: ScheduleSpec::new(
                name,
                "@hourly",
                Duration::from_secs(1),
                serde_json::json!({}),
            )
            .unwrap(),
            jitter: Duration::ZERO,
            next_run_at: at,
        };
        let first = view.create(&new("first"), at).unwrap();
        view.create(&new("second"), at).unwrap();
        view.set_enabled(&ScheduleName::new("second").unwrap(), false)
            .unwrap();
        assert!(matches!(
            view.create(&new("third"), at),
            Err(CronError::InvalidSpec(_))
        ));
        view.stop(first.id, first.generation, at, "end").unwrap();
        view.create(&new("third"), at).unwrap();
        assert!(matches!(
            view.create(&new("first"), at),
            Err(CronError::InvalidSpec(_))
        ));
        assert!(matches!(
            view.replace_and_set_enabled(&new("first"), true, at, &[]),
            Err(CronError::InvalidSpec(_))
        ));
        assert_eq!(
            view.get(first.id).unwrap().unwrap().generation,
            first.generation
        );
        assert_eq!(view.list_active().unwrap().len(), 2);
        // Active replacement is allowed at the ceiling; unsupported upstream
        // target metadata is rejected before any row can be changed.
        assert!(
            view.replace_and_set_enabled(&new("second"), true, at, &[])
                .unwrap()
                .is_some()
        );
        let mut unsupported = new("second");
        unsupported.spec.hook = HookTarget::python("hook.py").unwrap();
        assert!(matches!(
            view.replace_and_set_enabled(&unsupported, true, at, &[]),
            Err(CronError::InvalidSpec(_))
        ));
    }
}
