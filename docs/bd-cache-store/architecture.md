# bd-cache-store Architecture

Status: Approved (lead ruling, bc-lead, 2026-10-24). Crate-local decisions (domain
code `STORE`). Repo-level context: [`../architecture.md`](../architecture.md). Requirements:
[`requirements.md`](requirements.md). Created by sprint a-4.

`bd-cache-store` implements a-1's `CacheStore` and `DrainLock` ports
(`bd-cache-model`) over one SQLite file per dolt database, the file
BD-CACHE v3's `bd_cache.py` uses. It depends on `bd-cache-model`, `sc-beads`,
rusqlite (bundled, NFR-BDC-0007), fs4 (the drain lock) and `serde_json` (the
`row_json` and ready-base text), as the Crate Layout of
[`../architecture.md`](../architecture.md) lists them.

| Module | Contents |
|---|---|
| `location` | `DoltDatabase`, `ProjectId`, `DatabaseIdentity` (`.beads/metadata.json`, `$BEADS_DIR` aware), `CacheLocation`, `load_repo_config` and `ConfigLoadError` |
| `schema` | the v3 DDL as `bd_cache.py` runs it, the extension tables, columns and indexes, the membership take-over (ADR-STORE-0001, 0002) |
| `store` | `SqliteStore`: open (settings, version, identity), every `CacheStore` method |
| `codec` | encoding and validated decoding of stored rows, edges, journal entries, ready bases and timestamps |
| `lock` | `FlockDrainLock` and its guard |
| `registry` | the process-wide claim that keeps one connection per cache file (ADR-STORE-0003) |
| `error` | the one mapping of the store's own failures to `CacheError` (ADR-STORE-0003) |

## ADR-STORE-0001: The file schema: v3 unchanged plus additive extensions

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by ADR-STORE-0004 (a-7: `ready_base_blocked`, `ready_base.indexed`, `journal.applied_epoch`, `journal_applied_epoch`, `journal_applied_unknown`; the index `journal_applied_at` of an earlier a-7 build is dropped)

Context: the file is shared with `bd_cache.py` (REQ-BDC-0804) at
`user_version` 3 (REQ-BDC-0103). Phase a needs scopes with membership
(ADR-BDC-0001), a per-scope ready base (ADR-BDC-0002), `applied_at` for the
ready adjustment, and room for an HTTP backend (ADR-BDC-0004). P-2 (agreement
on atm-bd-orchestration#557) is open; its fallback is additive tables and
nullable columns only, with no version change.

Decision: the v3 tables and indexes are created with `bd_cache.py`'s DDL
text, unchanged (`meta`, `beads`, `edges`, `journal`, `sync_state`;
`beads_phase`, `edges_target`, `journal_pending`). The extensions, created on
open when absent, are:

```sql
CREATE TABLE scope (name TEXT PRIMARY KEY, kind TEXT NOT NULL DEFAULT 'phase');
CREATE TABLE scope_member (scope TEXT NOT NULL, bead TEXT NOT NULL, PRIMARY KEY (scope, bead));
CREATE INDEX scope_member_bead ON scope_member(bead);
CREATE TABLE ready_base (scope TEXT PRIMARY KEY, blocked_json TEXT NOT NULL,
  ready_json TEXT NOT NULL, fetched_at TEXT NOT NULL, stale INTEGER NOT NULL DEFAULT 0);
CREATE TABLE ready_base_blocked (scope TEXT NOT NULL, bead TEXT NOT NULL,
  blocked_by_json TEXT NOT NULL, PRIMARY KEY (scope, bead)) WITHOUT ROWID;   -- ADR-STORE-0004
ALTER TABLE ready_base ADD COLUMN indexed INTEGER;       -- ADR-STORE-0004
ALTER TABLE beads ADD COLUMN membership_known INTEGER;   -- ADR-STORE-0002
ALTER TABLE beads ADD COLUMN revision TEXT;              -- ADR-BDC-0004
ALTER TABLE journal ADD COLUMN applied_at TEXT;          -- set by apply_with_readback
ALTER TABLE journal ADD COLUMN applied_epoch INTEGER;    -- ADR-STORE-0004: Unix seconds of applied_at
ALTER TABLE sync_state ADD COLUMN backend_kind TEXT;     -- ADR-BDC-0004
ALTER TABLE sync_state ADD COLUMN endpoint TEXT;
ALTER TABLE sync_state ADD COLUMN replica_id TEXT;
ALTER TABLE sync_state ADD COLUMN journal_seq INTEGER;
CREATE INDEX beads_membership_unknown ON beads(id) WHERE membership_known IS NULL;
DROP INDEX IF EXISTS journal_applied_at;                 -- ADR-STORE-0004: an earlier build's expression index
CREATE INDEX journal_applied_epoch ON journal(applied_epoch) WHERE state = 'applied';          -- ADR-STORE-0004
CREATE INDEX journal_applied_unknown ON journal(seq) WHERE state = 'applied' AND applied_epoch IS NULL;  -- ADR-STORE-0004
```

No index holds a function (ADR-STORE-0002, ADR-STORE-0004). Open also runs the `applied_epoch`
backfill of ADR-STORE-0004.

Scopes are keyed by phase name (`ScopeId::name`), as the v3 `phase` columns
are. `row_json` is the normalized row (REQ-BDC-0108) with its outgoing edges
as `{issue_id, depends_on_id, type}`, rendered as Python's
`json.dumps(sort_keys=True)`; the `phase` column is the first valid
`phase-<x>` label. On read, `dependencies` comes from `edges` (the authority;
`bd_cache.py` keeps bd's full dependency objects in `row_json`), and the rest
of `row_json` is decoded with the row model's serde shape (unknown keys
refused, ids validated), the exact inverse of the encoding. The ready base's
lists are JSON arrays of `BlockedRow` and bead ids. The outage and sync
alerts are the v3 `meta` keys (`outage_since`, `outage_error`,
`sync_alert:<phase>`); clearing the outage also clears
`outage_announced_at`, as `bd_cache.py` does. `user_version` stays 3: 0 is
initialized, 3 opened, any other value refused and left unchanged.

Consequences: `bd_cache.py` opens the extended file (its `CREATE ... IF NOT
EXISTS` and column-named inserts ignore the additions) and its `status`
shows the extra `sync_state` columns. A later version change is an explicit
migration command, never done on open.

## ADR-STORE-0002: Sharing the file with bd_cache.py

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); revised by fix-a-4-2 (lead ruling, bc-lead, 2026-10-24)

Context: `bd_cache.py` writes the same file and knows nothing of the
extensions. Its `INSERT OR REPLACE INTO beads(...)` resets the extension
columns of the rows it stores; its `prune` deletes beads by the `phase`
column; its rebuild deletes `beads`, `edges` and `sync_state`. Either may run
while the other has the file open.

Decision:
- **Membership.** `beads.membership_known` is 1 on every row this store
  writes. A row with `membership_known IS NULL` was last written by
  `bd_cache.py`; its membership is its v3 `phase` column, which is how
  `bd_cache.py` itself lists a phase. Reads apply that rule directly
  (`rows_in_scope`, `status`), so a row `bd_cache.py` writes while this store
  is open reads correctly at once. Open and `prune_scope` take such rows
  over (membership from `phase`, `membership_known = 1`) and drop
  `scope_member` rows whose bead no longer exists; an upsert takes over the
  row it stores. A bead deleted by `bd_cache.py` and stored again unscoped is
  a new row with no membership, so stale memberships never revive.
- **Ready bases** that `bd_cache.py`'s rebuild or prune leaves behind are
  kept; the next sync of the scope replaces them (a-6), and their rows are
  read as absent until then.
- **Drain lock.** The lock file is named as v3 names it (the cache file's
  extension replaced by `drain.lock`), so the two drains exclude each other.
- **`synced_at`.** The v3 column is `NOT NULL` but neither implementation
  reads it; `upsert_rows` takes no time, so the store writes SQLite's
  current UTC time, as `bd_cache.py` writes its own clock. No value the port
  returns depends on it; every other timestamp comes from the caller.
- **Concurrency.** Every mutating method is one `BEGIN IMMEDIATE`
  transaction under the 5 s busy timeout, as `bd_cache.py`'s writes are,
  single-statement ones included (`SqliteStore::write`). Every read that
  needs more than one value sees one snapshot: `status`, `get_row` and
  `rows_in_scope` run in one read transaction (`SqliteStore::read`);
  `outage` (both meta keys) and `open_work` (both counts) are one statement
  each. Open checks `user_version` and `project_id` (and creates the
  schema) before it sets WAL, so a refused file keeps its journal mode;
  `bd_cache.py`'s own order (WAL first) does not constrain ours.
- **Database names.** `DoltDatabase` is stricter than `bd_cache.py` (which
  refuses only `/`): the name becomes a file name on every supported
  platform, so `\`, `: * ? " < > |`, control characters, a trailing dot or
  space and the Windows device names (`CON`, `PRN`, `AUX`, `NUL`,
  `COM1`-`COM9`, `LPT1`-`LPT9`, `CONIN$`, `CONOUT$`, any case, any extension) are
  `NO_DATABASE`. A database `bd_cache.py` accepts with such a name cannot be
  shared until it is renamed.
- **`applied_since`** is bounded in SQL by an instant, never text (ADR-STORE-0004):
  `applied_epoch >= ?`, or, for an applied entry with no epoch, its time
  `COALESCE(applied_at, updated_at)` read by `unixepoch()` in the query; the two are
  separate index range reads joined by `UNION ALL` (`APPLIED_SINCE_SQL`, ADR-STORE-0004).
  **No expression index.** No index holds a
  function, `unixepoch()` included: an index on `unixepoch(..)` would make the
  file's schema unreadable, and every journal write fail, for a `bd_cache.py`
  linked against SQLite older than 3.38 (Ubuntu 22.04 ships 3.37.2). The epoch
  is a plain column (ADR-STORE-0004) so that plain partial indexes serve the query.

Consequences: rows and memberships stay consistent across a `bd_cache.py`
prune, rebuild or sync of the same file (tests `compat.rs`, `store.rs`).
`bd_cache.py` journal args that a-1's `Op::parse` refuses (for example
`{"add_labels": []}`, ADR-MODEL-0001) read as `BAD_ARGS` naming the entry.

## ADR-STORE-0003: One connection per process, and the store's own failures

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); failure mapping amended for a-1 amendment 1 (`STORE_FAILED`); revised by fix-a-4-2 (lead ruling, bc-lead, 2026-10-24)

Context: NFR-BDC-0007 allows one connection to the cache file per process.
The store can fail on its own (an SQLite error, a file that cannot be
created, a stored value that does not decode); a-1 amendment 1 gives those
failures their own code, `BD_CACHE.STORE_FAILED`, carrying the underlying
error as its source.

Decision:
- `SqliteStore::open_at` opens the file, then claims its canonical path
  (symlinks and `.` segments resolved) in a process-wide set (a
  `Mutex<BTreeSet<PathBuf>>`, released when the store drops); a second open
  of the same file in the same process, under any path that resolves to it,
  is refused and its connection closed before it reads or writes. Known
  limit: `canonicalize` resolves symlinks, not hard links, so a hard link to
  the cache file is a second path with its own claim; do not hard-link the
  cache file.
- The store's own failures are `BD_CACHE.STORE_FAILED`
  (`CacheError::store_failed`): `detail` names the step and the file, and
  the underlying error (rusqlite, I/O, `serde_json`) is kept as the error's
  `source` when there is one. `STORE_FAILED` has no source for: any stored
  value that does not decode or is inconsistent (`row_json`, an edge or
  scope kind, an id, timestamp or phase name, journal fields, the ready
  base, an `outage_since` without `outage_error`), each naming the entry
  and never skipped; a refused one-connection claim (NFR-BDC-0007); and a
  journal mode SQLite will not set to WAL. A type-conversion error
  rusqlite raises while reading a column keeps that rusqlite error as its
  source (`failed`); a value the store's own validation refuses after
  reading has no source (`corrupt`). `NO_DATABASE` means only missing or
  invalid `metadata.json` (REQ-BDC-0102).
  Journal text that no longer parses keeps `Op::parse`'s own code
  (`BAD_OP` / `BAD_ARGS` / `BAD_PRECONDITION`) with the entry's seq. One
  module makes the mapping: `error::failed` (with a source) and
  `error::corrupt` (without).
- `ConfigLoadError` (`load_repo_config`) carries its cause one way only: as
  `Error::source`. Its `Display` names the file and the fix, without the
  cause, so a reporter walking the source chain prints the cause once.

Consequences: a caller tells a broken cache file (`STORE_FAILED`) from a
workspace without bd metadata (`NO_DATABASE`), and walks the source chain
with `Error::source`.

## ADR-STORE-0004: A per-bead index of the ready base, and an index for the journal check (a-7 amendment)

**Status:** Approved (lead ruling, bc-lead, 2026-10-24, fix-a-7-2; journal indexes revised by lead ruling S18); amends ADR-STORE-0001 (the extension tables, columns and indexes)
and ADR-STORE-0002 (what is read as one snapshot, and what a foreign writer may leave)

Context: `CacheStore::base_entry` and `has_entries_since` (ADR-MODEL-0003's a-7
amendment, ADR-CORE-0206) must not cost more as the scope or the journal grows.

Decision:
- An additive table `ready_base_blocked(scope, bead, blocked_by_json)`
  (`WITHOUT ROWID`, primary key `(scope, bead)`) holds one row per blocked bead,
  written in the same transaction as `ready_base` by `replace_ready_base` and
  deleted with it by a rebuild and by `prune_scope`; a nullable column
  `ready_base.indexed` is 1 when the index belongs to the stored base. The
  blockers are merged in order without repeats; a bead without blockers has no row.
- `base_entry` runs in one read transaction (`SqliteStore::read`), so the head
  and the bead's row are one snapshot even while another process replaces the
  base. A base with `indexed` NULL is read whole, inside the same transaction.
- **No function in any index.** `bd_cache.py` may be linked against an SQLite older
  than 3.38 (Ubuntu 22.04 ships 3.37.2), which has no `unixepoch()`. Such a build
  cannot evaluate an index whose key uses it: it would fail to use the file's schema
  and every `journal` write of that writer would fail. The a-7 build that added
  `journal_applied_at ON journal(unixepoch(applied_at))` is therefore withdrawn: open
  runs `DROP INDEX IF EXISTS journal_applied_at`, so a file written by that build
  is made readable again.
- **`journal.applied_epoch`**, a nullable `INTEGER`: the Unix seconds of the instant
  `SqliteStore::apply_with_readback` records in `applied_at`, written in the same
  `UPDATE` that sets `state = 'applied'`. Two plain partial indexes serve the check:
  `journal_applied_epoch ON journal(applied_epoch) WHERE state = 'applied'`, and
  `journal_applied_unknown ON journal(seq) WHERE state = 'applied' AND applied_epoch IS NULL`
  (entries with no epoch).
- **Backfill** (`BACKFILL_APPLIED_EPOCH`, an `UPDATE` through `journal_applied_unknown`):
  for each applied row with no epoch, `applied_epoch = unixepoch(COALESCE(applied_at, updated_at))`,
  where SQLite can read that time. This process's bundled SQLite evaluates
  `unixepoch()`; no index mentions it. `bd_cache.py` writes `%Y-%m-%dT%H:%M:%SZ`,
  which SQLite reads. It runs in the open transaction, and again (QA4-001) at the start of
  `has_entries_since` and `applied_since` whenever `journal_applied_unknown` holds an entry
  whose time SQLite can read: one `bd_cache.py` applied while the store is open. The test
  for that is one probe of the index, empty in a healthy file, so the journal check's cost
  does not grow with what that writer applies; without the dating each such entry would
  cost the check one `unixepoch()` call (20 000 of them made a cached `get` 1.7 ms). The
  read-path `UPDATE` is best effort: it runs with no busy timeout, so a writer holding the
  lock, or a read-only file, leaves the entries undated for now, and the check answers the
  same from them, only slower; the next read tries again. A cached `get` may therefore write
  the file (these epochs only); it never waits for the write lock to do so.
- **Unreadable times count as touched.** A row whose `COALESCE(applied_at, updated_at)`
  SQLite cannot read as a time keeps a NULL epoch, stays in `journal_applied_unknown`,
  and counts as applied since any instant (`COALESCE(... >= ?1, 1)`). Reporting
  activity that did not happen only costs a fresh read; missing it would serve a
  stale ready answer. `applied_since` agrees with it. The cost: while such a row exists,
  every cached `get` of a bead with a stored base takes the adjusted path (the whole base
  is recomputed). So open, after the backfill, logs a warning naming the seqs of the
  applied entries still undated (the first 20, and their count). **Operator fix:** find the
  entry (`SELECT seq, idem_key, bead, updated_at FROM journal WHERE seq = <seq>`) and either
  resolve it (`UPDATE journal SET state = 'resolved' WHERE seq = <seq>`, which takes it out
  of the applied set) or delete it (`DELETE FROM journal WHERE seq = <seq>`); an applied
  entry is history only, so neither loses a pending write. Setting its `updated_at` to a
  real time works too; the next read dates it.
- **The pre-existing miss, fixed.** `bd_cache.py`'s `_mark` sets only `state` and
  `updated_at`, so an entry it applied has `applied_at` NULL. Before this fix
  `applied_since` and `has_entries_since` compared `applied_at` alone and so missed
  every Python-applied entry. Its time is now `COALESCE(applied_at, updated_at)`
  (backfilled, or read in the query), and the miss is closed.
- `has_entries_since` is three `EXISTS` lookups (`HAS_ENTRIES_SINCE_SQL`): `journal_pending`
  for pending entries; `journal_applied_epoch` for entries with an epoch; and
  `journal_applied_unknown` for those without. The last two are named with `INDEXED BY`,
  because the planner otherwise scans the applied rows through `journal_pending`. The
  first two cost an index probe each whatever the history; the third walks the entries
  without an epoch, so its cost is the number of those that are older than `at`. The
  backfill above keeps that number at zero (the entries with an unreadable time stop the
  walk at once: they count as since), so the check stays bounded; it is not bounded
  for entries the backfill could not yet date (the write lock was busy).
- `applied_since` is two range reads joined by `UNION ALL` and ordered by seq
  (`APPLIED_SINCE_SQL`): `journal_applied_epoch` for `applied_epoch >= at`, and
  `journal_applied_unknown` for the undated entries. One `WHERE` with an `OR` of the two
  (the earlier form) lets neither partial index serve it, so it read every applied entry.
- **A missing hinted index.** An index another writer dropped makes `INDEXED BY` an error
  (`SQLITE_ERROR`, `no such index`; matched on the code and the message). The store then
  logs one warning, remembers it, and from then on asks the unhinted form of each journal
  read (`HAS_ENTRIES_SINCE_UNHINTED_SQL`, `APPLIED_SINCE_UNHINTED_SQL`, and the backfill's):
  the same question, correct, slower on a long history. The hints are not tried again
  until the next open, which recreates the indexes. Tests assert the plans of the hinted
  queries, their SQL work in VM steps, and that the fallback is not retried.
- **`bd_cache.py` compatibility** (read at atm-core
  `05831dfe866d389feed138add3c29cb93cd6c82b`, `bd_cache.py`): it never creates,
  reads or writes `ready_base`, `ready_base_blocked` or `scope_member`. Its
  rebuild deletes `edges`, `beads` and `sync_state` (lines 359-361); its prune
  deletes a phase's `edges`, `beads`, `journal` and `sync_state` rows (611-614);
  every other write is `INSERT OR REPLACE` on `meta` and `beads`. So a stored
  base and its index survive its rebuild and prune together and stay in
  agreement (ADR-STORE-0002: the next sync replaces them). Were a foreign writer to
  `INSERT OR REPLACE` a `ready_base` row without the new column, `indexed` would
  reset to NULL and the left-over index rows would be ignored; an in-place
  `UPDATE` of `blocked_json` is the one write the index could not see, and no
  known writer does it. The tests below pin each case.
- A file written before this ADR (no table, no column) opens unchanged
  (`CREATE TABLE IF NOT EXISTS`, `ALTER TABLE ... ADD COLUMN` when absent) and its
  base reads whole until the next replace indexes it; its applied entries are dated by the backfill. The file stays at
  `user_version` 3.

Consequences: the contract suite's `base_entry_rules` and `has_entries_since_rules`
pin both methods to their provided bodies for every store; the store tests
`a_python_style_rebuild_or_prune_leaves_the_base_and_its_index_in_agreement`,
`a_base_replaced_by_another_writer_is_read_from_its_blob_not_from_a_stale_index`,
`a_file_from_before_the_ready_base_index_opens_and_migrates_on_the_next_replace`
and `the_entries_since_check_uses_indexes_and_ignores_a_long_history` check the
compatibility claims; `entries_python_applies_while_open_are_dated_by_the_next_check`,
`the_check_does_not_wait_for_the_write_lock_to_date_entries`,
`applied_since_reads_both_partial_indexes_and_no_history`,
`the_entries_since_check_survives_a_dropped_index` (store tests),
`the_open_finds_the_applied_entries_it_could_not_date` (unit) and
`a_cached_get_dates_entries_python_applied_while_open` (bd-cache-sync `perf.rs`) check the
dating, the plans and SQL work (VM steps), the undatable-entry warning and the hint fallback.
