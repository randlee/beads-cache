//! The file schema: BD-CACHE v3 unchanged plus additive extensions (ADR-STORE-0001).

use crate::codec::journal_columns;

/// `PRAGMA user_version` of the shared file (REQ-BDC-0103); never changed on open.
pub const SCHEMA_VERSION: i32 = 3;

/// The BD-CACHE v3 DDL, byte for byte as the v3 Python writer (baseline `05831df`) runs it.
pub(crate) const V3_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS beads (
  id TEXT PRIMARY KEY, phase TEXT, status TEXT, updated_at TEXT,
  row_json TEXT NOT NULL, synced_at TEXT NOT NULL, stale INTEGER NOT NULL DEFAULT 0);
CREATE INDEX IF NOT EXISTS beads_phase ON beads(phase, status);
CREATE TABLE IF NOT EXISTS edges (
  issue_id TEXT NOT NULL, depends_on_id TEXT NOT NULL, type TEXT NOT NULL,
  PRIMARY KEY (issue_id, depends_on_id, type));
CREATE INDEX IF NOT EXISTS edges_target ON edges(depends_on_id, type);
CREATE TABLE IF NOT EXISTS journal (
  seq INTEGER PRIMARY KEY AUTOINCREMENT, idem_key TEXT NOT NULL UNIQUE, phase TEXT,
  bead TEXT NOT NULL, op TEXT NOT NULL, args_json TEXT NOT NULL, precondition_json TEXT NOT NULL,
  actor TEXT NOT NULL, state TEXT NOT NULL CHECK (state IN ('pending','applied','escalated','resolved')),
  attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  announced_at TEXT);
CREATE INDEX IF NOT EXISTS journal_pending ON journal(state, bead, seq);
CREATE TABLE IF NOT EXISTS sync_state (
  phase TEXT PRIMARY KEY, last_started_at TEXT, last_ok_at TEXT, last_error TEXT);
";

/// Additive tables (ADR-BDC-0001, 0002; ADR-STORE-0006 adds `tick_lease`
/// and `hook_state`). The v3 tables are untouched.
pub(crate) const EXTENSION_TABLES: &str = "
CREATE TABLE IF NOT EXISTS scope (
  name TEXT PRIMARY KEY, kind TEXT NOT NULL DEFAULT 'phase');
CREATE TABLE IF NOT EXISTS scope_member (
  scope TEXT NOT NULL, bead TEXT NOT NULL, PRIMARY KEY (scope, bead));
CREATE INDEX IF NOT EXISTS scope_member_bead ON scope_member(bead);
CREATE TABLE IF NOT EXISTS ready_base (
  scope TEXT PRIMARY KEY, blocked_json TEXT NOT NULL, ready_json TEXT NOT NULL,
  fetched_at TEXT NOT NULL, stale INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS ready_base_blocked (
  scope TEXT NOT NULL, bead TEXT NOT NULL, blocked_by_json TEXT NOT NULL,
  PRIMARY KEY (scope, bead)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS tick_lease (
  id INTEGER PRIMARY KEY CHECK (id = 1), run_id TEXT, pid INTEGER, host TEXT, expires_at TEXT);
CREATE TABLE IF NOT EXISTS hook_state (
  id INTEGER PRIMARY KEY CHECK (id = 1), consecutive_failures INTEGER NOT NULL DEFAULT 0,
  first_failure_at TEXT, last_error TEXT, next_attempt_at TEXT);
";

/// The `meta` key holding the version of sc-cron's columns in this file
/// (ADR-BDC-0014): written on open when absent, checked by
/// `CacheScheduleStore`, never `user_version`.
pub const SC_CRON_SCHEDULE_VERSION_KEY: &str = "sc_cron_schedule_version";

/// The version of sc-cron's `sc_cron_schedule` columns this store creates
/// and serves (sc-cron's `SCHEMA_VERSION`, 2).
pub(crate) const SC_CRON_SCHEDULE_VERSION: i64 = sc_cron::SCHEMA_VERSION;

/// The schedule table (ADR-BDC-0014, ADR-STORE-0006): sc-cron's version-2
/// columns and checks, with sc-cron's own size limits, created when absent.
pub(crate) fn sc_cron_schedule_table() -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS sc_cron_schedule (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    generation   INTEGER NOT NULL CHECK (generation BETWEEN 1 AND 4294967295),
    name         TEXT    NOT NULL UNIQUE
                 CHECK (length(CAST(name AS BLOB)) BETWEEN 1 AND {name}),
    expr         TEXT    NOT NULL CHECK (length(CAST(expr AS BLOB)) <= {expr}),
    timeout_ms   INTEGER NOT NULL CHECK (timeout_ms > 0),
    jitter_ms    INTEGER NOT NULL CHECK (jitter_ms >= 0),
    payload      TEXT    NOT NULL
                 CHECK (length(CAST(payload AS BLOB)) <= {payload}),
    created_at   TEXT    NOT NULL,
    next_run_at  TEXT,
    last_run_at  TEXT,
    last_outcome TEXT,
    ended_at     TEXT,
    stop_reason  TEXT,
    enabled      INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    CHECK ((ended_at IS NULL) = (next_run_at IS NOT NULL)),
    CHECK ((ended_at IS NULL) = (stop_reason IS NULL))
);",
        name = sc_cron::MAX_NAME_LEN,
        expr = sc_cron::MAX_EXPR_LEN,
        payload = sc_cron::MAX_PAYLOAD_BYTES,
    )
}

/// Additive nullable columns: `(table, column, type)` (ADR-BDC-0004, ADR-STORE-0001).
pub(crate) const EXTENSION_COLUMNS: &[(&str, &str, &str)] = &[
    // 1 when this store last wrote the row and its scope_member rows are
    // authoritative; NULL when another implementation (the v3 Python writer) replaced
    // the row, whose membership is then its v3 `phase` column.
    ("beads", "membership_known", "INTEGER"),
    // 1 when the scope's ready_base_blocked rows were written with the base
    // (they index its blocked rows by bead); NULL for a base stored before.
    ("ready_base", "indexed", "INTEGER"),
    ("beads", "revision", "TEXT"),
    ("journal", "applied_at", "TEXT"),
    // Unix seconds of `applied_at`, filled when this store marks an entry
    // applied (ADR-STORE-0004); NULL for entries the v3 Python writer applied.
    ("journal", "applied_epoch", "INTEGER"),
    // The `CODE` of `last_error`, written with it by this store; NULL when
    // the v3 Python writer wrote the error (read from the text's `CODE: ` prefix).
    ("journal", "last_error_code", "TEXT"),
    // The resolve note, `resolved by <actor>: <note>`, written by this
    // store's resolve; NULL when the v3 Python writer resolved (it appends the note to
    // `last_error`, split out on read).
    ("journal", "note", "TEXT"),
    // 1 / 0: whether the write changed the view when it was journaled, as a
    // duplicate returns it (lead ruling S62); NULL for the v3 Python writer's entries.
    ("journal", "changed", "INTEGER"),
    ("sync_state", "backend_kind", "TEXT"),
    ("sync_state", "endpoint", "TEXT"),
    ("sync_state", "replica_id", "TEXT"),
    ("sync_state", "journal_seq", "INTEGER"),
    // ADR-STORE-0006: opaque, stored per scope by `ingest_scope`.
    ("scope", "revision", "TEXT"),
    // ADR-STORE-0006 / ADR-BDC-0013: NULL = the phase-a label scope;
    // otherwise the scope's filter as JSON.
    ("scope", "filter", "TEXT"),
    // ADR-STORE-0006 / REQ-BDC-0728: NULL = not failing.
    ("sync_state", "failing_since", "TEXT"),
    ("sync_state", "notified_since", "TEXT"),
    // ADR-STORE-0006 / REQ-BDC-0727: NULL = never offered to a hook.
    ("journal", "first_offered_at", "TEXT"),
];

/// Created after the extension columns exist.
///
/// No index holds a function (ADR-STORE-0002, ADR-STORE-0004): the v3 Python writer
/// on an SQLite older than 3.38 (Ubuntu 22.04 ships 3.37.2) cannot evaluate
/// `unixepoch()`, and an index that used it would make every journal write
/// of that writer fail. `journal_applied_at` of an earlier build of this
/// store was such an index and is dropped.
pub(crate) const EXTENSION_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS beads_membership_unknown ON beads(id) WHERE membership_known IS NULL;
DROP INDEX IF EXISTS journal_applied_at;
CREATE INDEX IF NOT EXISTS journal_applied_epoch ON journal(applied_epoch) WHERE state = 'applied';
CREATE INDEX IF NOT EXISTS journal_applied_unknown ON journal(seq) WHERE state = 'applied' AND applied_epoch IS NULL;
";

/// Fills `journal.applied_epoch` for entries applied without it: by
/// the v3 Python writer (which sets `state` and `updated_at` and never `applied_at`),
/// or before the column existed. The time of such an entry is `applied_at` if
/// it has one, else `updated_at`; a value SQLite cannot read as a time stays
/// NULL and keeps counting as "applied since" (conservative). `unixepoch()` is
/// evaluated by this process's bundled SQLite and appears in no index.
///
/// Run in the open transaction, and again by the reads that ask the journal
/// (`has_entries_since`, `applied_since`) when [`UNDATED_ABOVE_SQL`] finds
/// more than `READ_DATING_THRESHOLD` entries to date (ADR-STORE-0004, lead
/// ruling S26): the work is one `journal_applied_unknown` walk over the
/// entries not yet dated, so the journal check stays three index lookups
/// however many entries that writer applies while this store is open.
pub(crate) const BACKFILL_APPLIED_EPOCH: &str = "\
UPDATE journal INDEXED BY journal_applied_unknown \
SET applied_epoch = unixepoch(COALESCE(applied_at, updated_at)) \
WHERE state = 'applied' AND applied_epoch IS NULL \
  AND unixepoch(COALESCE(applied_at, updated_at)) IS NOT NULL";

/// [`BACKFILL_APPLIED_EPOCH`] without the index hint (see [`HAS_ENTRIES_SINCE_UNHINTED_SQL`]).
pub(crate) const BACKFILL_APPLIED_EPOCH_UNHINTED: &str = "\
UPDATE journal SET applied_epoch = unixepoch(COALESCE(applied_at, updated_at)) \
WHERE state = 'applied' AND applied_epoch IS NULL \
  AND unixepoch(COALESCE(applied_at, updated_at)) IS NOT NULL";

/// How many applied entries without an epoch have a seq above `?1`, counted
/// up to `?2`. `?1` is the store's floor: every undated entry at or below it
/// has an unreadable time (warned about once), so those are not walked again; `?2` caps
/// the walk just above the read-path dating threshold (ADR-STORE-0004, S26).
/// A `journal_applied_unknown` range read that evaluates no function: empty,
/// one index probe, in a healthy file.
pub(crate) const UNDATED_ABOVE_SQL: &str = "SELECT COUNT(*) FROM (SELECT 1 FROM journal \
    INDEXED BY journal_applied_unknown WHERE state = 'applied' AND applied_epoch IS NULL \
    AND seq > ?1 LIMIT ?2)";

/// [`UNDATED_ABOVE_SQL`] without the index hint.
pub(crate) const UNDATED_ABOVE_UNHINTED_SQL: &str = "SELECT COUNT(*) FROM (SELECT 1 FROM journal \
    WHERE state = 'applied' AND applied_epoch IS NULL AND seq > ?1 LIMIT ?2)";

/// The highest seq of an applied entry without an epoch, or NULL: one probe
/// of the end of `journal_applied_unknown`.
pub(crate) const LAST_UNDATED_SQL: &str = "SELECT MAX(seq) FROM journal \
    INDEXED BY journal_applied_unknown WHERE state = 'applied' AND applied_epoch IS NULL";

/// [`LAST_UNDATED_SQL`] without the index hint.
pub(crate) const LAST_UNDATED_UNHINTED_SQL: &str = "SELECT MAX(seq) FROM journal \
    WHERE state = 'applied' AND applied_epoch IS NULL";

/// The seqs in `(?1, ?2]` of applied entries without an epoch whose time
/// SQLite cannot read, by seq: the entries no dating will ever date. Asked
/// once for each range of seqs the store has not examined yet, so each
/// undated entry's time is read here at most once while the store is open.
pub(crate) const UNDATABLE_BETWEEN_SQL: &str = "SELECT seq FROM journal \
    INDEXED BY journal_applied_unknown WHERE state = 'applied' AND applied_epoch IS NULL \
    AND seq > ?1 AND seq <= ?2 AND unixepoch(COALESCE(applied_at, updated_at)) IS NULL \
    ORDER BY seq";

/// The lowest seq above `?1` of an applied entry without an epoch whose
/// time SQLite can read, or NULL: where the store's floor of known
/// unreadable entries must stop, so no entry the dating can date is hidden
/// below it. Walks the unreadable entries above `?1` until the first
/// readable one; asked only when the floor may rise.
pub(crate) const FIRST_DATEABLE_ABOVE_SQL: &str = "SELECT MIN(seq) FROM journal \
    INDEXED BY journal_applied_unknown WHERE state = 'applied' AND applied_epoch IS NULL \
    AND seq > ?1 AND unixepoch(COALESCE(applied_at, updated_at)) IS NOT NULL";

/// [`FIRST_DATEABLE_ABOVE_SQL`] without the index hint.
pub(crate) const FIRST_DATEABLE_ABOVE_UNHINTED_SQL: &str = "SELECT MIN(seq) FROM journal \
    WHERE state = 'applied' AND applied_epoch IS NULL \
    AND seq > ?1 AND unixepoch(COALESCE(applied_at, updated_at)) IS NOT NULL";

/// [`UNDATABLE_BETWEEN_SQL`] without the index hint.
pub(crate) const UNDATABLE_BETWEEN_UNHINTED_SQL: &str = "SELECT seq FROM journal \
    WHERE state = 'applied' AND applied_epoch IS NULL \
    AND seq > ?1 AND seq <= ?2 AND unixepoch(COALESCE(applied_at, updated_at)) IS NULL \
    ORDER BY seq";

/// The seqs of applied entries left without an epoch after a backfill: their
/// time cannot be read, so they count as "applied since" every instant and
/// keep every `get` of their scope on the adjusted (slow) path. Read at open
/// to warn (ADR-STORE-0004 names the operator's fix).
pub(crate) const UNDATED_SEQS: &str = "SELECT seq FROM journal \
    INDEXED BY journal_applied_unknown \
    WHERE state = 'applied' AND applied_epoch IS NULL ORDER BY seq";

/// `CacheStore::has_entries_since`: is anything pending, or applied at or
/// after `?1` (Unix seconds)? Three index lookups (`tests/store.rs` asserts
/// the query plan): pending entries (`journal_pending`); entries with an
/// epoch, by `applied_epoch` (`journal_applied_epoch`); and entries applied
/// without one (`journal_applied_unknown`), whose time is `applied_at` else
/// `updated_at`, and which count as applied since when that time is
/// unreadable. The last lookup walks the undated entries, so its cost grows
/// with their number: the store dates them (`BACKFILL_APPLIED_EPOCH`) at
/// open, and before a check that would walk more than
/// `READ_DATING_THRESHOLD` of them (S26), which leaves only entries with an
/// unreadable time, and the first of those answers `true`.
/// The planner, without statistics, scans the applied rows through
/// `journal_pending` unless told otherwise, so the last two name their index;
/// [`HAS_ENTRIES_SINCE_UNHINTED_SQL`] is the same question for a file another
/// writer dropped one of them from.
pub const HAS_ENTRIES_SINCE_SQL: &str = "SELECT \
    EXISTS (SELECT 1 FROM journal WHERE state = 'pending') OR \
    EXISTS (SELECT 1 FROM journal INDEXED BY journal_applied_epoch \
        WHERE state = 'applied' AND applied_epoch >= ?1) OR \
    EXISTS (SELECT 1 FROM journal INDEXED BY journal_applied_unknown \
        WHERE state = 'applied' AND applied_epoch IS NULL \
        AND COALESCE(unixepoch(COALESCE(applied_at, updated_at)) >= ?1, 1))";

/// [`HAS_ENTRIES_SINCE_SQL`] without the index hints: correct whatever indexes
/// exist, slower on a long history. Used, after one warning, for the rest of
/// the store's life once a hinted index is found missing (`INDEXED BY` is an
/// error then); the next open recreates the indexes.
pub const HAS_ENTRIES_SINCE_UNHINTED_SQL: &str = "SELECT \
    EXISTS (SELECT 1 FROM journal WHERE state = 'pending') OR \
    EXISTS (SELECT 1 FROM journal WHERE state = 'applied' AND applied_epoch >= ?1) OR \
    EXISTS (SELECT 1 FROM journal WHERE state = 'applied' AND applied_epoch IS NULL \
        AND COALESCE(unixepoch(COALESCE(applied_at, updated_at)) >= ?1, 1))";

/// `CacheStore::applied_since`: the entries applied at or after `?1` (Unix
/// seconds), by seq. Two index range reads joined by `UNION ALL` (disjoint:
/// one has an epoch, the other none), so neither walks the applied history:
/// entries with an epoch through `journal_applied_epoch`, and undated ones
/// through `journal_applied_unknown`, kept when their time is at or after
/// `?1` or unreadable. An `OR` of the two in one `WHERE` would let neither
/// partial index serve the query (`tests/store.rs` asserts the plan).
pub const APPLIED_SINCE_SQL: &str = concat!(
    "SELECT ",
    journal_columns!(),
    " FROM journal INDEXED BY journal_applied_epoch \
     WHERE state = 'applied' AND applied_epoch >= ?1 \
     UNION ALL SELECT ",
    journal_columns!(),
    " FROM journal INDEXED BY journal_applied_unknown \
     WHERE state = 'applied' AND applied_epoch IS NULL \
     AND COALESCE(unixepoch(COALESCE(applied_at, updated_at)) >= ?1, 1) \
     ORDER BY seq"
);

/// [`APPLIED_SINCE_SQL`] without the index hints.
pub const APPLIED_SINCE_UNHINTED_SQL: &str = concat!(
    "SELECT ",
    journal_columns!(),
    " FROM journal WHERE state = 'applied' AND applied_epoch >= ?1 \
     UNION ALL SELECT ",
    journal_columns!(),
    " FROM journal WHERE state = 'applied' AND applied_epoch IS NULL \
     AND COALESCE(unixepoch(COALESCE(applied_at, updated_at)) >= ?1, 1) \
     ORDER BY seq"
);

/// Membership of rows another implementation wrote, taken from their `phase`
/// column and made authoritative; leftover `scope_member` rows of deleted
/// beads are dropped (ADR-STORE-0002). Run inside a write transaction.
///
/// A v3 `phase` column names a phase-a scope only: it is never read as a
/// membership of a filter scope that has the same name (ADR-BDC-0013).
pub(crate) const RECONCILE_MEMBERSHIP: &str = "
DELETE FROM scope_member WHERE bead IN (SELECT id FROM beads WHERE membership_known IS NULL);
INSERT OR IGNORE INTO scope_member(scope, bead)
  SELECT phase, id FROM beads WHERE membership_known IS NULL AND phase IS NOT NULL
    AND NOT EXISTS (SELECT 1 FROM scope s WHERE s.name = beads.phase AND s.filter IS NOT NULL);
UPDATE beads SET membership_known = 1 WHERE membership_known IS NULL;
DELETE FROM scope_member WHERE bead NOT IN (SELECT id FROM beads);
";

/// SQL predicate: bead `b` is in the scope bound to `?1`, whoever wrote it
/// last. A row another writer left reads its v3 `phase` as a phase-a
/// scope's membership, never a filter scope's (ADR-BDC-0013). A literal
/// macro, so the statements built from it are `&'static str` (`concat!`).
macro_rules! in_scope {
    () => {
        "((b.membership_known IS NOT NULL AND EXISTS \
         (SELECT 1 FROM scope_member m WHERE m.scope = ?1 AND m.bead = b.id)) \
         OR (b.membership_known IS NULL AND b.phase = ?1 AND NOT EXISTS \
         (SELECT 1 FROM scope s WHERE s.name = ?1 AND s.filter IS NOT NULL)))"
    };
}
pub(crate) use in_scope;
