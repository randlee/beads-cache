//! The file schema: BD-CACHE v3 unchanged plus additive extensions (ADR-STORE-0001).

use crate::codec::journal_columns;

/// `PRAGMA user_version` of the shared file (REQ-BDC-0103); never changed on open.
pub const SCHEMA_VERSION: i32 = 3;

/// The BD-CACHE v3 DDL, byte for byte as `bd_cache.py` (atm-core `05831df`) runs it.
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

/// Additive tables (ADR-BDC-0001, 0002). The v3 tables are untouched.
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
";

/// Additive nullable columns: `(table, column, type)` (ADR-BDC-0004, ADR-STORE-0001).
pub(crate) const EXTENSION_COLUMNS: &[(&str, &str, &str)] = &[
    // 1 when this store last wrote the row and its scope_member rows are
    // authoritative; NULL when another implementation (bd_cache.py) replaced
    // the row, whose membership is then its v3 `phase` column.
    ("beads", "membership_known", "INTEGER"),
    // 1 when the scope's ready_base_blocked rows were written with the base
    // (they index its blocked rows by bead); NULL for a base stored before.
    ("ready_base", "indexed", "INTEGER"),
    ("beads", "revision", "TEXT"),
    ("journal", "applied_at", "TEXT"),
    // Unix seconds of `applied_at`, filled when this store marks an entry
    // applied (ADR-STORE-0004); NULL for entries bd_cache.py applied.
    ("journal", "applied_epoch", "INTEGER"),
    ("sync_state", "backend_kind", "TEXT"),
    ("sync_state", "endpoint", "TEXT"),
    ("sync_state", "replica_id", "TEXT"),
    ("sync_state", "journal_seq", "INTEGER"),
];

/// Created after the extension columns exist.
///
/// No index holds a function (ADR-STORE-0002, ADR-STORE-0004): `bd_cache.py`
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
/// `bd_cache.py` (which sets `state` and `updated_at` and never `applied_at`),
/// or before the column existed. The time of such an entry is `applied_at` if
/// it has one, else `updated_at`; a value SQLite cannot read as a time stays
/// NULL and keeps counting as "applied since" (conservative). `unixepoch()` is
/// evaluated by this process's bundled SQLite and appears in no index.
///
/// Run in the open transaction, and again by the reads that ask the journal
/// (`has_entries_since`, `applied_since`) whenever [`HAS_DATEABLE_SQL`] finds
/// an entry `bd_cache.py` applied since (ADR-STORE-0004): the work is one
/// `journal_applied_unknown` walk over the entries not yet dated, so the
/// journal check stays three index lookups however many entries that writer
/// applies while this store is open.
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

/// Is there an applied entry without an epoch whose time SQLite can read,
/// that is, one [`BACKFILL_APPLIED_EPOCH`] would date? A walk of
/// `journal_applied_unknown`, which after a backfill holds only the entries
/// with an unreadable time (warned about at open), so it is empty in a
/// healthy file and the check is one index probe.
pub(crate) const HAS_DATEABLE_SQL: &str = "SELECT EXISTS (SELECT 1 FROM journal \
    INDEXED BY journal_applied_unknown WHERE state = 'applied' AND applied_epoch IS NULL \
    AND unixepoch(COALESCE(applied_at, updated_at)) IS NOT NULL)";

/// [`HAS_DATEABLE_SQL`] without the index hint.
pub(crate) const HAS_DATEABLE_UNHINTED_SQL: &str = "SELECT EXISTS (SELECT 1 FROM journal \
    WHERE state = 'applied' AND applied_epoch IS NULL \
    AND unixepoch(COALESCE(applied_at, updated_at)) IS NOT NULL)";

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
/// open and before each check that finds one dateable, which leaves only
/// entries with an unreadable time, and the first of those answers `true`.
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
pub(crate) const RECONCILE_MEMBERSHIP: &str = "
DELETE FROM scope_member WHERE bead IN (SELECT id FROM beads WHERE membership_known IS NULL);
INSERT OR IGNORE INTO scope_member(scope, bead)
  SELECT phase, id FROM beads WHERE membership_known IS NULL AND phase IS NOT NULL;
UPDATE beads SET membership_known = 1 WHERE membership_known IS NULL;
DELETE FROM scope_member WHERE bead NOT IN (SELECT id FROM beads);
";

/// SQL predicate: bead `b` is in the scope bound to `?1`, whoever wrote it last.
pub(crate) const IN_SCOPE: &str = "((b.membership_known IS NOT NULL AND EXISTS \
    (SELECT 1 FROM scope_member m WHERE m.scope = ?1 AND m.bead = b.id)) \
    OR (b.membership_known IS NULL AND b.phase = ?1))";
