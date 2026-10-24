//! [`ScheduleStore`] over SQLite (bundled), in its own database file.
//!
//! The store owns its file and its one connection (NFR-BDC-0007): it never
//! opens a second connection to another store's file. On open it sets the
//! busy timeout first, then WAL, then checks the schema version in
//! `PRAGMA user_version`. A fresh file (version 0, no tables) gets the
//! schema and version [`SCHEMA_VERSION`]. A file at another version, or one
//! that already holds other tables, is refused with
//! [`StoreErrorKind::SchemaMismatch`].
//!
//! `create` runs as one immediate transaction, so the active-name check and
//! the write cannot interleave with another writer. Run-state writes are
//! guarded by `id`, `generation` and `ended_at IS NULL`. Table CHECKs keep
//! `next_run_at` set exactly while a schedule is active and `stop_reason`
//! set exactly once it has ended, and repeat the size limits. Timestamps
//! become text only here, in the fixed-width form [`Timestamp`] displays.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Row, ToSql, Transaction, TransactionBehavior, params,
};

use crate::limits::{MAX_EXPR_LEN, MAX_NAME_LEN, MAX_PAYLOAD_BYTES, encode_payload};
use crate::{
    CronError, CronExpr, Generation, NewSchedule, RunOutcome, RunTimeout, Schedule, ScheduleId,
    ScheduleName, ScheduleSpec, ScheduleStore, StoreError, StoreErrorKind, Timestamp,
};

const TABLE: &str = "sc_cron_schedule";

/// The `PRAGMA user_version` this code writes and accepts.
pub const SCHEMA_VERSION: i64 = 1;

fn schema() -> String {
    format!(
        "CREATE TABLE sc_cron_schedule (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    generation   INTEGER NOT NULL CHECK (generation BETWEEN 1 AND 4294967295),
    name         TEXT    NOT NULL UNIQUE
                 CHECK (length(CAST(name AS BLOB)) BETWEEN 1 AND {MAX_NAME_LEN}),
    expr         TEXT    NOT NULL CHECK (length(CAST(expr AS BLOB)) <= {MAX_EXPR_LEN}),
    timeout_ms   INTEGER NOT NULL CHECK (timeout_ms > 0),
    jitter_ms    INTEGER NOT NULL CHECK (jitter_ms >= 0),
    payload      TEXT    NOT NULL
                 CHECK (length(CAST(payload AS BLOB)) <= {MAX_PAYLOAD_BYTES}),
    created_at   TEXT    NOT NULL,
    next_run_at  TEXT,
    last_run_at  TEXT,
    last_outcome TEXT,
    ended_at     TEXT,
    stop_reason  TEXT,
    CHECK ((ended_at IS NULL) = (next_run_at IS NOT NULL)),
    CHECK ((ended_at IS NULL) = (stop_reason IS NULL))
);"
    )
}

const COLUMNS: &str = "id, generation, name, expr, timeout_ms, jitter_ms, payload, created_at, \
     next_run_at, last_run_at, last_outcome, ended_at, stop_reason";

/// How long a writer waits for another connection's lock on the same file.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// SQLite-backed schedule store on its own database file.
///
/// # Examples
///
/// ```
/// use sc_cron::{ScheduleStore, SqliteScheduleStore};
///
/// let store = SqliteScheduleStore::open_in_memory()?;
/// assert!(store.list_active()?.is_empty());
/// # Ok::<(), sc_cron::CronError>(())
/// ```
#[derive(Debug)]
pub struct SqliteScheduleStore {
    // A rusqlite `Connection` is `Send` but not `Sync`, and the port's
    // methods take `&self` from any thread, so the one connection sits
    // behind a mutex. Calls are short single statements or one transaction.
    connection: Mutex<Connection>,
}

impl SqliteScheduleStore {
    /// Opens or creates the sc-cron database at `path`.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`]: [`StoreErrorKind::Unavailable`] if the file
    /// cannot be opened, [`StoreErrorKind::SchemaMismatch`] if it holds an
    /// unknown schema version or another store's tables.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CronError> {
        let connection = Connection::open(path.as_ref())
            .map_err(|error| store_error("failed to open database", error))?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|error| store_error("failed to set busy timeout", error))?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| store_error("failed to enable WAL", error))?;
        Self::with_connection(connection)
    }

    /// Opens a private in-memory database, for tests and ephemeral use.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if SQLite cannot allocate the database.
    pub fn open_in_memory() -> Result<Self, CronError> {
        let connection = Connection::open_in_memory()
            .map_err(|error| store_error("failed to open database", error))?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|error| store_error("failed to set busy timeout", error))?;
        Self::with_connection(connection)
    }

    fn with_connection(mut connection: Connection) -> Result<Self, CronError> {
        init_schema(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        // A panic while the lock was held cannot leave a transaction open:
        // rusqlite rolls back on drop, so the connection is still sound.
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
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
            .lock()
            .execute(sql, all.as_slice())
            .map_err(|error| store_error(&format!("failed to update {TABLE} id {id}"), error))?;
        Ok(changed == 1)
    }
}

/// Applies or checks the schema in one immediate transaction.
fn init_schema(connection: &mut Connection) -> Result<(), CronError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| store_error("failed to begin schema transaction", error))?;
    let version: i64 = transaction
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| store_error("failed to read schema version", error))?;
    match version {
        0 => {
            let tables: i64 = transaction
                .query_row(
                    "SELECT count(*) FROM sqlite_master
                     WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                    [],
                    |row| row.get(0),
                )
                .map_err(|error| store_error("failed to inspect database", error))?;
            if tables > 0 {
                return Err(StoreError::new(
                    StoreErrorKind::SchemaMismatch,
                    format!(
                        "database holds {tables} other table(s); sc-cron needs its own database file"
                    ),
                )
                .into());
            }
            transaction
                .execute_batch(&schema())
                .map_err(|error| store_error("failed to create schema", error))?;
            transaction
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|error| store_error("failed to set schema version", error))?;
        }
        SCHEMA_VERSION => {}
        other => {
            return Err(StoreError::new(
                StoreErrorKind::SchemaMismatch,
                format!("schema version {other} is not {SCHEMA_VERSION}"),
            )
            .into());
        }
    }
    transaction
        .commit()
        .map_err(|error| store_error("failed to commit schema", error))
}

impl ScheduleStore for SqliteScheduleStore {
    fn create(&self, schedule: &NewSchedule, created_at: Timestamp) -> Result<Schedule, CronError> {
        let columns = Columns::new(schedule, created_at)?;
        let mut connection = self.lock();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| store_error("failed to begin transaction", error))?;
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
                columns.restart(&transaction, id, generation)?;
                id
            }
            None => columns.insert(&transaction, schedule.spec.name.as_str())?,
        };
        let row = select_one(&transaction, id)?.ok_or_else(|| {
            StoreError::new(
                StoreErrorKind::Other,
                format!("{TABLE} id {id}: missing after create"),
            )
        })?;
        transaction
            .commit()
            .map_err(|error| store_error("failed to commit", error))?;
        Ok(row)
    }

    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError> {
        let Ok(id) = i64::try_from(id.get()) else {
            return Ok(None);
        };
        select_one(&self.lock(), id)
    }

    fn list_active(&self) -> Result<Vec<Schedule>, CronError> {
        let connection = self.lock();
        let mut statement = connection
            .prepare(&format!(
                "SELECT {COLUMNS} FROM sc_cron_schedule WHERE ended_at IS NULL
                 ORDER BY created_at, id"
            ))
            .map_err(|error| store_error(&format!("failed to read {TABLE}"), error))?;
        let raws = statement
            .query_map([], RawRow::read)
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|error| store_error(&format!("failed to read {TABLE}"), error))?;
        raws.into_iter().map(RawRow::decode).collect()
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
             WHERE id = ?1 AND generation = ?2 AND ended_at IS NULL",
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
}

/// One schedule's column values, converted before any write.
///
/// Name, expression and timeout are valid by type; only the payload size
/// and the integer range of the durations remain to check.
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
        Ok(Self {
            expr: spec.expr.as_str().to_owned(),
            timeout_ms: millis("timeout", spec.timeout.get())?,
            jitter_ms: millis("jitter", schedule.jitter)?,
            payload: encode_payload(&spec.payload)?,
            created_at: created_at.to_string(),
            next_run_at: schedule.next_run_at.to_string(),
        })
    }

    fn insert(&self, transaction: &Transaction<'_>, name: &str) -> Result<i64, CronError> {
        transaction
            .execute(
                "INSERT INTO sc_cron_schedule
                    (generation, name, expr, timeout_ms, jitter_ms, payload,
                     created_at, next_run_at)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    name,
                    self.expr,
                    self.timeout_ms,
                    self.jitter_ms,
                    self.payload,
                    self.created_at,
                    self.next_run_at,
                ],
            )
            .map_err(|error| store_error(&format!("failed to insert {TABLE}"), error))?;
        Ok(transaction.last_insert_rowid())
    }

    fn restart(
        &self,
        transaction: &Transaction<'_>,
        id: i64,
        generation: i64,
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
                    stop_reason = NULL
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
                ],
            )
            .map_err(|error| store_error(&format!("failed to restart {TABLE} id {id}"), error))?;
        Ok(())
    }
}

fn millis(field: &str, value: Duration) -> Result<i64, CronError> {
    i64::try_from(value.as_millis())
        .map_err(|_| CronError::InvalidSpec(format!("{field} is too large to store")))
}

/// Classifies a rusqlite error so callers can tell busy from corrupt.
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
                .map_err(|_| corrupt(format!("{name} is negative")))
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
            id: ScheduleId::new(u64::try_from(id).map_err(|_| corrupt("id is negative".into()))?),
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn kind_of(code: std::ffi::c_int) -> StoreErrorKind {
        let error = rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None);
        match store_error("op", error) {
            CronError::Store(store) => store.kind(),
            other => panic!("expected a store error, got {other:?}"),
        }
    }

    #[test]
    fn rusqlite_errors_are_classified_so_busy_is_distinguishable() {
        use rusqlite::ffi;
        assert_eq!(kind_of(ffi::SQLITE_BUSY), StoreErrorKind::Busy);
        assert_eq!(kind_of(ffi::SQLITE_LOCKED), StoreErrorKind::Busy);
        assert_eq!(kind_of(ffi::SQLITE_CORRUPT), StoreErrorKind::Corrupt);
        assert_eq!(kind_of(ffi::SQLITE_NOTADB), StoreErrorKind::Corrupt);
        assert_eq!(kind_of(ffi::SQLITE_FULL), StoreErrorKind::Unavailable);
        assert_eq!(kind_of(ffi::SQLITE_READONLY), StoreErrorKind::Unavailable);
        assert_eq!(kind_of(ffi::SQLITE_CONSTRAINT), StoreErrorKind::Other);
    }

    #[test]
    fn a_store_error_keeps_the_rusqlite_error_as_its_source() {
        use std::error::Error;
        let error = store_error("op", rusqlite::Error::QueryReturnedNoRows);
        let store = error.source().expect("store error");
        assert!(store.source().is_some());
    }
}
