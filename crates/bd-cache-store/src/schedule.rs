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
    CronError, CronExpr, Generation, MAX_PAYLOAD_BYTES, NewSchedule, RunOutcome, RunTimeout,
    Schedule, ScheduleId, ScheduleName, ScheduleSpec, ScheduleStore, StoreError, StoreErrorKind,
    Timestamp,
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
    // Row ids already warned about as corrupt, so a bad row is reported once
    // per store rather than on every worker reload.
    warned: Mutex<HashSet<i64>>,
}

impl CacheScheduleStore {
    /// The view over `shared`, after checking the file's
    /// `sc_cron_schedule_version` row (ADR-BDC-0014).
    pub(crate) fn over(shared: Arc<Shared>) -> Result<Self, CronError> {
        let version: Option<String> = shared
            .lock()
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
            .lock()
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
    fn create(&self, schedule: &NewSchedule, created_at: Timestamp) -> Result<Schedule, CronError> {
        let columns = Columns::new(schedule, created_at)?;
        let mut inner = self.shared.lock();
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
                columns.rewrite(&transaction, id, generation, true)?;
                id
            }
            None => columns.insert(&transaction, schedule.spec.name.as_str(), true)?,
        };
        let row = select_one(&transaction, id)?.ok_or_else(|| missing(id, "create"))?;
        transaction
            .commit()
            .map_err(|error| store_error("failed to commit", error))?;
        Ok(row)
    }

    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError> {
        let Ok(id) = i64::try_from(id.get()) else {
            return Ok(None);
        };
        select_one(&self.shared.lock().conn, id)
    }

    fn list_active(&self) -> Result<Vec<Schedule>, CronError> {
        let rows = {
            let inner = self.shared.lock();
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
        self.guarded_update(
            "UPDATE sc_cron_schedule SET ended_at = ?3, stop_reason = ?4, next_run_at = NULL
             WHERE id = ?1 AND generation = ?2 AND ended_at IS NULL",
            id,
            generation,
            &[&at.to_string(), &reason],
        )
    }

    fn set_enabled(&self, name: &ScheduleName, enabled: bool) -> Result<bool, CronError> {
        // Flips only the `enabled` column, in place (REQ-CRON-0014).
        let changed = self
            .shared
            .lock()
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
        let mut inner = self.shared.lock();
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
        let name = &schedule.spec.name;
        if flags.iter().any(|(listed, _)| listed == name) {
            return Err(CronError::InvalidSpec(format!(
                "the flags to set name the written schedule {name}"
            )));
        }
        let columns = Columns::new(schedule, created_at)?;
        let mut inner = self.shared.lock();
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
        let id = match existing {
            Some((id, generation)) => {
                columns.rewrite(&transaction, id, generation, enabled)?;
                id
            }
            None => columns.insert(&transaction, name.as_str(), enabled)?,
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

    use super::*;
    use crate::location::{DatabaseIdentity, DoltDatabase, ProjectId};
    use crate::store::SqliteStore;

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
}
