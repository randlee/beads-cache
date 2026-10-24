# bd-cache-store Architecture

Status: Draft (sprint a-4; the crate ADRs below await lead approval). Crate-local decisions (domain
code `STORE`). Repo-level context: [`../architecture.md`](../architecture.md). Requirements:
[`requirements.md`](requirements.md). Created by sprint a-4.

`bd-cache-store` implements a-1's `CacheStore` and `DrainLock` ports
(`bd-cache-model`) over one SQLite file per dolt database, the file
BD-CACHE v3's `bd_cache.py` uses. It depends on `bd-cache-model`, `sc-beads`,
rusqlite (bundled, NFR-BDC-0007), fs4 (the drain lock) and serde_json (the
`row_json` and ready-base text; see Hand-off notes in the sprint doc).

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

**Status:** Draft (sprint a-4; P-2 fallback, awaiting lead ruling)

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
ALTER TABLE beads ADD COLUMN membership_known INTEGER;   -- ADR-STORE-0002
ALTER TABLE beads ADD COLUMN revision TEXT;              -- ADR-BDC-0004
ALTER TABLE journal ADD COLUMN applied_at TEXT;          -- set by apply_with_readback
ALTER TABLE sync_state ADD COLUMN backend_kind TEXT;     -- ADR-BDC-0004
ALTER TABLE sync_state ADD COLUMN endpoint TEXT;
ALTER TABLE sync_state ADD COLUMN replica_id TEXT;
ALTER TABLE sync_state ADD COLUMN journal_seq INTEGER;
CREATE INDEX beads_membership_unknown ON beads(id) WHERE membership_known IS NULL;
```

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

**Status:** Draft (sprint a-4, awaiting lead ruling)

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
  transaction under the 5 s busy timeout, as `bd_cache.py`'s writes are.

Consequences: rows and memberships stay consistent across a `bd_cache.py`
prune, rebuild or sync of the same file (tests `compat.rs`, `store.rs`).
`bd_cache.py` journal args that a-1's `Op::parse` refuses (for example
`{"add_labels": []}`, ADR-MODEL-0001) read as `BAD_ARGS` naming the entry.

## ADR-STORE-0003: One connection per process, and the store's own failures

**Status:** Draft (sprint a-4; the failure mapping is a stopgap pending an a-1 amendment)

Context: NFR-BDC-0007 allows one connection to the cache file per process.
a-1's `CacheError` has a code for every BD-CACHE outcome but none for a
failure of the store itself (an SQLite error, a file that cannot be
created, a stored value that does not decode); its trait docs say a method
may return "the implementation's own storage failures".

Decision:
- `SqliteStore::open_at` claims the canonical path of the file in a
  process-wide set (a `Mutex<BTreeSet<PathBuf>>`, released when the store
  drops); a second open of the same file in the same process is refused.
- Until a-1 adds a code, the store's own failures are
  `BD_CACHE.NO_DATABASE` ("the cache file cannot be used"), with the step,
  the file and the whole source chain rendered in `detail`. A stored row
  that does not decode, an unknown edge type, an invalid stored id or
  timestamp are the same code, naming the entry; they are never skipped.
  Journal text that no longer parses keeps `Op::parse`'s own code
  (`BAD_OP` / `BAD_ARGS` / `BAD_PRECONDITION`) with the entry's seq. One
  function (`error::failed` / `error::corrupt`) makes the mapping.

Consequences: the source chain is kept as text, not as `Error::source`,
until the a-1 amendment (a `STORE_FAILED`-style variant carrying a source)
is ruled; the change is then local to `error.rs`.
