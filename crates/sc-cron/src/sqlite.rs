//! [`ScheduleStore`] over SQLite (bundled), in one `sc_cron_schedule` table.
//!
//! The table can share a database file with other tables; it is created on
//! open if missing. One connection is held behind a mutex. `create` runs as
//! one immediate transaction, so the active-name check and the write cannot
//! interleave with another writer on the same file. Run-state writes are
//! guarded by `id`, `generation` and `ended_at IS NULL`.
//!
//! A schedule is active exactly while `ended_at` is null; a table CHECK
//! keeps `next_run_at` null once a schedule has ended.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Row, ToSql, TransactionBehavior, params};

use crate::{
    CronError, Generation, NewSchedule, RunOutcome, Schedule, ScheduleId, ScheduleSpec,
    ScheduleStore, timestamp,
};

const TABLE: &str = "sc_cron_schedule";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sc_cron_schedule (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    generation   INTEGER NOT NULL CHECK (generation BETWEEN 1 AND 4294967295),
    name         TEXT    NOT NULL UNIQUE CHECK (length(name) > 0),
    expr         TEXT    NOT NULL,
    timeout_ms   INTEGER NOT NULL CHECK (timeout_ms > 0),
    jitter_ms    INTEGER NOT NULL CHECK (jitter_ms >= 0),
    payload      TEXT    NOT NULL,
    created_at   TEXT    NOT NULL,
    next_run_at  TEXT,
    last_run_at  TEXT,
    last_outcome TEXT,
    ended_at     TEXT,
    stop_reason  TEXT,
    CHECK (ended_at IS NULL OR next_run_at IS NULL),
    CHECK ((ended_at IS NULL) = (stop_reason IS NULL))
);";

const COLUMNS: &str = "id, generation, name, expr, timeout_ms, jitter_ms, payload, created_at, \
     next_run_at, last_run_at, last_outcome, ended_at, stop_reason";

/// How long a writer waits for another connection's lock on the same file.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// SQLite-backed schedule store.
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
    connection: Mutex<Connection>,
}

impl SqliteScheduleStore {
    /// Opens or creates the database at `path` and ensures the table exists.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if the file cannot be opened or the schema
    /// cannot be applied.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CronError> {
        let connection = Connection::open(path.as_ref())
            .map_err(|error| store_error("failed to open database", &error))?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| store_error("failed to enable WAL", &error))?;
        Self::with_connection(connection)
    }

    /// Opens a private in-memory database, for tests and ephemeral use.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if SQLite cannot allocate the database.
    pub fn open_in_memory() -> Result<Self, CronError> {
        let connection = Connection::open_in_memory()
            .map_err(|error| store_error("failed to open database", &error))?;
        Self::with_connection(connection)
    }

    fn with_connection(connection: Connection) -> Result<Self, CronError> {
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|error| store_error("failed to set busy timeout", &error))?;
        connection
            .execute_batch(SCHEMA)
            .map_err(|error| store_error("failed to create table", &error))?;
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
            .map_err(|error| store_error(&format!("failed to update {TABLE} id {id}"), &error))?;
        Ok(changed == 1)
    }
}

impl ScheduleStore for SqliteScheduleStore {
    fn create(&self, schedule: &NewSchedule, created_at: &str) -> Result<Schedule, CronError> {
        let columns = Columns::new(schedule, created_at)?;
        let mut connection = self.lock();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| store_error("failed to begin transaction", &error))?;
        let existing = transaction
            .query_row(
                "SELECT id, generation, ended_at IS NULL FROM sc_cron_schedule WHERE name = ?1",
                [&columns.name],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| store_error(&format!("failed to read {TABLE}"), &error))?;
        let id = match existing {
            Some((_, _, true)) => return Err(CronError::AlreadyActive(columns.name)),
            Some((id, generation, false)) => {
                let next = u32::try_from(generation)
                    .ok()
                    .and_then(Generation::new)
                    .and_then(Generation::next)
                    .ok_or_else(|| {
                        CronError::Store(format!("{TABLE} id {id}: generation exhausted"))
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
                            columns.expr,
                            columns.timeout_ms,
                            columns.jitter_ms,
                            columns.payload,
                            columns.created_at,
                            columns.next_run_at,
                        ],
                    )
                    .map_err(|error| store_error(&format!("failed to restart {TABLE}"), &error))?;
                id
            }
            None => {
                transaction
                    .execute(
                        "INSERT INTO sc_cron_schedule
                            (generation, name, expr, timeout_ms, jitter_ms, payload,
                             created_at, next_run_at)
                         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            columns.name,
                            columns.expr,
                            columns.timeout_ms,
                            columns.jitter_ms,
                            columns.payload,
                            columns.created_at,
                            columns.next_run_at,
                        ],
                    )
                    .map_err(|error| store_error(&format!("failed to insert {TABLE}"), &error))?;
                transaction.last_insert_rowid()
            }
        };
        let row = select_one(&transaction, id)?
            .ok_or_else(|| CronError::Store(format!("{TABLE} id {id}: missing after create")))?;
        transaction
            .commit()
            .map_err(|error| store_error("failed to commit", &error))?;
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
            .map_err(|error| store_error(&format!("failed to read {TABLE}"), &error))?;
        let raws = statement
            .query_map([], RawRow::read)
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|error| store_error(&format!("failed to read {TABLE}"), &error))?;
        raws.into_iter().map(RawRow::decode).collect()
    }

    fn record_run(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: &str,
        outcome: &RunOutcome,
    ) -> Result<bool, CronError> {
        check_timestamp("at", at)?;
        self.guarded_update(
            "UPDATE sc_cron_schedule SET last_run_at = ?3, last_outcome = ?4
             WHERE id = ?1 AND generation = ?2 AND ended_at IS NULL",
            id,
            generation,
            &[&at, &encode_outcome(outcome)],
        )
    }

    fn set_next(
        &self,
        id: ScheduleId,
        generation: Generation,
        next_run_at: &str,
    ) -> Result<bool, CronError> {
        check_timestamp("next_run_at", next_run_at)?;
        self.guarded_update(
            "UPDATE sc_cron_schedule SET next_run_at = ?3
             WHERE id = ?1 AND generation = ?2 AND ended_at IS NULL",
            id,
            generation,
            &[&next_run_at],
        )
    }

    fn stop(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: &str,
        reason: &str,
    ) -> Result<bool, CronError> {
        check_timestamp("at", at)?;
        self.guarded_update(
            "UPDATE sc_cron_schedule SET ended_at = ?3, stop_reason = ?4, next_run_at = NULL
             WHERE id = ?1 AND generation = ?2 AND ended_at IS NULL",
            id,
            generation,
            &[&at, &reason],
        )
    }
}

/// One schedule's values, validated and converted before any write.
struct Columns {
    name: String,
    expr: String,
    timeout_ms: i64,
    jitter_ms: i64,
    payload: String,
    created_at: String,
    next_run_at: String,
}

impl Columns {
    fn new(schedule: &NewSchedule, created_at: &str) -> Result<Self, CronError> {
        check_timestamp("created_at", created_at)?;
        check_timestamp("next_run_at", &schedule.next_run_at)?;
        let spec = &schedule.spec;
        if spec.name.is_empty() {
            return Err(CronError::InvalidSpec("name is empty".into()));
        }
        let timeout_ms = millis("timeout", spec.timeout)?;
        if timeout_ms == 0 {
            return Err(CronError::InvalidSpec(
                "timeout must be at least 1 ms".into(),
            ));
        }
        Ok(Self {
            name: spec.name.clone(),
            expr: spec.expr.clone(),
            timeout_ms,
            jitter_ms: millis("jitter", schedule.jitter)?,
            payload: spec.payload.to_string(),
            created_at: created_at.to_owned(),
            next_run_at: schedule.next_run_at.clone(),
        })
    }
}

fn millis(field: &str, value: Duration) -> Result<i64, CronError> {
    i64::try_from(value.as_millis())
        .map_err(|_| CronError::InvalidSpec(format!("{field} is too large to store")))
}

fn check_timestamp(field: &str, text: &str) -> Result<(), CronError> {
    timestamp::parse(text).map(drop).map_err(|error| {
        CronError::InvalidSpec(format!("{field} {text:?} is not RFC 3339: {error}"))
    })
}

fn store_error(context: &str, error: &rusqlite::Error) -> CronError {
    CronError::Store(format!("{context}: {error}"))
}

fn select_one(connection: &Connection, id: i64) -> Result<Option<Schedule>, CronError> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM sc_cron_schedule WHERE id = ?1"),
            [id],
            RawRow::read,
        )
        .optional()
        .map_err(|error| store_error(&format!("failed to read {TABLE} id {id}"), &error))?
        .map(RawRow::decode)
        .transpose()
}

/// Encodes an outcome as the text kept in `last_outcome`.
fn encode_outcome(outcome: &RunOutcome) -> String {
    outcome.to_string()
}

/// Decodes `last_outcome` text written by [`encode_outcome`].
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

    fn decode(self) -> Result<Schedule, CronError> {
        let id = self.id;
        let bad = |what: &str| CronError::Store(format!("{TABLE} id {id}: {what}"));
        let duration = |value: i64, what: &str| {
            u64::try_from(value)
                .map(Duration::from_millis)
                .map_err(|_| bad(&format!("{what} is negative")))
        };
        let payload = serde_json::from_str(&self.payload)
            .map_err(|error| bad(&format!("payload is not JSON: {error}")))?;
        let last_outcome = self
            .last_outcome
            .as_deref()
            .map(|text| {
                decode_outcome(text).ok_or_else(|| bad(&format!("unknown outcome {text:?}")))
            })
            .transpose()?;
        Ok(Schedule {
            id: ScheduleId::new(u64::try_from(id).map_err(|_| bad("id is negative"))?),
            generation: u32::try_from(self.generation)
                .ok()
                .and_then(Generation::new)
                .ok_or_else(|| bad("generation is out of range"))?,
            spec: ScheduleSpec {
                name: self.name,
                expr: self.expr,
                timeout: duration(self.timeout_ms, "timeout_ms")?,
                payload,
            },
            jitter: duration(self.jitter_ms, "jitter_ms")?,
            created_at: self.created_at,
            next_run_at: self.next_run_at,
            last_run_at: self.last_run_at,
            last_outcome,
            ended_at: self.ended_at,
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
            assert_eq!(decode_outcome(&encode_outcome(&outcome)), Some(outcome));
        }
        assert_eq!(decode_outcome("exit"), None);
    }
}
