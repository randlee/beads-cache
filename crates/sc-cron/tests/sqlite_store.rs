//! `SqliteScheduleStore`: the store contract with fixed timestamps,
//! generation fencing, the table CHECKs, the schema version and reopen
//! durability, and the `enabled` column with the version 1 upgrade
//! (REQ-CRON-0009, REQ-CRON-0014).

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed expectation is the test failure"
)]

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;
use sc_cron::{
    CronError, Generation, MAX_PAYLOAD_BYTES, NewSchedule, RunOutcome, SCHEMA_VERSION, ScheduleId,
    ScheduleName, ScheduleSpec, ScheduleStore, SqliteScheduleStore, StoreErrorKind, Timestamp,
};
use tempfile::TempDir;

fn stamp(day: u32) -> Timestamp {
    stamp_text(day).parse().expect("timestamp")
}

/// The fixed-width text the store writes for `stamp(day)`.
fn stamp_text(day: u32) -> String {
    format!("2026-10-{day:02}T12:00:00.000000Z")
}

fn new_schedule(name: &str) -> NewSchedule {
    NewSchedule {
        spec: ScheduleSpec::new(
            name,
            "*/5 * * * * *",
            Duration::from_secs(30),
            serde_json::json!({"goal": "x", "n": [1, {"y": null}]}),
        )
        .expect("spec"),
        jitter: Duration::from_millis(1799),
        next_run_at: stamp(20),
    }
}

fn generation(value: u32) -> Generation {
    Generation::new(value).expect("generation")
}

fn store_kind(result: Result<impl std::fmt::Debug, CronError>) -> StoreErrorKind {
    match result {
        Err(CronError::Store(error)) => error.kind(),
        other => panic!("expected a store error, got {other:?}"),
    }
}

struct Harness {
    // Fields drop in declaration order: the store (and its connection) must
    // close before the temp dir is removed, or Windows cannot delete the file.
    store: SqliteScheduleStore,
    path: PathBuf,
    _root: TempDir,
}

impl Harness {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("cron.db");
        let store = SqliteScheduleStore::open(&path).expect("store");
        Self {
            _root: root,
            path,
            store,
        }
    }

    fn sql(&self) -> Connection {
        Connection::open(&self.path).expect("connection")
    }

    fn create(&self, name: &str, day: u32) -> sc_cron::Schedule {
        self.store
            .create(&new_schedule(name), stamp(day))
            .expect("create")
    }
}

/// Inserts a row by SQL with explicit end-state columns, for the CHECK tests.
fn sql_insert(
    path: &Path,
    name: &str,
    next_run_at: Option<&str>,
    ended_at: Option<&str>,
    stop_reason: Option<&str>,
) -> rusqlite::Result<usize> {
    Connection::open(path)?.execute(
        "INSERT INTO sc_cron_schedule
            (generation, name, expr, timeout_ms, jitter_ms, payload, created_at,
             next_run_at, ended_at, stop_reason)
         VALUES (1, ?1, '@hourly', 1000, 0, '{}', ?2, ?3, ?4, ?5)",
        rusqlite::params![name, stamp_text(1), next_run_at, ended_at, stop_reason],
    )
}

fn assert_check_violation(result: rusqlite::Result<usize>, case: &str) {
    let message = result.expect_err(case).to_string();
    assert!(
        message.contains("CHECK constraint failed"),
        "{case}: expected a CHECK violation, got: {message}"
    );
}

#[test]
fn create_round_trips_every_field_and_timestamps_are_kept_as_given() {
    let h = Harness::new();
    let created = h.create("nightly", 3);
    assert_eq!(created.generation, Generation::FIRST);
    assert_eq!(created.spec, new_schedule("nightly").spec);
    assert_eq!(created.jitter, Duration::from_millis(1799));
    assert_eq!(created.created_at, stamp(3));
    assert_eq!(created.next_run_at, Some(stamp(20)));
    assert_eq!(created.last_run_at, None);
    assert_eq!(created.last_outcome, None);
    assert!(created.is_active());
    assert_eq!(h.store.get(created.id).expect("get"), Some(created));
    let stored: String = h
        .sql()
        .query_row("SELECT created_at FROM sc_cron_schedule", [], |row| {
            row.get(0)
        })
        .expect("created_at");
    assert_eq!(stored, stamp_text(3));
}

#[test]
fn list_active_survives_reopen_and_orders_by_created_at() {
    let h = Harness::new();
    h.create("later", 5);
    let ended = h.create("ended", 2);
    h.create("earlier", 3);
    assert!(
        h.store
            .stop(ended.id, ended.generation, stamp(6), "done")
            .expect("stop")
    );
    let before = h.store.list_active().expect("list");
    let names: Vec<_> = before.iter().map(|row| row.spec.name.as_str()).collect();
    assert_eq!(names, ["earlier", "later"]);

    let reopened = SqliteScheduleStore::open(&h.path).expect("reopen");
    assert_eq!(reopened.list_active().expect("list"), before);
}

#[test]
fn create_after_stop_restarts_in_place_and_raises_the_generation() {
    let h = Harness::new();
    let first = h.create("nightly", 3);
    assert!(
        h.store
            .record_run(first.id, first.generation, stamp(4), &RunOutcome::Ok)
            .expect("record")
    );
    assert!(
        h.store
            .stop(first.id, first.generation, stamp(4), "phase closed")
            .expect("stop")
    );
    let replacement = NewSchedule {
        spec: ScheduleSpec::new(
            "nightly",
            "0 15 * * * *",
            Duration::from_secs(12),
            serde_json::json!({"replacement": true}),
        )
        .expect("spec"),
        jitter: Duration::from_secs(31),
        next_run_at: stamp(7),
    };
    let restarted = h.store.create(&replacement, stamp(6)).expect("restart");
    assert_eq!(restarted.id, first.id);
    assert_eq!(restarted.generation, generation(2));
    assert_eq!(restarted.spec, replacement.spec);
    assert_eq!(restarted.jitter, Duration::from_secs(31));
    assert_eq!(restarted.created_at, stamp(6));
    assert_eq!(restarted.next_run_at, Some(stamp(7)));
    assert_eq!(restarted.last_run_at, None);
    assert_eq!(restarted.last_outcome, None);
    assert_eq!(restarted.ended_at, None);
    assert_eq!(restarted.stop_reason, None);
}

#[test]
fn create_refuses_an_active_duplicate() {
    let h = Harness::new();
    h.create("nightly", 3);
    let error = h
        .store
        .create(&new_schedule("nightly"), stamp(4))
        .expect_err("duplicate");
    assert!(
        matches!(&error, CronError::AlreadyActive(name) if name.as_str() == "nightly"),
        "{error:?}"
    );
    assert_eq!(h.store.list_active().expect("list").len(), 1);
}

#[test]
fn create_refuses_an_over_size_payload() {
    let h = Harness::new();
    let mut big = new_schedule("big");
    big.spec.payload = serde_json::Value::String("x".repeat(MAX_PAYLOAD_BYTES));
    assert!(matches!(
        h.store.create(&big, stamp(3)),
        Err(CronError::InvalidSpec(_))
    ));
    assert!(h.store.list_active().expect("list").is_empty());
}

#[test]
fn a_stale_generation_changes_nothing() {
    let h = Harness::new();
    let first = h.create("nightly", 3);
    assert!(
        h.store
            .stop(first.id, first.generation, stamp(4), "end")
            .expect("stop")
    );
    let restarted = h.create("nightly", 6);
    let stale = first.generation;
    assert!(
        !h.store
            .record_run(first.id, stale, stamp(7), &RunOutcome::Ok)
            .expect("stale record")
    );
    assert!(
        !h.store
            .set_next(first.id, stale, stamp(8))
            .expect("stale next")
    );
    assert!(
        !h.store
            .stop(first.id, stale, stamp(7), "stale")
            .expect("stale stop")
    );
    assert_eq!(h.store.get(first.id).expect("get"), Some(restarted.clone()));

    assert!(
        h.store
            .record_run(
                first.id,
                restarted.generation,
                stamp(8),
                &RunOutcome::TimedOut
            )
            .expect("current record")
    );
    let row = h.store.get(first.id).expect("get").expect("row");
    assert_eq!(row.last_outcome, Some(RunOutcome::TimedOut));
    assert_eq!(row.last_run_at, Some(stamp(8)));
}

#[test]
fn stop_ends_the_row_once_and_clears_the_next_run() {
    let h = Harness::new();
    let row = h.create("nightly", 3);
    assert!(
        h.store
            .stop(row.id, row.generation, stamp(4), "phase closed")
            .expect("stop")
    );
    assert!(
        !h.store
            .stop(row.id, row.generation, stamp(5), "again")
            .expect("again")
    );
    let ended = h.store.get(row.id).expect("get").expect("row");
    assert!(!ended.is_active());
    assert_eq!(ended.ended_at, Some(stamp(4)));
    assert_eq!(ended.stop_reason.as_deref(), Some("phase closed"));
    assert_eq!(ended.next_run_at, None);
    assert!(h.store.list_active().expect("list").is_empty());
}

#[test]
fn run_state_writes_on_an_ended_row_change_nothing() {
    let h = Harness::new();
    let row = h.create("nightly", 3);
    h.store
        .stop(row.id, row.generation, stamp(4), "end")
        .expect("stop");
    assert!(
        !h.store
            .record_run(row.id, row.generation, stamp(5), &RunOutcome::Ok)
            .expect("late run")
    );
    assert!(
        !h.store
            .set_next(row.id, row.generation, stamp(6))
            .expect("late next")
    );
    let ended = h.store.get(row.id).expect("get").expect("row");
    assert_eq!(ended.last_outcome, None);
    assert_eq!(ended.next_run_at, None);
}

#[test]
fn set_next_stores_the_given_instant() {
    let h = Harness::new();
    let row = h.create("nightly", 3);
    assert!(
        h.store
            .set_next(row.id, row.generation, stamp(9))
            .expect("next")
    );
    let stored = h.store.get(row.id).expect("get").expect("row");
    assert_eq!(stored.next_run_at, Some(stamp(9)));
}

#[test]
fn every_outcome_class_round_trips() {
    let h = Harness::new();
    let row = h.create("nightly", 3);
    for outcome in [
        RunOutcome::Ok,
        RunOutcome::Refused("quota: full".into()),
        RunOutcome::Failed("job panicked: boom".into()),
        RunOutcome::TimedOut,
    ] {
        assert!(
            h.store
                .record_run(row.id, row.generation, stamp(4), &outcome)
                .expect("record")
        );
        let stored = h.store.get(row.id).expect("get").expect("row");
        assert_eq!(stored.last_outcome, Some(outcome));
    }
}

#[test]
fn an_unstorable_id_changes_nothing_and_reads_as_absent() {
    let h = Harness::new();
    let id = ScheduleId::new(u64::MAX);
    assert!(
        !h.store
            .record_run(id, Generation::FIRST, stamp(4), &RunOutcome::Ok)
            .expect("record")
    );
    assert_eq!(h.store.get(id).expect("get"), None);
}

#[test]
fn table_checks_refuse_contradictory_and_over_size_rows() {
    let h = Harness::new();
    let next = stamp_text(2);
    sql_insert(&h.path, "ok-active", Some(&next), None, None).expect("valid active row");
    sql_insert(&h.path, "ok-ended", None, Some(&next), Some("done")).expect("valid ended row");
    assert_check_violation(
        sql_insert(
            &h.path,
            "ended-with-next",
            Some(&next),
            Some(&next),
            Some("done"),
        ),
        "ended row with a next run",
    );
    assert_check_violation(
        sql_insert(&h.path, "active-without-next", None, None, None),
        "active row without a next run",
    );
    assert_check_violation(
        sql_insert(&h.path, "ended-no-reason", None, Some(&next), None),
        "ended row without a reason",
    );
    assert_check_violation(
        sql_insert(
            &h.path,
            &"n".repeat(sc_cron::MAX_NAME_LEN + 1),
            Some(&next),
            None,
            None,
        ),
        "over-long name",
    );
}

fn schedule_name(name: &str) -> ScheduleName {
    ScheduleName::new(name).expect("name")
}

#[test]
fn set_enabled_changes_only_the_flag_of_the_named_active_schedule() {
    let h = Harness::new();
    let nightly = h.create("nightly", 3);
    let other = h.create("other", 3);
    assert!(nightly.enabled && other.enabled);

    assert!(
        h.store
            .set_enabled(&schedule_name("nightly"), false)
            .expect("disable")
    );
    assert_eq!(
        h.store.get(nightly.id).expect("get"),
        Some(sc_cron::Schedule {
            enabled: false,
            ..nightly.clone()
        })
    );
    assert_eq!(h.store.get(other.id).expect("get"), Some(other.clone()));
    // Idempotent, and it lists with the flag.
    assert!(
        h.store
            .set_enabled(&schedule_name("nightly"), false)
            .expect("again")
    );
    let listed = h.store.list_active().expect("list");
    assert_eq!(
        listed.iter().map(|s| s.enabled).collect::<Vec<_>>(),
        [false, true]
    );

    // A name with no active schedule: nothing changes.
    assert!(
        !h.store
            .set_enabled(&schedule_name("missing"), true)
            .expect("missing")
    );
    assert!(
        h.store
            .stop(nightly.id, nightly.generation, stamp(5), "end")
            .expect("stop")
    );
    assert!(
        !h.store
            .set_enabled(&schedule_name("nightly"), true)
            .expect("ended")
    );
    assert!(!h.store.get(nightly.id).expect("get").expect("row").enabled);
}

#[test]
fn restarting_an_ended_name_enables_it_again() {
    let h = Harness::new();
    let first = h.create("nightly", 3);
    assert!(
        h.store
            .set_enabled(&schedule_name("nightly"), false)
            .expect("disable")
    );
    assert!(
        h.store
            .stop(first.id, first.generation, stamp(4), "end")
            .expect("stop")
    );
    assert!(h.create("nightly", 6).enabled);
}

#[test]
fn set_next_refuses_a_disabled_schedule_and_the_other_writes_still_apply() {
    let h = Harness::new();
    let row = h.create("nightly", 3);
    assert!(
        h.store
            .set_enabled(&schedule_name("nightly"), false)
            .expect("disable")
    );
    assert!(
        !h.store
            .set_next(row.id, row.generation, stamp(9))
            .expect("set_next on a disabled row"),
        "a disabled schedule is not given a next run"
    );
    assert_eq!(
        h.store.get(row.id).expect("get").expect("row").next_run_at,
        row.next_run_at
    );
    // A run in flight when the flag was cleared can still record, and the
    // schedule can still be ended.
    assert!(
        h.store
            .record_run(row.id, row.generation, stamp(7), &RunOutcome::Ok)
            .expect("record")
    );
    assert!(
        h.store
            .stop(row.id, row.generation, stamp(8), "end")
            .expect("stop")
    );
}

#[test]
fn disable_is_fenced_and_records_the_reason_as_a_refused_outcome() {
    let h = Harness::new();
    let first = h.create("nightly", 3);
    assert!(
        h.store
            .stop(first.id, first.generation, stamp(4), "end")
            .expect("stop")
    );
    let restarted = h.create("nightly", 6);
    // A stop check from the old generation cannot disable the restarted schedule.
    assert!(
        !h.store
            .disable(first.id, first.generation, stamp(7), "stale")
            .expect("stale disable")
    );
    assert_eq!(h.store.get(first.id).expect("get"), Some(restarted.clone()));

    assert!(
        h.store
            .disable(first.id, restarted.generation, stamp(8), "quiet")
            .expect("disable")
    );
    let row = h.store.get(first.id).expect("get").expect("row");
    assert!(row.is_active() && !row.enabled);
    assert_eq!(row.last_outcome, Some(RunOutcome::Refused("quiet".into())));
    assert_eq!(row.last_run_at, Some(stamp(8)));
    assert_eq!(row.spec, restarted.spec);
    assert_eq!(row.next_run_at, restarted.next_run_at);
}

#[test]
fn the_enabled_column_accepts_only_zero_or_one() {
    let h = Harness::new();
    h.create("nightly", 3);
    assert_check_violation(
        h.sql()
            .execute("UPDATE sc_cron_schedule SET enabled = 2", []),
        "enabled = 2",
    );
}

/// The version 1 table, before the `enabled` column.
const SCHEMA_V1: &str = "CREATE TABLE sc_cron_schedule (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    generation   INTEGER NOT NULL CHECK (generation BETWEEN 1 AND 4294967295),
    name         TEXT    NOT NULL UNIQUE CHECK (length(CAST(name AS BLOB)) BETWEEN 1 AND 128),
    expr         TEXT    NOT NULL CHECK (length(CAST(expr AS BLOB)) <= 256),
    timeout_ms   INTEGER NOT NULL CHECK (timeout_ms > 0),
    jitter_ms    INTEGER NOT NULL CHECK (jitter_ms >= 0),
    payload      TEXT    NOT NULL CHECK (length(CAST(payload AS BLOB)) <= 65536),
    created_at   TEXT    NOT NULL,
    next_run_at  TEXT,
    last_run_at  TEXT,
    last_outcome TEXT,
    ended_at     TEXT,
    stop_reason  TEXT,
    CHECK ((ended_at IS NULL) = (next_run_at IS NOT NULL)),
    CHECK ((ended_at IS NULL) = (stop_reason IS NULL))
);";

#[test]
fn a_version_1_file_gains_the_enabled_column_with_every_schedule_enabled() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("cron.db");
    {
        let old = Connection::open(&path).expect("connection");
        old.execute_batch(SCHEMA_V1).expect("v1 schema");
        old.execute_batch(
            "INSERT INTO sc_cron_schedule
                (generation, name, expr, timeout_ms, jitter_ms, payload, created_at, next_run_at)
             VALUES (3, 'kept', '*/5 * * * * *', 30000, 1799, '{\"a\":1}',
                     '2026-10-03T12:00:00.000000Z', '2026-10-20T12:00:00.000000Z');
             INSERT INTO sc_cron_schedule
                (generation, name, expr, timeout_ms, jitter_ms, payload, created_at, ended_at,
                 stop_reason)
             VALUES (1, 'ended', '@hourly', 1000, 0, '{}', '2026-10-01T12:00:00.000000Z',
                     '2026-10-02T12:00:00.000000Z', 'done');",
        )
        .expect("v1 rows");
        old.pragma_update(None, "user_version", 1).expect("v1");
    }

    let store = SqliteScheduleStore::open(&path).expect("upgrade");
    let kept = store.list_active().expect("list");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].spec.name.as_str(), "kept");
    assert_eq!(kept[0].generation, generation(3));
    assert_eq!(kept[0].spec.payload, serde_json::json!({"a": 1}));
    assert!(kept[0].enabled, "an existing schedule stays enabled");
    let ended = store.get(ScheduleId::new(2)).expect("get").expect("ended");
    assert!(!ended.is_active() && ended.enabled);
    drop(store);

    let version: i64 = Connection::open(&path)
        .expect("connection")
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version");
    assert_eq!(version, SCHEMA_VERSION);
    // Opening again is a plain open, and the column now works.
    let store = SqliteScheduleStore::open(&path).expect("reopen");
    assert!(
        store
            .set_enabled(&schedule_name("kept"), false)
            .expect("disable")
    );
}

#[test]
fn a_corrupt_row_is_a_corrupt_store_error_naming_the_row() {
    for (column, value) in [
        ("payload", "'not json'"),
        ("last_outcome", "'exit'"),
        ("next_run_at", "'soon'"),
        ("expr", "'* * * *'"),
    ] {
        let fresh = Harness::new();
        let row = fresh.create("nightly", 3);
        fresh
            .sql()
            .execute(
                &format!("UPDATE sc_cron_schedule SET {column} = {value}"),
                [],
            )
            .expect("corrupt");
        // `get` names the row; `list_active` skips it (PE-RSH-007).
        let error = fresh.store.get(row.id).expect_err("corrupt row refused");
        assert!(
            matches!(&error, CronError::Store(store)
                if store.kind() == StoreErrorKind::Corrupt
                    && store.context().contains(&format!("sc_cron_schedule id {}", row.id))),
            "{column}: {error}"
        );
        assert!(
            fresh.store.list_active().expect("list").is_empty(),
            "{column}"
        );
    }
}

#[test]
fn a_corrupt_active_row_is_skipped_and_the_good_schedule_still_lists() {
    for (column, value) in [
        ("payload", "'not json'"),
        ("last_outcome", "'exit'"),
        ("next_run_at", "'soon'"),
        ("expr", "'* * * *'"),
        ("payload", "x'00ff'"),
    ] {
        let h = Harness::new();
        let bad = h.create("bad", 3);
        let good = h.create("good", 4);
        h.sql()
            .execute(
                &format!("UPDATE sc_cron_schedule SET {column} = {value} WHERE id = ?1"),
                [i64::try_from(bad.id.get()).expect("id")],
            )
            .expect("corrupt");
        // Twice: the second reload skips the same row again.
        for _ in 0..2 {
            assert_eq!(
                h.store.list_active().expect("list"),
                vec![good.clone()],
                "{column}"
            );
        }
    }
}

#[test]
fn a_file_with_other_tables_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("cache.db");
    Connection::open(&path)
        .expect("connection")
        .execute_batch("CREATE TABLE beads (id TEXT);")
        .expect("foreign table");
    assert_eq!(
        store_kind(SqliteScheduleStore::open(&path)),
        StoreErrorKind::SchemaMismatch
    );
}

#[test]
fn an_unknown_schema_version_is_refused_and_the_current_one_is_written() {
    let h = Harness::new();
    let version: i64 = h
        .sql()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version");
    assert_eq!(version, SCHEMA_VERSION);
    h.sql()
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .expect("bump version");
    let error = SqliteScheduleStore::open(&h.path).expect_err("unknown version");
    assert_eq!(error.code(), "STORE_FAILED");
    assert!(
        matches!(&error, CronError::Store(store) if store.kind() == StoreErrorKind::SchemaMismatch),
        "{error}"
    );
}

#[test]
fn store_errors_expose_their_rusqlite_source() {
    let root = tempfile::tempdir().expect("tempdir");
    let missing = root.path().join("no-such-dir").join("cron.db");
    let error = SqliteScheduleStore::open(&missing).expect_err("cannot open");
    assert!(
        matches!(&error, CronError::Store(store) if store.kind() == StoreErrorKind::Unavailable),
        "{error}"
    );
    let store = error.source().expect("store error");
    assert!(store.source().is_some(), "rusqlite error is preserved");
}

// --- REQ-CRON-0017: several rows' flags, and a row written with its flag ---

fn name(text: &str) -> ScheduleName {
    ScheduleName::new(text).expect("name")
}

/// The `enabled` flags of `names`, read in one statement by `c`.
fn flags_of(c: &Connection, names: &[&str]) -> Vec<Option<bool>> {
    names
        .iter()
        .map(|n| {
            c.query_row(
                "SELECT enabled FROM sc_cron_schedule WHERE name = ?1 AND ended_at IS NULL",
                [n],
                |row| row.get::<_, bool>(0),
            )
            .ok()
        })
        .collect()
}

/// Every pair of flags a second connection read while `flip` ran on the store.
fn poll_while(path: &Path, names: [&str; 2], flip: impl FnOnce()) -> Vec<(bool, bool)> {
    let path = path.to_path_buf();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop = std::sync::Arc::clone(&done);
    let names = names.map(str::to_owned);
    let poller = std::thread::spawn(move || {
        let c = Connection::open(&path).expect("second connection");
        c.busy_timeout(Duration::from_secs(5)).expect("busy");
        let mut seen = Vec::new();
        while !stop.load(std::sync::atomic::Ordering::SeqCst) {
            // One statement: one snapshot of both rows.
            let pair: (bool, bool) = c
                .query_row(
                    "SELECT (SELECT enabled FROM sc_cron_schedule WHERE name = ?1),
                            (SELECT enabled FROM sc_cron_schedule WHERE name = ?2)",
                    [&names[0], &names[1]],
                    |row| {
                        Ok((
                            row.get::<_, Option<bool>>(0)?.unwrap_or(false),
                            row.get::<_, Option<bool>>(1)?.unwrap_or(false),
                        ))
                    },
                )
                .expect("poll");
            seen.push(pair);
        }
        seen
    });
    flip();
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    poller.join().expect("poller")
}

/// `set_enabled_many` flips two rows in one transaction, seen whole by a
/// second connection, and touches no other column.
#[test]
fn set_enabled_many_flips_two_rows_seen_whole_by_a_second_connection() {
    let h = Harness::new();
    let a = h.create("a", 1);
    h.create("b", 2);
    let seen = poll_while(&h.path, ["a", "b"], || {
        for round in 0..200 {
            let on = round % 2 == 1;
            assert!(
                h.store
                    .set_enabled_many(&[(name("a"), on), (name("b"), on)])
                    .expect("flip")
            );
        }
    });
    assert!(!seen.is_empty());
    assert!(
        seen.iter().all(|(x, y)| x == y),
        "a second connection saw one row flipped without the other"
    );
    let after = h.store.get(a.id).expect("get").expect("row");
    assert!(after.enabled);
    assert_eq!(
        (after.generation, after.spec, after.next_run_at),
        (a.generation, a.spec, a.next_run_at)
    );
    assert!(h.store.set_enabled_many(&[]).expect("empty list"));
}

/// A flip naming a missing row changes neither row.
#[test]
fn set_enabled_many_naming_a_missing_row_changes_nothing() {
    let h = Harness::new();
    h.create("a", 1);
    h.create("b", 2);
    let changed = h
        .store
        .set_enabled_many(&[(name("a"), false), (name("missing"), false)])
        .expect("flip");
    assert!(!changed);
    assert_eq!(flags_of(&h.sql(), &["a", "b"]), [Some(true), Some(true)]);
}

/// `replace_and_set_enabled` creating a disabled row while enabling another,
/// and back, polled by a second connection, never shows both enabled.
#[test]
fn replace_and_set_enabled_never_shows_both_rows_enabled() {
    let h = Harness::new();
    h.create("builtin", 1);
    let seen = poll_while(&h.path, ["builtin", "python"], || {
        for round in 0..100 {
            let python_on = round % 2 == 0;
            let written = h
                .store
                .replace_and_set_enabled(
                    &new_schedule("python"),
                    python_on,
                    stamp(2),
                    &[(name("builtin"), !python_on)],
                )
                .expect("write")
                .expect("builtin is active");
            assert_eq!(written.enabled, python_on);
        }
    });
    assert!(!seen.is_empty());
    assert!(
        !seen.iter().any(|(b, p)| *b && *p),
        "a second connection saw both rows enabled"
    );
}

/// A replace of an active row keeps its id, raises its generation, keeps the
/// given flag, clears its run state and fences a run of the old generation.
#[test]
fn replace_of_an_active_row_keeps_its_id_and_fences_the_old_generation() {
    let h = Harness::new();
    let first = h.create("python", 1);
    assert!(
        h.store
            .record_run(first.id, first.generation, stamp(2), &RunOutcome::Ok)
            .expect("run")
    );
    let mut replacement = new_schedule("python");
    replacement.spec.payload = serde_json::json!({"module": "new"});
    let replaced = h
        .store
        .replace_and_set_enabled(&replacement, false, stamp(3), &[])
        .expect("replace")
        .expect("written");
    assert_eq!(replaced.id, first.id);
    assert_eq!(replaced.generation, generation(2));
    assert!(!replaced.enabled);
    assert_eq!(replaced.spec.payload, serde_json::json!({"module": "new"}));
    assert_eq!((replaced.last_run_at, replaced.last_outcome), (None, None));
    assert!(
        !h.store
            .record_run(first.id, first.generation, stamp(4), &RunOutcome::Ok)
            .expect("fenced run"),
        "a run of the old generation is fenced"
    );
    // An ended row is restarted in place, with the given flag.
    assert!(
        h.store
            .stop(replaced.id, replaced.generation, stamp(5), "done")
            .expect("stop")
    );
    let restarted = h
        .store
        .replace_and_set_enabled(&new_schedule("python"), true, stamp(6), &[])
        .expect("restart")
        .expect("written");
    assert_eq!(
        (restarted.id, restarted.generation, restarted.enabled),
        (first.id, generation(3), true)
    );
    assert!(restarted.is_active());
}

/// A call naming a missing row changes nothing: the written row is neither
/// created nor replaced; a list naming the written row is `InvalidSpec`.
#[test]
fn replace_and_set_enabled_naming_a_missing_row_changes_nothing() {
    let h = Harness::new();
    let builtin = h.create("builtin", 1);
    let refused = h
        .store
        .replace_and_set_enabled(
            &new_schedule("python"),
            true,
            stamp(2),
            &[(name("builtin"), false), (name("missing"), false)],
        )
        .expect("call");
    assert_eq!(refused, None);
    assert!(
        h.store
            .list_active()
            .expect("list")
            .iter()
            .all(|s| s.spec.name.as_str() != "python")
    );
    assert_eq!(h.store.get(builtin.id).expect("get"), Some(builtin));
    let named_itself = h.store.replace_and_set_enabled(
        &new_schedule("python"),
        true,
        stamp(2),
        &[(name("python"), false)],
    );
    assert!(
        matches!(named_itself, Err(CronError::InvalidSpec(_))),
        "{named_itself:?}"
    );
}
