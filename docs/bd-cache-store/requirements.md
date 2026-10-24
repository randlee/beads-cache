# bd-cache-store Requirements

Status: Approved (lead ruling, bc-lead, 2026-10-24). Crate-local requirements (domain code `STORE`)
refining [`../requirements.md`](../requirements.md). Architecture:
[`architecture.md`](architecture.md). Created by sprint a-4.

Test files are under `crates/bd-cache-store/tests/`.

## REQ-STORE-0001: Locating the file starts no bd process

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`CacheLocation::resolve` reads `dolt_database` and `project_id` from
`.beads/metadata.json`, or `$BEADS_DIR/metadata.json` when `BEADS_DIR` is
set and not empty, and names the file `cache_path` from
`.beads/bd-cache.toml` (relative to the repository root) or
`<repo>/.bd-cache/<dolt_database>.bd-cache.sqlite3`. A missing or unreadable
file, invalid JSON, a missing or non-string key, an empty project id or a
database name that is not a file name on every supported platform (empty,
`.`, `..`, a path separator, a control character, any of `: * ? " < > |`, a
trailing dot or space, a Windows device name) is `BD_CACHE.NO_DATABASE`.

**Refines:** REQ-BDC-0101, 0102
**Acceptance evidence:** `test_cache_file_is_named_for_the_dolt_database`,
`test_beads_dir_env_names_the_database`, `test_missing_metadata_is_typed`,
`configured_cache_path_is_taken_from_the_repository_root`,
`identity_values_are_validated` (`location.rs`).

## REQ-STORE-0002: Open checks settings, version and identity

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Open sets WAL mode (refused if SQLite keeps another mode) and a 5 s busy
timeout; `user_version` 0 is initialized to 3, 3 is opened, any other value
is `BD_CACHE.SCHEMA_VERSION` and the file is left unchanged. The first open
records `dolt_database` and `project_id` in `meta`; a later open with
another `project_id` is `BD_CACHE.WRONG_DATABASE`. A second open of the same
file in one process, while the first store lives, is refused, also through
another path (a symlink) that resolves to it.

**Refines:** REQ-BDC-0103, 0104; NFR-BDC-0007
**Acceptance evidence:** `test_newer_cache_schema_is_refused`,
`version_zero_is_initialized_to_three_and_three_is_kept`,
`test_file_from_another_project_is_refused`, `wal_mode_and_busy_timeout`,
`one_connection_per_process`, `one_connection_per_process_through_a_symlink`
(Unix) (`open.rs`).

## REQ-STORE-0003: v3 tables unchanged, extensions additive

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

The v3 tables and indexes are created with `bd_cache.py`'s DDL; the
extensions are the tables `scope`, `scope_member`, `ready_base` and nullable
columns only (ADR-STORE-0001). Stored rows are normalized and `row_json` is
Python's `json.dumps(sort_keys=True)` of the row with its edge triples.

**Refines:** REQ-BDC-0107, 0108, 0804; ADR-BDC-0001, 0002, 0004
**Acceptance evidence:** `v3_tables_are_unchanged_and_extensions_are_additive`
(`open.rs`), `stored_rows_are_normalized_and_written_for_python`
(`store.rs`).

## REQ-STORE-0004: Every CacheStore rule over SQLite

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`SqliteStore` keeps every rule of the `CacheStore` doc comment
(`bd-cache-model`, ADR-MODEL-0003), including the newer-row guard comparing
`updated_at` as an instant with unparseable values never newer, edges
written from `Bead.dependencies` and the edges index by target, membership
from labels on scoped upserts only, stale clearing, the rebuild rule keeping
the journal, ready bases, persisted scopes, `applied_since` on
`applied_at`, announce marks, sync alerts and the outage record, and
`prune_scope` keeping beads another scope holds, and `status` listing the
stale rows. Every mutating method is one `BEGIN IMMEDIATE` transaction;
`status`, `get_row` and `rows_in_scope` each read one snapshot. `apply_with_readback` refuses a row that
is not the entry's bead (`BD_CACHE.BD_CONTRACT`, nothing changed). Every
value read back is validated, and one that does not decode is an error
naming the entry. The store's own failures are `BD_CACHE.STORE_FAILED`
keeping the underlying error as the source (ADR-STORE-0003).

**Refines:** REQ-BDC-0105, 0109, 0405, 0502, 0504, 0601, 0602, 0604, 0901,
0902, 0909, 0603 (store side)
**Acceptance evidence:** the a-1 contract suite (`store_contract.rs`, 19
rules, including `status_lists_stale_rows_until_reread`); `test_dependents_use_the_edge_index`,
`test_escalations_are_unannounced_until_marked`, `test_sync_alert_round_trip`,
`test_pending_work_counts_as_open`,
`pruning_a_scope_deletes_only_beads_no_other_scope_holds`,
`undecodable_stored_values_are_errors_not_skipped`,
`a_read_back_row_of_another_bead_is_refused_and_changes_nothing` (`store.rs`);
`a_store_failure_is_store_failed_with_its_source` (`open.rs`).

## REQ-STORE-0005: Cross-process drain lock

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`FlockDrainLock` takes an exclusive non-blocking `flock` (Windows:
`LockFileEx`) on the cache file's lock file, named as v3 names it
(`<name>.bd-cache.drain.lock` next to the cache file). Each `try_acquire`
opens its own handle, so the lock is not reentrant; dropping the guard, or
the process ending, releases it.

**Refines:** REQ-BDC-0401
**Acceptance evidence:** `two_processes_contend_for_the_drain_lock`
(`drain_lock.rs`), `a_guard_dropped_on_another_thread_releases_the_lock`
(`store.rs`),
`drain_lock_is_exclusive` (contract suite),
`the_python_and_rust_drain_locks_exclude_each_other` (`compat.rs`, Unix).

## REQ-STORE-0006: The file is shared with bd_cache.py both ways

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A v3 file `bd_cache.py` wrote opens, and its rows, edges, journal, sync
state, alert and outage read back equal; `bd_cache.py`'s `get` and `status`
read a file this store wrote, extension tables present and intact
afterwards; after `bd_cache.py`'s `prune` and a forced rebuild on an
extended file, the store reads consistent rows and memberships
(ADR-STORE-0002).

**Refines:** REQ-BDC-0804
**Acceptance evidence:** `a_python_written_v3_file_opens_and_reads_back_equal`,
`the_python_get_and_status_read_a_file_the_rust_store_wrote`,
`after_python_prune_and_rebuild_the_rust_store_reads_consistent_rows`
(`compat.rs`); `rows_another_implementation_wrote_belong_to_their_phase`,
`a_python_row_written_while_open_is_read_by_its_phase` (`store.rs`).

## REQ-STORE-0007: The one repository configuration loader

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`load_repo_config(repo_root)` reads `<repo_root>/.beads/bd-cache.toml` into
a-1's `RepoConfig`: a missing file is the default; an unreadable file,
invalid TOML, an unknown key, a mistyped value or a `scopes` entry that is
not a phase name is a `ConfigLoadError` keeping its cause, whose message
shows the cause chain and a fix.

**Refines:** REQ-BDC-0101 (`cache_path`); a-1's `RepoConfig` contract
**Acceptance evidence:** `missing_repo_config_is_the_default`,
`repo_config_is_parsed_and_unknown_keys_refused`,
`unreadable_repo_config_is_an_error_not_the_default` (`location.rs`).
