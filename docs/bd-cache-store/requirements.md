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
trailing dot or space, a Windows device name including `CONIN$` and
`CONOUT$`) is `BD_CACHE.NO_DATABASE`. `CacheLocation::resolve_with` and
`SqliteStore::open_with` take the `$BEADS_DIR` value as an argument (`None` =
unset) and read no environment; `resolve` and `SqliteStore::open` pass the
process's own. Tests use the `_with` forms, so none depends on the ambient
`BEADS_DIR` (lead ruling S36).

**Refines:** REQ-BDC-0101, 0102
**Acceptance evidence:** `test_cache_file_is_named_for_the_dolt_database`,
`test_beads_dir_env_names_the_database` (`resolve_with` with an explicit and an
empty `$BEADS_DIR`, and `resolve` reading the real environment only in the child
process `beads_dir_env_child`, S36), `test_missing_metadata_is_typed`
(`resolve_with(.., None)` and `SqliteStore::open_with(.., None)`, S36),
`configured_cache_path_is_taken_from_the_repository_root`,
`identity_values_are_validated` (`tests/location.rs`); the
`SqliteStore::open_with(root, .., None)` doctests (`src/store.rs`, `src/lib.rs`).

Amendment (fix-a-7-15, S48, S49, S50, RBP-F047, RBP-F048, QA10-001, RSH-038, RSH-039; **Status:** Approved (lead rulings S48 and S49, bc-lead, 2026-10-24)):
the `$BEADS_DIR` value is one validated type, `BeadsDirOverride`, built once by
`BeadsDirOverride::new(Option<&OsStr>)`: unset or empty is `None`, any other value is
made absolute with `std::path::absolute` against the process's working directory (on
Unix joined with it, resolving nothing; on Windows through `GetFullPathNameW`, which
resolves `.` and `..` textually; S50). A value that cannot be made absolute is a
`BeadsDirError` (the value, and the I/O error as its source), never kept relative
(S48). `CacheLocation::resolve_with`, `SqliteStore::open_with` and `beads_dir` take
`Option<&BeadsDirOverride>`; `resolve` and `SqliteStore::open` build it from the
environment with `BeadsDirOverride::from_env`, and an unresolvable environment value
is `BD_CACHE.NO_DATABASE` naming the value, the I/O error and the remedy ("set an
absolute BEADS_DIR or run from a readable working directory", PE-RSH-043). Evidence (`src/location.rs`):
`an_unset_or_empty_beads_dir_is_no_override`,
`a_relative_beads_dir_is_made_absolute_against_the_working_directory`,
`an_absolute_beads_dir_is_kept`,
`a_beads_dir_that_cannot_be_made_absolute_is_an_error_with_its_cause` (the `absolute`
step injected to fail); the `BeadsDirOverride` doctest; `test_beads_dir_env_names_the_database`
(`tests/location.rs`, through the type). Design: the ADR-STORE-0003 amendment (S48/S49).

## REQ-STORE-0002: Open checks settings, version and identity

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Open sets a 5 s busy timeout; `user_version` 0 is initialized to 3, 3 is
opened, any other value is `BD_CACHE.SCHEMA_VERSION`. The first open
records `dolt_database` and `project_id` in `meta`; a later open with
another `project_id` is `BD_CACHE.WRONG_DATABASE`. A refused file is left
unchanged: version, data and journal mode (both checks run before WAL is
set). An accepted file is then set to WAL mode (refused if SQLite keeps
another mode). A second open of the same
file in one process, while the first store lives, is refused, also through
another path (a symlink) that resolves to it.

**Refines:** REQ-BDC-0103, 0104; NFR-BDC-0007
**Acceptance evidence:** `test_newer_cache_schema_is_refused`,
`version_zero_is_initialized_to_three_and_three_is_kept`,
`test_file_from_another_project_is_refused`,
`a_refused_file_keeps_its_journal_mode_version_and_data`, `wal_mode_and_busy_timeout`,
`one_connection_per_process`, `one_connection_per_process_through_a_symlink`
(Unix) (`open.rs`).

## REQ-STORE-0003: v3 tables unchanged, extensions additive

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

The v3 tables and indexes are created with the baseline Python implementation's DDL; the
extensions are the tables `scope`, `scope_member`, `ready_base` and
`ready_base_blocked`, nullable columns only (including `ready_base.indexed` and `journal.applied_epoch`) and
the plain indexes `scope_member_bead` (`scope_member(bead)`), `beads_membership_unknown`,
`journal_applied_epoch` and `journal_applied_unknown` (the last three partial); no index
holds a function (ADR-STORE-0001, as amended by ADR-STORE-0004, has the DDL). Stored rows are normalized and `row_json` is
Python's `json.dumps(sort_keys=True)` of the row with its edge triples.

*Amended (lead ruling, bc-lead, 2026-10-24, fix-a-7-2; revised by S18): the sprint a-7 extensions `ready_base_blocked`, `ready_base.indexed`, `journal.applied_epoch`, `journal_applied_epoch` and `journal_applied_unknown` are additive in the same way; the baseline Python implementation never reads them, and none uses a function, so the baseline Python implementation on SQLite older than 3.38 still reads and writes the file. Open drops the earlier `journal_applied_at` expression index and fills `applied_epoch` for applied entries without one.*

*Amended by ADR-STORE-0006 (Rand, 2026-10-26): the phase-B extensions (`scope.revision`, `scope.filter`, `sync_state.failing_since`, `sync_state.notified_since`, `journal.first_offered_at`, the tables `tick_lease`, `hook_state` and `sc_cron_schedule`, and the `meta` row `sc_cron_schedule_version`) are additive in the same way; acceptance evidence: the format golden (`format_golden.rs`).*

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
naming the entry. The store's own failures are `BD_CACHE.STORE_FAILED`,
keeping the underlying error as the source when there is one; an
undecodable or inconsistent stored value, a refused one-connection claim
and WAL refused have none (ADR-STORE-0003). A type-conversion error rusqlite raises while reading a column keeps that rusqlite error as its source (`failed`); a value the store's own validation refuses after reading has no source (`corrupt`).

**Refines:** REQ-BDC-0105, 0109, 0405, 0502, 0504, 0601, 0602, 0604, 0901,
0902, 0909, 0603 (store side)
**Acceptance evidence:** the a-1 contract suite (`store_contract.rs`, 25
rules, including `status_lists_stale_rows_until_reread` and b-1's
`insert_entry_stores_changed`, `error_writes_store_the_code`,
`journal_counts_rules`); `a_reader_never_sees_an_entry_without_its_changed`
and `journal_counts_is_one_query_and_its_buckets_sum_to_the_total`
(`store.rs` unit tests, sprint b-1); `test_dependents_use_the_edge_index`,
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

## REQ-STORE-0006: The file is shared with the baseline Python implementation both ways

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A v3 file the baseline Python implementation wrote opens, and its rows,
edges, journal, sync state, alert and outage read back equal; its `get` and
`status` read a file this store wrote, extension tables present and intact
afterwards; after its `prune` and a forced rebuild on an
extended file, the store reads consistent rows and memberships
(ADR-STORE-0002).

*Amended by ADR-STORE-0006 (Rand, 2026-10-26): the baseline implementation is no longer vendored (S84). From sprint b-9 the cross-implementation run is the consumer's own test against the published wheel; in this repository the format golden (`format_golden.rs`, sprint b-7) pins the file, and the read rules for rows and journal columns another implementation wrote keep their store tests (`rows_another_implementation_wrote_belong_to_their_phase`, `a_python_row_written_while_open_is_read_by_its_phase`). The `compat.rs` tests below are removed in b-9.*

**Refines:** REQ-BDC-0804
**Acceptance evidence:** `a_python_written_v3_file_opens_and_reads_back_equal`,
`the_python_get_and_status_read_a_file_the_rust_store_wrote`,
`after_python_prune_and_rebuild_the_rust_store_reads_consistent_rows`
(`compat.rs`); b-1's three journal columns both ways (ADR-STORE-0001,
Amendment b-1): `python_resolves_a_rust_written_entry_and_the_rust_store_splits_it_back`
(`compat.rs`); `rows_another_implementation_wrote_belong_to_their_phase`,
`a_python_row_written_while_open_is_read_by_its_phase` (`store.rs`).

## REQ-STORE-0007: The one repository configuration loader

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by ADR-BDC-0013 (sprint b-7: the loader also reads the `[[scope]]` tables, and a `scopes` entry is a `ConfigLoadError` only when it is not a valid `ScopeName`; a valid entry is a phase-a scope)

`load_repo_config(repo_root)` reads `<repo_root>/.beads/bd-cache.toml` into
a-1's `RepoConfig`: a missing file is the default; an unreadable file,
invalid TOML, an unknown key, a mistyped value, a zero `call_timeout_s`,
`miss_fill_timeout_ms` or `max_attempts` (`ConfigLoadError::Parse`, each with its
own message from `RepoConfig::validate`; PE-RSH-006) or a `scopes` entry that is
not a phase name is a `ConfigLoadError`. Its message names the file and a
fix; the cause is carried once, as `Error::source`. Each variant has one
store-owned code, `store_config_unreadable`, `store_config_invalid` or
`store_config_bad_scope` (not `CacheError` codes and not CLI-contract codes), and
one fix (`ConfigLoadError::code`, `fix`; ADR-STORE-0003 amendment, lead rulings S34
and S37). `ConfigLoadError` is exhaustive, so a new variant fails to compile in each
consumer until it is mapped (lead ruling S38).

**Refines:** REQ-BDC-0101 (`cache_path`); a-1's `RepoConfig` contract
**Acceptance evidence:** `missing_repo_config_is_the_default`,
`repo_config_is_parsed_and_unknown_keys_refused` (the `store_config_invalid` and
`store_config_bad_scope` codes, `Display` ending with `fix()`),
`unreadable_repo_config_is_an_error_not_the_default` (the `store_config_unreadable`
code), `zero_timeouts_and_attempts_are_refused_at_load_each_with_its_own_message`
(PE-RSH-006) (`tests/location.rs`, S37); the `ConfigLoadError::code` doctest
(`src/location.rs`).

## REQ-STORE-0008: Cached reads cost the same at any scope size and journal length

**Status:** Approved (lead ruling, bc-lead, 2026-10-24, fix-a-7-2)

`base_entry` reads one bead's entry of a scope's ready base from a per-bead
index, in one read snapshot, and falls back to reading the whole base when it
was not stored with the index (stored before it existed, or replaced by another
writer). `has_entries_since` is three index lookups (pending, applied with an epoch, applied without one), whatever the journal's
history; an entry the baseline Python implementation applied is dated by `updated_at`, and one with an unreadable time counts as touched. Neither changes what a reader sees.

**Refines:** NFR-BDC-0001; ADR-STORE-0004
**Acceptance evidence:** `base_entry_rules`, `has_entries_since_rules` (the store
contract suite, both stores); `the_ready_base_index_follows_the_base_through_replace_prune_and_rebuild`,
`the_entries_since_check_uses_indexes_and_ignores_a_long_history` (query plan),
`a_python_style_rebuild_or_prune_leaves_the_base_and_its_index_in_agreement`,
`a_base_replaced_by_another_writer_is_read_from_its_blob_not_from_a_stale_index`,
`a_file_from_before_the_ready_base_index_opens_and_migrates_on_the_next_replace`
(`store.rs`); `v3_tables_are_unchanged_and_extensions_are_additive` (`open.rs`).
