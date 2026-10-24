//! The file schema: BD-CACHE v3 unchanged plus additive extensions (ADR-STORE-0001).

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
    ("sync_state", "backend_kind", "TEXT"),
    ("sync_state", "endpoint", "TEXT"),
    ("sync_state", "replica_id", "TEXT"),
    ("sync_state", "journal_seq", "INTEGER"),
];

/// Created after the extension columns exist.
pub(crate) const EXTENSION_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS beads_membership_unknown ON beads(id) WHERE membership_known IS NULL;
";

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
