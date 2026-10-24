---
id: a-4
phase: a
status: in-review
closure_type: boundary
target_boundary: bd-cache-store
track: store
wave: 2
branch: sprint/a-4-store
worktree: sprint/a-4-store
recommended_agent: rust-developer
recommended_model: opus
must_follow: [a-1]
parallel_safe: [a-2, a-3, a-5, a-6]
requirements:
  owns: [REQ-BDC-0101, REQ-BDC-0102, REQ-BDC-0103, REQ-BDC-0104, REQ-BDC-0107, REQ-BDC-0108, REQ-BDC-0109, REQ-BDC-0604, REQ-BDC-0804, REQ-BDC-0901, NFR-BDC-0007, REQ-STORE-0001, REQ-STORE-0002, REQ-STORE-0003, REQ-STORE-0004, REQ-STORE-0005, REQ-STORE-0006, REQ-STORE-0007]
  constrained_by: [REQ-BDC-0105, REQ-BDC-0401, REQ-BDC-0405, REQ-BDC-0502, REQ-BDC-0503, REQ-BDC-0504, REQ-BDC-0601, REQ-BDC-0602, REQ-BDC-0603, REQ-BDC-0902, REQ-BDC-0903, REQ-BDC-0904, REQ-BDC-0907, REQ-BDC-0908, REQ-BDC-0909, REQ-BDC-0911, NFR-BDC-0001, NFR-BDC-0005]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0007, ADR-MODEL-0001, ADR-MODEL-0002, ADR-MODEL-0003, ADR-CORE-0001, ADR-STORE-0001, ADR-STORE-0002, ADR-STORE-0003]
owned_paths:
  - "crates/bd-cache-store/**"
  - "docs/bd-cache-store/**"
---

# Sprint a-4 — SQLite store

## Outcome and closure

Implement a-1's `CacheStore` and `DrainLock` ports over SQLite and flock.
Closes when `SqliteStore` passes a-1's contract suite and its own cases, and
files are shared with the Python implementation in both directions.

## must_follow rationale

Implements the `CacheStore` trait and stores the `Bead`, `Edge` and
`JournalEntry` types defined in a-1.

## Decision prerequisites

P-2. Either way `user_version` stays 3 and the extensions are additive tables
and nullable columns only (REQ-BDC-0103, 0107, 0804). If P-2 is refused, a
later version change is an explicit migration command, never done on open.
The Rust store tolerates rows the Python implementation leaves behind in
the extension tables (for example `scope_member` rows after a Python
prune or rebuild). Record the schema as ADR-STORE-0001 in
`docs/bd-cache-store/architecture.md`.

## Deliverables

1. File location from the configured `cache_path` (default
   `.bd-cache/<dolt_database>.bd-cache.sqlite3`), with `dolt_database` and
   `project_id` from `.beads/metadata.json` (`$BEADS_DIR` aware), starting
   no `bd` process (REQ-BDC-0101); `NO_DATABASE` (REQ-BDC-0102).
2. Open: WAL, 5 s busy timeout, `user_version` check and `SCHEMA_VERSION`
   (REQ-BDC-0103); project id record and `WRONG_DATABASE` (REQ-BDC-0104);
   bundled rusqlite, one connection per process (NFR-BDC-0007).
3. Schema: the baseline v3 tables and indexes (REQ-BDC-0107) plus the
   extensions: `scope` and `scope_member` (REQ-BDC-0901, ADR-BDC-0001), a per-scope ready base (blocked rows with `blocked_by`, ready ids,
   fetched_at, stale; ADR-BDC-0002), nullable
   `revision` and `sync_state` columns `backend_kind`, `endpoint`,
   `replica_id`, `journal_seq` (ADR-BDC-0004).
4. Every `CacheStore` method and every rule in a-1's `CacheStore` doc comment
   (edges written from `Bead.dependencies` and rebuilt on read, membership,
      stale clearing, rebuild scope, ready base, persisted scopes,
   `applied_since` with an additive `applied_at` journal column), including
   `ensure_scope`, membership from
   labels on scoped upserts, the stale-clearing rule, the outage record,
   out-of-scope rows stored with no membership and swept by `prune_scope`, the newer-row guard and forced
   upsert, row normalization (REQ-BDC-0108), the edges index by target
   (REQ-BDC-0109), `apply_with_readback` in one transaction,
      `record_fingerprint` with the rebuild rule as a-1 states it (deletes
   beads, edges, memberships, sync_state rows and ready bases in one
   transaction, keeps the journal), announce marks
   (REQ-BDC-0604), sync alerts, and `prune_scope` keeping beads another scope
   holds.
5. `DrainLock` with `flock` on `<cache file>.drain.lock`
   (cross-process; REQ-BDC-0401 constraint).
6. Compatibility both ways (REQ-BDC-0804): the reference `bd_cache.py`,
   `bd_models.py` and a v3 fixture, vendored at atm-core `05831df` under
   `crates/bd-cache-store/tests/compat/`, run with `uv` (installed by a-1's CI) to read a file the Rust store wrote;
   and the Rust store reads a Python-written one.
7. `load_repo_config(repo_root)`: the one loader of `.beads/bd-cache.toml`
   into a-1's `RepoConfig` (missing file: defaults; unknown key: error).
8. Per-crate docs `docs/bd-cache-store/requirements.md` and `architecture.md`.

## Acceptance criteria

| Criterion | Root |
|---|---|
| a-1's store contract suite passes against `SqliteStore` | ADR-BDC-0004 |
| `test_cache_file_is_named_for_the_dolt_database`, `test_beads_dir_env_names_the_database`, `test_missing_metadata_is_typed` (REQ-BDC-0101, 0102) | boundary:bd-cache-store |
| `test_newer_cache_schema_is_refused`, `test_file_from_another_project_is_refused` (REQ-BDC-0103, 0104) | boundary:bd-cache-store |
| `test_dependents_use_the_edge_index` (REQ-BDC-0109) | boundary:bd-cache-store |
| `test_escalations_are_unannounced_until_marked`, `test_sync_alert_round_trip`, `test_pending_work_counts_as_open` (REQ-BDC-0604) | boundary:bd-cache-store |
| A v3 file written by the Python implementation opens, and its rows, edges and journal read back equal; the vendored Python `get` and `status` read a file written by the Rust store, extension tables present (REQ-BDC-0804) | boundary:bd-cache-store |
| Two processes contend for the drain lock; only one acquires it; dropping the guard releases it | boundary:bd-cache-store |
| After the vendored Python `prune` and a forced rebuild on a Rust-extended file, the Rust store opens it and reads consistent rows and memberships (REQ-BDC-0804) | boundary:bd-cache-store |
| Pruning a scope deletes only beads no other scope holds (REQ-BDC-0901) | ADR-BDC-0001 |
| Planted bugs: no newer-row guard, skip project-id check, rebuild deletes journal, no `mark_announced`, no set/clear sync alert each fail a test | boundary:bd-cache-store |

## This sprint does not close

Calling the store from drain, sync or refresh (a-5, a-6), or composing it
with the real backend (a-7).

## Required validation

Crate tests and lints; `cargo build --workspace`.

## Public contract

Normative for the store's users (a-7, a-8). It implements a-1's
`CacheStore` and `DrainLock` unchanged; this block matches
`crates/bd-cache-store/src/lib.rs` and its modules.

```rust
// ================= bd-cache-store =================
pub const SCHEMA_VERSION: i32 = 3;                          // never changed on open (REQ-BDC-0103)
pub const BUSY_TIMEOUT: Duration;                           // 5 s
pub const BEADS_DIR_ENV: &str = "BEADS_DIR";
pub const DEFAULT_CACHE_DIR: &str = ".bd-cache";
pub const REPO_CONFIG_FILE: &str = ".beads/bd-cache.toml";

pub struct DoltDatabase(/* private */);  // new(impl Into<String>) -> Result<Self, CacheError> (NO_DATABASE): a portable file name: nonempty, not . or ..,
                                         // no / \ : * ? " < > | or control char, no trailing dot or space, not a Windows device name (incl. CONIN$, CONOUT$); as_str
pub struct ProjectId(/* private */);     // new(impl Into<String>) -> Result<Self, CacheError> (NO_DATABASE): not blank; as_str
pub struct DatabaseIdentity { pub dolt_database: DoltDatabase, pub project_id: ProjectId }
impl DatabaseIdentity { pub fn read(beads_dir: &Path) -> Result<Self, CacheError>; } // <beads_dir>/metadata.json; NO_DATABASE
pub fn beads_dir(repo_root: &Path, beads_dir_env: Option<&OsStr>) -> PathBuf;          // env when set and nonempty, else <root>/.beads
pub struct CacheLocation { pub identity: DatabaseIdentity, pub cache_path: PathBuf }
impl CacheLocation {
    pub fn resolve(repo_root: &Path, config: &RepoConfig) -> Result<Self, CacheError>;  // reads $BEADS_DIR; starts no bd
    pub fn resolve_with(repo_root: &Path, config: &RepoConfig, beads_dir_env: Option<&OsStr>) -> Result<Self, CacheError>;
}
pub enum ConfigLoadError { Read { path: PathBuf, source: io::Error }, Parse { path: PathBuf, source: ConfigError },
    Scope { path: PathBuf, value: String } }            // #[non_exhaustive]; Error::source kept
pub fn load_repo_config(repo_root: &Path) -> Result<RepoConfig, ConfigLoadError>;       // missing file: RepoConfig::default()

pub struct SqliteStore { /* private: one connection */ } // Send, !Sync; impl CacheStore
impl SqliteStore {
    pub fn open(repo_root: &Path, config: &RepoConfig) -> Result<Self, CacheError>;
    pub fn open_at(path: &Path, identity: &DatabaseIdentity) -> Result<Self, CacheError>; // SCHEMA_VERSION, WRONG_DATABASE, STORE_FAILED
    pub fn path(&self) -> &Path;
    pub fn drain_lock(&self) -> FlockDrainLock;
}
pub struct FlockDrainLock { /* private */ }               // Clone; impl DrainLock (Send + Sync)
impl FlockDrainLock {
    pub fn for_cache_file(cache_file: &Path) -> Self;     // <cache>.with_extension("drain.lock"), as v3 names it
    pub fn path(&self) -> &Path;
}
// re-exported: bd_cache_model::{CacheStore, DrainLock}
```

## Requirement scope notes

`owns` lists the crate requirements REQ-STORE-0001..0007
(`docs/bd-cache-store/requirements.md`, Approved, lead ruling bc-lead 2026-10-24) next to the repo-level ids
they refine. `constrained_by` lists ids whose behaviour another sprint owns
and whose store side this sprint delivers through the a-1 port rules:

| Requirement | Store side delivered here | Behaviour owner |
|---|---|---|
| REQ-BDC-0105 (rebuild rule) | `record_fingerprint` (`fingerprint_rebuild_rule`) | a-1 helper, a-5/a-6 callers |
| REQ-BDC-0401 (single drain writer) | `FlockDrainLock` | a-5 (`drain` takes it) |
| REQ-BDC-0405 (read-back) | `apply_with_readback` in one transaction | a-5 |
| REQ-BDC-0502 (newer-row guard) | instant comparison in `upsert_rows` | a-6 |
| REQ-BDC-0503 (stale rows re-read) | `stale_rows`; any store clears `stale` (`a_store_clears_stale`) | a-6 (sync) |
| REQ-BDC-0504 (sync state) | `sync_state` / `record_sync` | a-6 |
| REQ-BDC-0601 (resolve) | `resolve` note and `NOT_ESCALATED` | a-6 |
| REQ-BDC-0602 (prune) | `prune_scope` | a-6 |
| REQ-BDC-0603 (status) | `status` from one read snapshot, with `stale` (`status_lists_stale_rows_until_reread`) | a-6 |
| REQ-BDC-0902 (filtered cache) | out-of-scope rows and the sweep | a-6 |
| REQ-BDC-0903 (slow or unreachable backend) | no store method calls the backend; the outage record (`outage` / `set_outage`) | a-1 helpers, a-5, a-6 |
| REQ-BDC-0904 (blocked state across the cache edge) | the per-scope ready base (`blocked_by`, ready ids, `fetched_at`, stale mark) and edges by target with their kind | a-6 |
| REQ-BDC-0907 (fresh read) | forced `upsert_rows(None, ..)`: stored whatever its age, membership unchanged | a-6 (`refresh`) |
| REQ-BDC-0908 (journal entry by seq) | `entry_by_seq` in any state | a-6 (`entry`) |
| REQ-BDC-0909 (on-demand scopes) | persisted `ensure_scope` | a-6 |
| REQ-BDC-0911 (ready reflects our writes) | `pending_entries` and `applied_since` on `journal.applied_at`, no backend call | a-6 (`ready`) |
| NFR-BDC-0001 (in-process get under 1 ms) | `get_row` is one primary-key lookup plus the bead's edges by the `edges` primary-key prefix, on the process's open connection | a-7 owns the measurement |
| NFR-BDC-0005 | planted-bug evidence below | all sprints |

`adrs` lists ADR-BDC-0007, ADR-MODEL-0001/0003 and ADR-CORE-0001
because the store relies on the strict rows, the Python-identical journal
text, the store rule clarifications and the threading model. It lists
ADR-MODEL-0002 (the error mapping) because its a-1 amendment 1 adds
`STORE_FAILED` with a kept source, which the store's failures use
(ADR-STORE-0003); ADR-STORE-0001..0003
are this crate's (Approved, lead ruling bc-lead 2026-10-24).

## Hand-off notes for the lead

Rulings needed:

1. **Crate ADRs and REQs.** ADR-STORE-0001 (schema), ADR-STORE-0002
   (sharing with `bd_cache.py`), ADR-STORE-0003 (one connection per process,
   store failures) and REQ-STORE-0001..0007 were Draft. Ruled: Approved
   (lead ruling, bc-lead, 2026-10-24).
2. **Store failures.** Ruled (R2) and done with a-1 amendment 1
   (integrate/phase-a `f3a4a40`, merged here): SQLite and file errors are
   `BD_CACHE.STORE_FAILED` with the underlying error kept as the source;
   stored values that do not decode are `STORE_FAILED` without one
   (ADR-STORE-0003). `NO_DATABASE` means only missing or invalid metadata.
   `status()` fills `StoreStatus.stale` from the same read snapshot as the
   rest of the status.
3. **serde_json dependency.** The store uses the workspace's `serde_json`
   (encoding `row_json` and the ready base, reading `metadata.json`). Ruled
   (R3): the Crate Layout row of `docs/architecture.md` for `bd-cache-store`
   lists it (integrate/phase-a `6ecccf3`, merged here). No workspace
   dependency was added.

Interpretations (no contract change):

- **Drain lock file.** "`<cache file>.drain.lock`" is implemented as v3
  names it: the cache file's extension replaced, `testdb.bd-cache.drain.lock`
  (BD-CACHE.md; `bd_cache.py` uses `path.with_suffix(".drain.lock")`), so the
  two drains exclude each other (`the_python_and_rust_drain_locks_exclude_each_other`).
- **"A v3 fixture".** No v3 SQLite file exists at atm-core `05831df`; the
  vendored v3 fixtures are the bd-1.3.1 `bd-schema.json` and
  `list-rows.json` the reference syncs, and the v3 file is written by the
  vendored `bd_cache.py` at test time (`tests/compat/PROVENANCE.md`).
- **`synced_at`.** `upsert_rows` takes no time, and the v3 column is
  `NOT NULL` but never read; it is SQLite's current UTC time
  (ADR-STORE-0002). No port value depends on it.
- **`load_repo_config`** reads `<repo_root>/.beads/bd-cache.toml` (not
  `$BEADS_DIR`), and refuses a `scopes` entry that is not a phase name,
  which `RepoConfig::cache_config` would otherwise skip.
- **Windows.** `bd_cache.py` imports `fcntl`; the compat driver stubs it on
  Windows (its drain lock is never taken there) and the cross-implementation
  lock test is Unix-only.

## Evidence: planted bugs (NFR-BDC-0005)

Rerun after R2 (a-1 amendment 1) on commit `28a1c863304965be2b9dc354b513d866ca0691c4`, the last commit
that changes code (later commits change this document only). Each mutation
was applied to the working tree, the crate's whole suite was run, and the
file was restored:

```text
cargo test -p bd-cache-store --all-features --no-fail-fast
git checkout -- <file>
```

| Planted bug | File | Exit | Compile error | Failing tests |
|---|---|---|---|---|
| no newer-row guard | `src/store.rs` | 101 | no | `newer_row_guard_compares_instants`, `newer_rows_are_kept_unless_forced` |
| skip project-id check | `src/store.rs` | 101 | no | `test_file_from_another_project_is_refused` |
| rebuild deletes journal | `src/store.rs` | 101 | no | `fingerprint_rebuild_rule` |
| no `mark_announced` | `src/store.rs` | 101 | no | `journal_lifecycle`, `test_escalations_are_unannounced_until_marked` |
| no set sync alert | `src/store.rs` | 101 | no | `sync_state_alerts_and_outage_round_trip`, `test_sync_alert_round_trip` |
| no clear sync alert | `src/store.rs` | 101 | no | `sync_state_alerts_and_outage_round_trip`, `test_sync_alert_round_trip` |
| `updated_at` compared as text | `src/codec.rs` | 101 | no | `codec::tests::newer_compares_instants_and_never_text`, `newer_row_guard_compares_instants` |
| no membership take-over of Python rows | `src/schema.rs` | 101 | no | `a_python_written_v3_file_opens_and_reads_back_equal`, `rows_another_implementation_wrote_belong_to_their_phase` |
| read-back row of another bead accepted | `src/store.rs` | 101 | no | `a_read_back_row_of_another_bead_is_refused_and_changes_nothing` |
| one-connection claim on the raw path | `src/store.rs` | 101 | no | `one_connection_per_process_through_a_symlink` |
| Windows device names accepted | `src/location.rs` | 101 | no | `identity_values_are_validated` |
| `applied_since` unbounded below | `src/store.rs` | 101 | no | `applied_since_is_inclusive_and_ordered` |
| store failures mapped to `NO_DATABASE` | `src/error.rs` | 101 | no | `a_store_failure_is_store_failed_with_its_source`, `error::tests::failures_carry_the_step_the_file_and_the_whole_chain` |
| `status` omits stale rows | `src/store.rs` | 101 | no | `status_lists_stale_rows_until_reread` |

The mutations, in the order above (paths under `crates/bd-cache-store/`):

1. `store_row` never skips (`&& is_newer(..)` replaced by `&& false`).
2. `initialize` never refuses another `project_id`.
3. `record_fingerprint`'s rebuild also runs `DELETE FROM journal`.
4. `mark_announced` updates `announced_at` to itself.
5. `set_sync_alert(Some)` writes nothing.
6. `set_sync_alert(None)` deletes nothing.
7. `is_newer` orders the stored and incoming text.
8. `RECONCILE_MEMBERSHIP` marks `bd_cache.py` rows known without taking
   their `phase` membership over.
9. `apply_with_readback` stores a row whose id is not the entry's bead.
10. `open_at` claims the path as given, not its canonical form.
11. `DoltDatabase` accepts Windows device names.
12. `applied_since`'s SQL bound and instant filter are removed.
13. `error::failed` builds `NO_DATABASE` (no source) instead of `STORE_FAILED`.
14. `status` returns an empty `stale` list.

Every bug fails at least one test, none by a compile error, and the tree
was clean after the run. Bugs 1 to 6 are the ones the acceptance criteria
list; 7 to 14 cover the instant rule, ADR-STORE-0002, ADR-STORE-0003, the fix-a-4-1 changes and the R2 changes.

## Validation

Local, on `28a1c86` (macOS; `cargo test --workspace --all-features`: 391 passed, store contract suite 19 rules):

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo clippy --workspace --lib -- -D warnings` | pass |
| `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps` | pass |
| `cargo test --workspace --all-features` | pass |
| `cargo build --workspace` | pass |

CI on `efd14ef` (code as at `1064d24`, plus this document) is green on
ubuntu, macos and windows, with the compatibility tests running on each:
run [40072300354](https://github.com/randlee/beads-cache/actions/runs/40072300354).
