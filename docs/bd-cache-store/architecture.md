# bd-cache-store Architecture

Status: Approved (lead ruling, bc-lead, 2026-10-24). Crate-local decisions (domain
code `STORE`). Repo-level context: [`../architecture.md`](../architecture.md). Requirements:
[`requirements.md`](requirements.md). Created by sprint a-4.

`bd-cache-store` implements a-1's `CacheStore` and `DrainLock` ports
(`bd-cache-model`) over one SQLite file per dolt database, the file
the baseline Python implementation of BD-CACHE v3 uses. It depends on `bd-cache-model`, `sc-beads`
(also its logging, `sc_beads::logging::tracing`), rusqlite (bundled,
NFR-BDC-0007), fs4 (the drain lock) and `serde_json` (the `row_json` and
ready-base text), as the Crate Layout of [`../architecture.md`](../architecture.md)
lists them; ADR-STORE-0005 records them with the test-only features.

| Module | Contents |
|---|---|
| `location` | `DoltDatabase`, `ProjectId`, `DatabaseIdentity` (`.beads/metadata.json`, `$BEADS_DIR` aware), `BeadsDirOverride` and `BeadsDirError` (the one validated `$BEADS_DIR`, S49), `CacheLocation`, `beads_dir`, `load_repo_config` and `ConfigLoadError` |
| `schema` | the v3 DDL as the baseline Python implementation runs it, the extension tables, columns and indexes, the membership take-over (ADR-STORE-0001, 0002) |
| `store` | `SqliteStore`: open (settings, version, identity; `open`, `open_with` with an explicit `BeadsDirOverride`, S36/S49, and `open_at`), every `CacheStore` method |
| `codec` | encoding and validated decoding of stored rows, edges, journal entries, ready bases and timestamps |
| `lock` | `FlockDrainLock` and its guard |
| `registry` | the process-wide claim that keeps one connection per cache file (ADR-STORE-0003) |
| `error` | the one mapping of the store's own failures to `CacheError` (ADR-STORE-0003) |

## ADR-STORE-0001: The file schema: v3 unchanged plus additive extensions

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by ADR-STORE-0004 (a-7: `ready_base_blocked`, `ready_base.indexed`, `journal.applied_epoch`, `journal_applied_epoch`, `journal_applied_unknown`; the index `journal_applied_at` of an earlier a-7 build is dropped; amended by sprint b-1 (contract §9, lead ruling S62: `journal.last_error_code`, `journal.note`, `journal.changed`; see "Amendment b-1" below); amended by ADR-STORE-0006 (the phase-B extensions; the in-repo evidence of the b-1 amendment becomes the format golden)

Context: the file is shared with the baseline Python implementation (REQ-BDC-0804) at
`user_version` 3 (REQ-BDC-0103). Phase a needs scopes with membership
(ADR-BDC-0001), a per-scope ready base (ADR-BDC-0002), `applied_at` for the
ready adjustment, and room for an HTTP backend (ADR-BDC-0004). P-2 (agreement
with the first consumer) is open; its fallback is additive tables and
nullable columns only, with no version change.

Decision: the v3 tables and indexes are created with the baseline Python implementation's DDL
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
ALTER TABLE journal ADD COLUMN last_error_code TEXT;     -- b-1: the CODE of last_error
ALTER TABLE journal ADD COLUMN note TEXT;                -- b-1: the resolve note
ALTER TABLE journal ADD COLUMN changed INTEGER;          -- b-1 (S62): 1 / 0, stored with the entry
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
the baseline Python implementation keeps bd's full dependency objects in `row_json`), and the rest
of `row_json` is decoded with the row model's serde shape (unknown keys
refused, ids validated), the exact inverse of the encoding. The ready base's
lists are JSON arrays of `BlockedRow` and bead ids. The outage and sync
alerts are the v3 `meta` keys (`outage_since`, `outage_error`,
`sync_alert:<phase>`); clearing the outage also clears
`outage_announced_at`, as the baseline Python implementation does. `user_version` stays 3: 0 is
initialized, 3 opened, any other value refused and left unchanged.

Consequences: the baseline Python implementation opens the extended file (its `CREATE ... IF NOT
EXISTS` and column-named inserts ignore the additions) and its `status`
shows the extra `sync_state` columns. A later version change is an explicit
migration command, never done on open.

### Amendment b-1: `last_error_code`, `note`, `changed`

Context: the contract's front ends (contract §5.3, §6, §9) answer a write
with `changed` and an entry with `last_error` as an error object and the
resolve note apart, and a duplicate must answer the `changed` of the
original write (REQ-BDC-0809, lead ruling S62).

Decision: three nullable journal columns, created on open when absent;
`user_version` stays 3 and no v3 column changes meaning for the baseline Python implementation.

| Column | Written by this store | When NULL (or stale) on read |
|---|---|---|
| `changed` | `insert_entry`, in the same `INSERT` as the entry (one transaction): 1 / 0 | `None` (an entry the baseline Python implementation journaled) |
| `last_error_code` | with every `last_error` it writes (`record_attempt`, `escalate`, `apply_with_readback`): the text's `CODE: ` prefix, NULL when it has none | the code of `last_error`'s `CODE: ` prefix. A stored code is used only while `last_error` still starts with `<code>: `, since the baseline Python implementation rewrites `last_error` without it |
| `note` | `resolve`: `resolved by <actor>: <note>`; `last_error` is left as it was (it no longer gets the note appended) | on a resolved entry, a trailing `" \| resolved by <actor>: <note>"` of `last_error` (what the baseline Python implementation's resolve appends) is split out into `note`, and the rest is `last_error` (`None` when empty) |

The split is on read only: the stored text the baseline Python implementation wrote is never
rewritten. The 0.1 Python `entry` dict renders `last_error` as before, the
error then `" | "` and the note (`JournalEntry::legacy_last_error`), so its
text is unchanged; the baseline Python implementation reading an entry this store resolved sees
`last_error` without the note.

Consequences: the baseline Python implementation's column-named `INSERT` leaves `changed` NULL
and its `_mark` leaves `last_error_code` stale, which the read rules cover;
`tests/compat.rs` (`python_resolves_a_rust_written_entry_and_the_rust_store_splits_it_back`)
runs both directions.

*Amended by ADR-STORE-0006:* `tests/compat.rs` is removed with the vendored implementation
(sprint b-9); the read rules above stay, pinned by the store's own tests and the format golden,
and the cross-implementation run is the consumer's test against the published wheel.

## ADR-STORE-0002: Sharing the file with the baseline Python implementation

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); revised by fix-a-4-2 (lead ruling, bc-lead, 2026-10-24); Concurrency bullet amended by ADR-STORE-0004 (lead rulings S25/S26, bc-lead, 2026-10-24) and by the bounded marker writes (lead ruling S33, bc-lead, 2026-10-24); amended by ADR-STORE-0006 (cross-implementation evidence moves to the consumer's own test against the published wheel; the in-repo evidence is the format golden)

Context: the baseline Python implementation writes the same file and knows nothing of the
extensions. Its `INSERT OR REPLACE INTO beads(...)` resets the extension
columns of the rows it stores; its `prune` deletes beads by the `phase`
column; its rebuild deletes `beads`, `edges` and `sync_state`. Either may run
while the other has the file open.

Decision:
- **Membership.** `beads.membership_known` is 1 on every row this store
  writes. A row with `membership_known IS NULL` was last written by
  the baseline Python implementation; its membership is its v3 `phase` column, which is how
  the baseline Python implementation itself lists a phase. Reads apply that rule directly
  (`rows_in_scope`, `status`), so a row the baseline Python implementation writes while this store
  is open reads correctly at once. Open and `prune_scope` take such rows
  over (membership from `phase`, `membership_known = 1`) and drop
  `scope_member` rows whose bead no longer exists; an upsert takes over the
  row it stores. A bead deleted by the baseline Python implementation and stored again unscoped is
  a new row with no membership, so stale memberships never revive.
- **Ready bases** that the baseline Python implementation's rebuild or prune leaves behind are
  kept; the next sync of the scope replaces them (a-6), and their rows are
  read as absent until then.
- **Drain lock.** The lock file is named as v3 names it (the cache file's
  extension replaced by `drain.lock`), so the two drains exclude each other.
- **`synced_at`.** The v3 column is `NOT NULL` but neither implementation
  reads it; `upsert_rows` takes no time, so the store writes SQLite's
  current UTC time, as the baseline Python implementation writes its own clock. No value the port
  returns depends on it; every other timestamp comes from the caller.
- **Concurrency.** Every mutating method is one `BEGIN IMMEDIATE`
  transaction under the 5 s busy timeout, as the baseline Python implementation's writes are,
  single-statement ones included (`SqliteStore::write`). **Exception**
  (ADR-STORE-0004, lead rulings S25/S26): the journal reads `has_entries_since`
  and `applied_since` may run one autocommit `UPDATE` that dates entries
  the baseline Python implementation applied while the store is open, only when more than
  `READ_DATING_THRESHOLD` (2000) such entries wait, and with the busy timeout
  set to zero, so they never wait for the write lock; the answer is the same
  whether or not the `UPDATE` runs. **Bounded marker writes** (lead ruling S33,
  below). Every read that
  needs more than one value sees one snapshot: `status`, `get_row` and
  `rows_in_scope` run in one read transaction (`SqliteStore::read`);
  `outage` (both meta keys) and `open_work` (both counts) are one statement
  each. Open checks `user_version` and `project_id` (and creates the
  schema) before it sets WAL, so a refused file keeps its journal mode;
  the baseline Python implementation's own order (WAL first) does not constrain ours.
- **Bounded marker writes (S33).**
  **Status:** Approved (lead ruling S33, bc-lead, 2026-10-24).
  `mark_announced_within` and `set_sync_alert_within` (port methods whose
  provided bodies ignore the wait, for a store that never waits) run their one
  `BEGIN IMMEDIATE` transaction with the busy timeout `min(BUSY_TIMEOUT, wait)`
  and put `BUSY_TIMEOUT` back after it (`SqliteStore::write_within`; `BusyWait`,
  the drop guard the read-path dating uses at zero, also restores on unwind).
  The tick passes the time it has left, so a write lock another process holds
  cannot carry it past its end; a lock held past the wait is the usual
  `STORE_FAILED` (`database is locked`) and nothing is written. The store keeps
  the SQLite details: the caller passes a `Duration`, never a pragma.
  `mark_announced` and `set_sync_alert` are these with `BUSY_TIMEOUT`. Tests:
  `a_bounded_marker_write_waits_at_most_its_wait_and_restores_the_timeout`
  (unit; the timeout sampled inside each write by a temporary trigger: the wait,
  zero, and `BUSY_TIMEOUT` for a longer wait) and
  `a_bounded_marker_write_gives_up_on_a_held_lock_within_its_wait`
  (`tests/store.rs`; a lock held by another connection until both writes have
  returned).
- **Database names.** `DoltDatabase` is stricter than the baseline Python implementation (which
  refuses only `/`): the name becomes a file name on every supported
  platform, so `\`, `: * ? " < > |`, control characters, a trailing dot or
  space and the Windows device names (`CON`, `PRN`, `AUX`, `NUL`,
  `COM1`-`COM9`, `LPT1`-`LPT9`, `CONIN$`, `CONOUT$`, any case, any extension) are
  `NO_DATABASE`. A database the baseline Python implementation accepts with such a name cannot be
  shared until it is renamed.
- **`applied_since`** is bounded in SQL by an instant, never text (ADR-STORE-0004):
  `applied_epoch >= ?`, or, for an applied entry with no epoch, its time
  `COALESCE(applied_at, updated_at)` read by `unixepoch()` in the query; the two are
  separate index range reads joined by `UNION ALL` (`APPLIED_SINCE_SQL`, ADR-STORE-0004).
  **No expression index.** No index holds a
  function, `unixepoch()` included: an index on `unixepoch(..)` would make the
  file's schema unreadable, and every journal write fail, for a baseline Python
  implementation linked against SQLite older than 3.38 (Ubuntu 22.04 ships 3.37.2). The epoch
  is a plain column (ADR-STORE-0004) so that plain partial indexes serve the query.

Consequences: rows and memberships stay consistent across a prune,
rebuild or sync of the same file by the baseline Python implementation (tests `compat.rs`, `store.rs`).
Baseline Python implementation journal args that a-1's `Op::parse` refuses (for example
`{"add_labels": []}`, ADR-MODEL-0001) read as `BAD_ARGS` naming the entry.

## ADR-STORE-0003: One connection per process, and the store's own failures

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); failure mapping amended for a-1 amendment 1 (`STORE_FAILED`); revised by fix-a-4-2 (lead ruling, bc-lead, 2026-10-24); amended by a-7 (lead ruling S34, bc-lead, 2026-10-24: `ConfigLoadError::code` and `ConfigLoadError::fix`, see the amendment below; its codes and exhaustiveness revised by lead rulings S37 and S38, fix-a-7-12; `BeadsDirOverride` and `BeadsDirError` added by lead rulings S48 and S49, fix-a-7-15, see the second amendment below); amended by ADR-BDC-0014 (the one connection is shared with `CacheScheduleStore`)

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
  and never skipped; and a journal mode SQLite will not set to WAL. A
  refused one-connection claim (NFR-BDC-0007) is not a store failure: it is
  `BD_CACHE.STORE_IN_USE` (PE-RBP-F008, ADR-MODEL-0002 amendment), whose fix
  says to close or reuse the other handle, since the file is fine. A type-conversion error
  rusqlite raises while reading a column keeps that rusqlite error as its
  source (`failed`); a value the store's own validation refuses after
  reading has no source (`corrupt`). `NO_DATABASE` means only missing or
  invalid `metadata.json` (REQ-BDC-0102); an I/O or JSON error reading it is
  kept as its source and its text stays in the detail, and a permission-denied
  read says so in the detail and the fix instead of the `bd init` advice
  (PE-RBP-F014).
  Journal text that no longer parses keeps `Op::parse`'s own code
  (`BAD_OP` / `BAD_ARGS` / `BAD_PRECONDITION`) with the entry's seq. One
  module makes the mapping: `error::failed` (with a source) and
  `error::corrupt` (without).
- `ConfigLoadError` (`load_repo_config`) carries its cause one way only: as
  `Error::source`. Its `Display` names the file and the fix, without the
  cause, so a reporter walking the source chain prints the cause once. Its
  code and fix are the amendment below.

Consequences: a caller tells a broken cache file (`STORE_FAILED`) from a
workspace without bd metadata (`NO_DATABASE`), and walks the source chain
with `Error::source`.

### ADR-STORE-0003 amendment (a-7, S34)

**Status:** Approved (lead ruling S34, bc-lead, 2026-10-24); codes moved out of the
`BD_CACHE.` namespace by lead ruling S37 (ARCH-801) and `#[non_exhaustive]` removed by
lead ruling S38 (RBP-F044, QA8-003, ARCH-808), bc-lead, 2026-10-24, fix-a-7-12

Context: `bd-cache-sync`'s `ComposeError` kept its own copy of the code and fix of
each way `.beads/bd-cache.toml` fails to load (RBP-F040). Moving them next to the type
(fix-a-7-6) made them store API, but recorded them by rewriting the Decision above in
place (ARCH-701), and the store then emitted the sync crate's `sync_config_*` codes,
a consumer's namespace (ARCH-702, RBP-F043). S34 first gave them `BD_CACHE.CONFIG_*`
codes; but every `BD_CACHE.*` code is a `CacheError` code (REQ-MODEL-0003, REQ-BDC-0803),
and `ConfigLoadError` is not a `CacheError` (ARCH-801, S37). A consumer's wildcard arm
over the `#[non_exhaustive]` enum mapped any later variant to an existing code with no
compile-time check (RBP-F044, QA8-003, ARCH-808, S38).

Decision:
- `ConfigLoadError::code` and `ConfigLoadError::fix` are public store API: one stable
  code and one fix text per variant, and `Display` states that same fix after `fix:`.
- The codes are store-owned and lowercase (S37): `store_config_unreadable` (`Read`:
  the file exists but cannot be read), `store_config_invalid` (`Parse`: bad TOML, an
  unknown key or a wrong type) and `store_config_bad_scope` (`Scope`: a `scopes` entry
  is not a phase name; its fix states `PhaseName::RULE`). They are **not `CacheError`
  codes** (the `BD_CACHE.*` namespace belongs to `CacheError` alone) and **not
  CLI-contract codes**: no CLI reply carries them; a consumer that reports a
  configuration failure reports its own code.
- `ConfigLoadError` is exhaustive, not `#[non_exhaustive]` (S38): each consumer matches
  every variant, with no wildcard arm.
- A consumer with codes of its own maps the variants at its own boundary; the store
  names no consumer's codes. `bd-cache-sync` keeps its `sync_config_*` codes by mapping
  `ConfigLoadError` variants in `ComposeError` and uses `fix()` as the recovery text.
  `bd-cache-py` maps the variants to its own exception kinds and does not use `code()`.

Consequences: changing a code or a fix text changes store API, and the tests that pin
them (`tests/location.rs`: `repo_config_is_parsed_and_unknown_keys_refused`,
`unreadable_repo_config_is_an_error_not_the_default`; the `ConfigLoadError::code`
doctest). A new variant needs a code and a fix here, and a mapping in each consumer;
the mechanism that enforces the mapping is the compiler (S38): because the enum is
exhaustive and the consumers' matches have no wildcard arm (`bd-cache-sync`'s
`ComposeError` code mapping, `bd-cache-py`'s `From<ConfigLoadError> for OpenError`), a
new variant is a compile error (`E0004`, non-exhaustive patterns) in each consumer
until it is mapped. Adding a variant is therefore a breaking change to store API.

### ADR-STORE-0003 amendment (a-7, S48/S49): one validated `$BEADS_DIR`

**Status:** Approved (lead rulings S48 and S49, bc-lead, 2026-10-24, fix-a-7-15;
QA10-001, RSH-038, RBP-F047, RBP-F048; per-platform wording by lead ruling S50, RSH-039)

Context: S44 normalized `$BEADS_DIR` (empty is unset, relative is made absolute) inside
`bd-cache-sync`'s `compose_with` only, and the store still took a raw `Option<&OsStr>`,
so another pairing of the store and a `bd` child could name two directories again
(RBP-F047). A value whose working directory could not be read was kept relative,
which reintroduced that split and dropped the I/O error (QA10-001, RSH-038, RBP-F048).

Decision:
- The rule lives in one validated type, `BeadsDirOverride` (a newtype over an absolute
  `PathBuf`). `BeadsDirOverride::new(Option<&OsStr>)` is the one fallible constructor:
  unset or empty is `Ok(None)`; any other value is made absolute with
  `std::path::absolute` against the process's working directory. On Unix that joins a
  relative path with the working directory and resolves nothing (no `..`, no link;
  only `.` components and repeated separators drop out, as `Path::components` does);
  on Windows it calls `GetFullPathNameW`, which also resolves `.` and `..` textually,
  without touching the file system (S50). `BeadsDirOverride::from_env` is `new` over
  the process's `$BEADS_DIR`. `as_path` and `as_os_str` give the value to the store and
  to a child.
- It fails closed (S48): a value that cannot be made absolute is a `BeadsDirError`,
  never a relative fallback. `BeadsDirError` names the value in `Display` and carries
  the `io::Error` only as `Error::source`; `value()` and `into_parts()` hand both to a
  consumer that maps it to an error of its own, and `BeadsDirError::new` lets a
  consumer build one to test that mapping.
- `SqliteStore::open_with`, `CacheLocation::resolve_with` and `beads_dir` take
  `Option<&BeadsDirOverride>` instead of `Option<&OsStr>` (a signature change of store
  API; `bd-cache-store` is not a btit API).
- The environment-reading paths, `SqliteStore::open` and `CacheLocation::resolve`, build
  the override with `from_env`. An unresolvable environment value is
  `BD_CACHE.NO_DATABASE`, its detail naming the value and the I/O error, before any file
  is read: the beads directory cannot be located, which is what `NO_DATABASE` reports.
  Its detail ends with the real remedy, "set an absolute BEADS_DIR or run from a
  readable working directory" (PE-RSH-043), since the code's `bd init` fix does not
  address it; the I/O error stays text only, so a permission-denied working directory
  does not select the fix for an unreadable `metadata.json`. `bd-cache-py`, which opens through `SqliteStore::open`, raises it as it raises
  any `NO_DATABASE`.
- `bd-cache-sync`'s `compose_with` builds the override once and passes the same value to
  `SqliteStore::open_with` and its `as_os_str()` to `BdCli::with_child_var`; it maps a
  `BeadsDirError` to its own `ComposeError::BeadsDir` (`sync_beads_dir_unresolvable`).

Consequences: the store and every `bd` child of a composition cannot be given two
different directories, since both take the one value the type holds. A consumer that
has a raw value calls `BeadsDirOverride::new` and handles its error. Evidence
(`src/location.rs` unit tests): `an_unset_or_empty_beads_dir_is_no_override`,
`a_relative_beads_dir_is_made_absolute_against_the_working_directory`,
`an_absolute_beads_dir_is_kept` and
`a_beads_dir_that_cannot_be_made_absolute_is_an_error_with_its_cause` (the
`absolute` step injected to fail; also the `NO_DATABASE` mapping of the
environment-reading paths); the `BeadsDirOverride` doctest; `tests/location.rs`
(`test_beads_dir_env_names_the_database` through the type).

## ADR-STORE-0004: A per-bead index of the ready base, and an index for the journal check (a-7 amendment)

**Status:** Approved (lead ruling, bc-lead, 2026-10-24, fix-a-7-2; journal indexes revised by lead ruling S18; read-path dating approved by lead rulings S25/S26, see below; NFR-BDC-0001's condition stated by lead ruling S31, see Remaining latency; floor invariant restated per rise, restatement only, no decision change (QA7-001, fix-a-7-10)); amends ADR-STORE-0001 (the extension tables, columns and indexes)
and ADR-STORE-0002 (what is read as one snapshot, what a foreign writer may leave, and the
Concurrency bullet: the journal reads may write, as the exception noted there)

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
- **No function in any index.** The baseline Python implementation may be linked against an SQLite older
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
  `unixepoch()`; no index mentions it. The baseline Python implementation writes `%Y-%m-%dT%H:%M:%SZ`,
  which SQLite reads. Open always runs it, in the open transaction.
- **Read-path dating (S25, S26).**
  **Status:** Approved (lead ruling S25/S26, bc-lead, 2026-10-24).
  S25 had `has_entries_since` and `applied_since` date on every read that found an
  entry to date; S26 replaces that with a threshold, so a small burst of entries
  the baseline Python implementation applies while the store is open never puts a WAL commit on the `get`
  path. Before answering, each of the two reads counts the undated entries above the
  floor of known unreadable ones (`UNDATED_ABOVE_SQL`, a `journal_applied_unknown`
  range read that evaluates no function, capped at the threshold + 1; one index probe
  in a healthy file). At or below `READ_DATING_THRESHOLD` (2000) nothing is written and
  the read walks the undated entries. Above it the read first runs the backfill as one
  autocommit `UPDATE` with the busy timeout set to zero (`BusyWait` at zero, a drop guard
  that restores `BUSY_TIMEOUT`, also on unwind), so it never waits for the write lock; a
  writer holding the lock, or a read-only file, leaves the entries undated for now, the
  read answers the same from them, and the next read tries again. The answer never
  depends on whether the `UPDATE` ran.
  *Measurements* (release build, Apple silicon, 1000-bead synced scope, cached `get` of
  a bead with a stored base, median of 500 gets; probe in the a-7 fix-5 QA scratchpad):
  idle `get` 21-23 us; with N undated entries walked, not dated: 500 → 62 us, 1000 → 101 us,
  1500 → 140 us, 2000 → 175-183 us (about 0.08 us per entry). The dating commit itself
  costs a first `get` 2.2-3.1 ms at 2001 entries, 3.2-5.4 ms at 3000, 5.2-11 ms at 5000,
  20-31 ms at 20 000 (QA's earlier run of the S25 build: 2.9 ms at 1000, 9.5-33 ms at 5000,
  21-25 ms at 20 000), after which `get` is back to 22 us. 2000 keeps the walked `get` under
  0.2 ms, a fifth of NFR-BDC-0001's 1 ms with room for a machine several times slower,
  while the commit it avoids costs 2 ms or more, ten times the walk.
  *Remaining latency against NFR-BDC-0001* (lead ruling S31: NFR-BDC-0001 now states
  the condition, at most `READ_DATING_THRESHOLD` undated entries and no foreign writer
  holding the write lock, and names these excursions): above the threshold, the one `get` that dates
  is a one-off excursion past 1 ms (2-31 ms as measured, growing with the burst); every
  later `get` is back under it. While another writer holds the write lock over more than
  2000 undated entries, each `get` walks them (1.3 ms median at 20 000) until the lock
  is free. A burst at or below the threshold costs at most the walk, about 0.2 ms, and
  writes nothing.
- **Unreadable entries on the read path (QA5-002).** The store keeps a floor of known
  unreadable entries. The count above starts at the floor, so those entries are not
  walked by the count on every call. The floor rises after a dating or a new warning,
  through the unreadable entries above it and never past an applied entry the dating
  can date (`FIRST_DATEABLE_ABOVE_SQL`). **The invariant holds per rise (QA7-001):** a
  rise from an old floor to a new one passes only the entries between the two (above
  the old, at or below the new), and every one of those that was applied and undated
  when the floor rose had an unreadable time, so no dateable entry above the old floor
  is hidden from the count. Entries already below the old floor are not examined again
  by a rise: the floor says nothing about them, and one of them may be readable (the
  case below), also after later rises. The store also remembers the highest seq it has examined:
  each read looks at the undated entries above it only (`LAST_UNDATED_SQL`,
  `UNDATABLE_BETWEEN_SQL`), so each entry's time is read there once, and warns once
  for each new highest undatable seq (the open warns about all of them and starts both
  marks there).
  **Entries applied below the floor (QA6-001).** An entry still pending when the floor
  rose above its seq, which the baseline Python implementation applies later without an epoch, sits below
  the floor: it is not in the threshold count and is not examined for a warning, and a
  later rise (one driven by a warning alone included) does not examine it either. It is
  answered correctly, as every undated entry is (the journal reads do not use the
  floor, and read its own time), and it is dated by the next dating, past the
  threshold or at the next open, because the backfill dates every readable entry
  whatever its seq. Until then reads walk that one
  entry. Capping the floor below the lowest pending seq was rejected: while an old
  entry stayed pending (during an outage, for hours), every unreadable entry above it
  would be back in the count, and past the threshold every read would try the dating
  again, the per-read walk and write this floor removed. Pinned by
  `a_pending_entry_applied_below_the_floor_is_answered_and_dated_by_the_next_dating`
  (unit), which also raises the floor a second time from a warning alone after the
  entry is applied below it: the entry stays undated and answered by its own time
  until the next dating dates it (QA7-001).
- **Unreadable times count as touched.** A row whose `COALESCE(applied_at, updated_at)`
  SQLite cannot read as a time keeps a NULL epoch, stays in `journal_applied_unknown`,
  and counts as applied since any instant (`COALESCE(... >= ?1, 1)`). Reporting
  activity that did not happen only costs a fresh read; missing it would serve a
  stale ready answer. `applied_since` agrees with it. The cost: while such a row exists,
  every cached `get` of a bead with a stored base takes the adjusted path (the whole base
  is recomputed). So open, after the backfill, logs a warning naming the seqs of the
  applied entries still undated (the first 20, their count and the highest), and the read
  path warns about new ones, once each (above). **Operator fix:** find the
  entry (`SELECT seq, idem_key, bead, updated_at FROM journal WHERE seq = <seq>`) and either
  resolve it (`UPDATE journal SET state = 'resolved' WHERE seq = <seq>`, which takes it out
  of the applied set) or delete it (`DELETE FROM journal WHERE seq = <seq>`); an applied
  entry is history only, so neither loses a pending write. Setting its `updated_at` to a
  real time works too; the next read dates it.
- **The pre-existing miss, fixed.** The baseline Python implementation's `_mark` sets only `state` and
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
  walk at once: they count as since), and the read-path dating keeps it at or below
  `READ_DATING_THRESHOLD` while the store is open (S26), so the check stays bounded; it is
  not bounded for entries the dating could not yet date (the write lock was busy).
- `applied_since` is two range reads joined by `UNION ALL` and ordered by seq
  (`APPLIED_SINCE_SQL`): `journal_applied_epoch` for `applied_epoch >= at`, and
  `journal_applied_unknown` for the undated entries. One `WHERE` with an `OR` of the two
  (the earlier form) lets neither partial index serve it, so it read every applied entry.
- **A missing hinted index.** An index another writer dropped makes `INDEXED BY` an error
  (`SQLITE_ERROR`, `no such index`; matched on the code and the message). SQLite has no
  more specific code for the case, so the detection relies on the message prefix
  `no such index` that SQLite's `INDEXED BY` failure carries (REQ-QA-503); the test
  `the_entries_since_check_survives_a_dropped_index` drops the indexes and so pins that
  prefix for the bundled SQLite: a release that changed it would fail that test, not
  production silently. The store then
  logs one warning, remembers it, and from then on asks the unhinted form of each journal
  read (`HAS_ENTRIES_SINCE_UNHINTED_SQL`, `APPLIED_SINCE_UNHINTED_SQL`, the backfill's and the
  read-path dating's own queries):
  the same question, correct, slower on a long history. The hints are not tried again
  until the next open, which recreates the indexes. Tests assert the plans of the hinted
  queries, their SQL work in VM steps, and that the fallback is not retried.
- **Baseline Python implementation compatibility** (read at the baseline
  commit `05831dfe866d389feed138add3c29cb93cd6c82b`): it never creates,
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
`a_small_burst_is_walked_and_not_dated_until_past_the_threshold` (S26: no commit at or below
the threshold, `PRAGMA data_version`), `an_unreadable_entry_does_not_hide_earlier_undated_ones_from_the_dating`,
`applied_since_reads_both_partial_indexes_and_no_history`,
`the_entries_since_check_survives_a_dropped_index` (store tests),
`the_open_finds_the_applied_entries_it_could_not_date`,
`undatable_entries_written_while_open_are_warned_once_and_not_walked_again` (unit, QA5-002),
`the_floor_stops_one_below_the_first_dateable_entry` (unit, QA5-002: the floor is F - 1),
`the_dating_update_runs_with_no_busy_timeout_and_restores_it` (unit, QA5-003: the busy
timeout sampled inside the `UPDATE` by a temporary trigger is 0, and is back afterwards;
it replaces the timed `a_write_after_a_skipped_dating_waits_for_the_lock_again`, removed
for FTQ-022),
`a_pending_entry_applied_below_the_floor_is_answered_and_dated_by_the_next_dating` (unit,
QA6-001) and
`a_cached_get_dates_entries_python_applied_while_open` (bd-cache-sync `perf.rs`) check the
dating, the plans and SQL work (VM steps), the undatable-entry warning and the hint fallback.

## ADR-STORE-0005: Dependencies and test-only features

**Status:** Approved (lead ruling S32, bc-lead, 2026-10-24); revised for QA6-002/QA6-003 (fix-a-7-8); tempfile for doctests (QA7-005, fix-a-7-10)

Context: the Crate Layout of [`../architecture.md`](../architecture.md) lists each
crate's dependencies; the store's test-only features were recorded nowhere (ARCH-403),
and the store took `tracing` directly, which the layout does not list (ARCH-401).

Decision:
- Runtime dependencies: `bd-cache-model`, `sc-beads`, rusqlite (workspace features:
  `bundled`, NFR-BDC-0007), fs4, `serde_json`. Nothing else.
- Logging goes through `sc_beads::logging::tracing` (ADR-BDC-0007), as `bd-cache` does;
  the store has no direct `tracing` dependency.
- Dev-dependencies, test builds only:
  - rusqlite with `functions`: a test registers its own `unixepoch` that fails, standing
    in for a baseline Python implementation on SQLite older than 3.38
    (`no_index_holds_a_function_and_journal_writes_work_without_unixepoch`); and the
    busy-timeout probe tests (`store.rs` unit tests,
    `the_dating_update_runs_with_no_busy_timeout_and_restores_it`, QA5-003, and
    `a_bounded_marker_write_waits_at_most_its_wait_and_restores_the_timeout`, S33)
    register `probe_busy_timeout`, which a temporary trigger calls from inside the
    store's write to read `PRAGMA busy_timeout` (QA6-003). That function's
    `Context::get_connection` call is the crate's only `unsafe`, in test code only.
    SAFETY: rusqlite marks it unsafe because a `ConnectionRef` could be sent to another
    thread (rusqlite issue 643); the probe uses it for one read on the thread running
    the statement and drops it before the function returns, so it neither outlives the
    call nor leaves the thread;
  - `tempfile`: the unit tests' cache files live in a `TempDir`, removed on drop after
    the store closes, also when a test fails (QA6-002); the doctests' workspaces too
    (QA7-005);
  - `bd-cache-model` with `testing`: the store contract suite (`store_contract.rs`);
  - `sc-beads` with `testing`: bead, id and actor fixtures (tests and doctests).

Consequences: no feature reaches a non-test build; adding a dependency or feature to
`crates/bd-cache-store/Cargo.toml` amends this ADR and the Crate Layout row.

## ADR-STORE-0006: The phase-B extensions for the sync rows, ingest and marking

**Status:** Approved (Rand, 2026-10-26); implemented by sprint b-7. Amends ADR-STORE-0001 (the
extension list) and ADR-STORE-0002 (where cross-implementation evidence lives).

Context: the sync on sc-cron rows, the consumer's Python sync and the post-sync hook
(ADR-BDC-0011, REQ-BDC-0720..0732) need per-scope state, scope filters, failure state, a lease
and the schedule rows in the cache file. ADR-STORE-0001 freezes the file as v3 plus additive
extensions; lead ruling S84 forbids vendoring any consumer code, so the compat tests that ran
the baseline Python implementation against this store leave the repository.

Decision: add, through the existing mechanism (`PRAGMA table_info` / `CREATE TABLE IF NOT
EXISTS` under `BEGIN IMMEDIATE`), exactly these extensions, all nullable or new tables;
`user_version` stays 3 and no v3 table, column or index changes:

| Kind | Name | Default and meaning |
|---|---|---|
| column | `scope.revision` TEXT | NULL; opaque, stored per scope by `ingest_scope` |
| column | `scope.filter` TEXT | NULL = the phase-a label scope; otherwise the scope's filter as JSON (ADR-BDC-0013) |
| column | `sync_state.failing_since` TEXT | NULL = not failing |
| column | `sync_state.notified_since` TEXT | NULL |
| column | `journal.first_offered_at` TEXT | NULL = never offered to a hook (`attempts` already exists) |
| table | `tick_lease` (`id` INTEGER PRIMARY KEY CHECK (`id` = 1), `run_id` TEXT, `pid` INTEGER, `host` TEXT, `expires_at` TEXT) | the one sync lease (REQ-BDC-0722) |
| table | `hook_state` (`id` INTEGER PRIMARY KEY CHECK (`id` = 1), `consecutive_failures` INTEGER NOT NULL DEFAULT 0, `first_failure_at` TEXT, `last_error` TEXT, `next_attempt_at` TEXT) | the built-in row's hook backoff (REQ-BDC-0727) |
| table | `sc_cron_schedule`, with sc-cron's version-2 columns and checks | the schedule rows, served by `CacheScheduleStore` (ADR-BDC-0014) |
| `meta` row | `sc_cron_schedule_version` = `2` | the version of sc-cron's columns in this file, checked by `CacheScheduleStore`; never `user_version` |

Existing names stay (`phase`, `announced_at`, `scope.kind DEFAULT 'phase'`); the API names them
neutrally. The format golden (`crates/bd-cache-store/tests/format_golden.rs` with its DDL
fixture, owned by sprint b-7) pins the exact v3 DDL plus every row above, and is the in-repo
evidence of ADR-STORE-0001 and ADR-STORE-0002 once the vendored compat tests go (sprint b-9).
Cross-implementation compatibility is the consumer's own test, run against a CI-built wheel
before the 0.2.0 release (S84).

Rejected: the r3 per-scope `scope.method`, the `journal.ingest_tick_id` and
`scope.last_ingest_tick_id` stamps and the `registration` table (superseded by the two-row
switch, Amendment A7); sc-cron's own file beside the cache file (two files to keep in step, and
the schedule would not move with the cache).

Consequences: a v3 reader that does not know an extension still reads and writes the file.
Further extensions need a later ADR.
