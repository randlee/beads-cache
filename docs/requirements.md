# beads-cache Requirements

Status: draft. Requirements below are Draft until approved.

## Scope

beads-cache extracts the beads SQLite cache that is in use today inside
atm-core, where it is part of the `atm-bd-orchestration` skill (which also
carries an integrated TUI and other orchestration features). The baseline is
the cache as atm-core runs it now: requirements here are drawn from that
implementation and scoped to the cache functionality, delivered first as a
minimum viable product (MVP). Anything left out of the MVP is recorded below
as out of scope, not dropped silently.

beads-cache is limited to the SQLite beads cache application. The hooks,
TUI and other orchestration features of `atm-bd-orchestration` stay in
atm-core. The application runs a sync task that keeps the cache in step with
a dolt server, which may be local or remote.
Its value is as a filtered local cache: it holds only the beads in its
configured sync scopes (several `phase-*` scopes at once today), because bd
databases are too large to copy whole. Besides the `bd-cache`/`bdc` CLI, a
later front end is an HTTPS MCP server that serves the local cache.

beads-cache is built against sc-runtime, which is being specified in
parallel. Needs that sc-runtime does not yet meet go to the sc-runtime team
as requests; beads-cache does not work around them locally.

Baseline (the contract): `.claude/skills/atm-bd-orchestration/scripts/BD-CACHE.md`
in atm-core, introduced by https://github.com/randlee/atm-core/pull/2219.
Reference implementation: `bd_cache.py`, `bd_models.py` and `bd_cache_cron.py`
in the same directory. Acceptance suite: `tests/test_bd_cache.py` (41 tests)
and `tests/test_bd_cache_cron.py` (9), with fixtures in
`tests/fixtures/bd-1.3.1/`. Design tracker:
https://github.com/randlee/atm-bd-orchestration/issues/557. Where BD-CACHE.md
and the Python code disagree, report it on that issue.
Upstream dependency: sc-runtime (https://github.com/randlee/sc-runtime).

### Out of Scope for the MVP

From BD-CACHE.md Scope (v1): creating beads (pours stay `bd import` /
sc-compose), comments, dependency removal, and float, null or structured
metadata values. Agents' own `bd show` / `bd ready` keep reading the
backend and lag journaled writes until the drain applies them.

## Requirement ID Convention

- Functional requirements: `REQ-<DOMAIN>-nnnn`; non-functional
  requirements: `NFR-<DOMAIN>-nnnn`. `<DOMAIN>` is the product's domain code
  `BDC`, or the crate's domain code in per-crate docs.
- Each requirement is its own section: `## REQ-<DOMAIN>-nnnn: Title`.
- IDs are never reused. A retired requirement keeps its ID with
  `**Status:** Retired` and a pointer to its replacement.
- Requirement template: each requirement carries `**Status:**` (Draft, Approved, Active, Retired),
  a one-sentence testable statement, and its acceptance evidence.

## Functional Requirements

Drawn from atm-core `BD-CACHE.md` (PR randlee/atm-core#2219) with the
section as provenance, plus the requirements stated in planning (source
"user"). IDs are grouped by area: 00xx goals, 01xx storage, 02xx reads,
03xx journal, 04xx drain, 05xx sync, 06xx resolve/prune/status, 07xx sync
task, 08xx CLI, 09xx planning requirements.

## REQ-BDC-0001: Reads never wait on the backend

**Status:** Draft

A read of a cached bead is answered from the local SQLite file and starts no `bd` process.

**Source:** BD-CACHE.md Goals 1, How it works (Read)
**Acceptance evidence:** `test_miss_fetches_once_then_serves_from_cache`

## REQ-BDC-0002: Writes are journaled locally

**Status:** Draft

`write` validates the operation, checks its precondition against the current view and inserts a `pending` journal entry; it calls `bd` only to fill a cache miss.

**Source:** BD-CACHE.md How it works (Write), Algorithms (write)
**Acceptance evidence:** `test_write_is_visible_to_reads_but_cache_row_is_untouched`

## REQ-BDC-0003: Every write ends applied, refused or escalated

**Status:** Draft

Every write either lands in `bd` (verified by a read-back), is refused at write time with a typed error, is escalated to the phase lead with the fix, or stays pending while the backend is unreachable; no write is dropped or guessed, and a reachable backend never retries one past `MAX_ATTEMPTS`.

*Amended in planning (ADR or review): an unreachable backend leaves writes pending without spending attempts (ADR-BDC-0005); additive to the baseline, needs randlee/atm-bd-orchestration#557.*

**Source:** BD-CACHE.md Goals 2, Failure handling
**Acceptance evidence:** `test_retries_exhausted_escalate_and_never_silently_drop`

## REQ-BDC-0004: Idempotent writes

**Status:** Draft

`idem_key` is required. Re-writing the same key with the same `(bead, op, args)` returns the existing entry with `duplicate: true`; the same key with a different write is `BD_CACHE.IDEM_CONFLICT`.

**Source:** BD-CACHE.md Goals 3, Journal operations
**Acceptance evidence:** `test_duplicate_idem_key_is_one_entry_and_a_conflicting_reuse_is_refused`

## REQ-BDC-0005: A landed write is never repeated

**Status:** Draft

When the backend already holds a write's effect (`achieved`), the drain stores the fresh row and marks the entry applied without running the `bd` command again.

**Source:** BD-CACHE.md Goals 3, Algorithms (drain step 2)
**Acceptance evidence:** `test_write_that_landed_before_a_crash_is_not_repeated`

## REQ-BDC-0006: Backend access only through bd

**Status:** Draft

All backend access goes through `bd` (the CLI; `bd serve` under ADR-BDC-0004), so the cache works with every backend `bd` supports; it never reads dolt tables.

**Source:** BD-CACHE.md Goals 4

## REQ-BDC-0007: Strict row contract

**Status:** Draft

Every row is validated against types generated from `bd schema` plus the `--json` extras (`comment_count`, `dependency_count`, `dependent_count`, `parent`). An unknown key, missing required key or wrong type is `BD_CACHE.BD_CONTRACT` naming the bead and field.

**Source:** BD-CACHE.md Goals 5, Bead shape
**Acceptance evidence:** `test_recorded_bd_rows_validate`, `test_pinned_model_matches_recorded_schema`

## REQ-BDC-0008: A bad row stores nothing

**Status:** Draft

A sync or fetch whose result contains one row that fails validation stores no row from that result.

**Source:** BD-CACHE.md Bead shape
**Acceptance evidence:** `test_contract_violation_is_typed_and_sync_stores_nothing`

## REQ-BDC-0101: Cache file location without bd

**Status:** Draft

The cache file's location is set by `cache_path` in `.beads/bd-cache.toml`, defaulting to `<repo>/.bd-cache/<dolt_database>.bd-cache.sqlite3`; `dolt_database` and `project_id` come from `.beads/metadata.json` (`$BEADS_DIR` when set), so locating the file starts no `bd` process. A consumer such as btit finds the cache by reading that config.

*Amended in planning: the cache is not atm-bd-specific, so it moves from `.atm-bd/` to `.bd-cache/` by default; atm-bd can point `cache_path` at its existing `.atm-bd/` file during cutover (user, 2026-10-24).*

**Source:** BD-CACHE.md Storage
**Acceptance evidence:** `test_cache_file_is_named_for_the_dolt_database`, `test_beads_dir_env_names_the_database`

## REQ-BDC-0102: Missing metadata is typed

**Status:** Draft

Missing or invalid metadata is `BD_CACHE.NO_DATABASE`.

**Source:** BD-CACHE.md Storage
**Acceptance evidence:** `test_missing_metadata_is_typed`

## REQ-BDC-0103: SQLite settings and schema version

**Status:** Draft

The file uses WAL mode, a 5 s busy timeout and `PRAGMA user_version` 3. A file with another nonzero version is refused (`BD_CACHE.SCHEMA_VERSION`); version 0 is initialized. The Rust store never changes `user_version` without P-2 agreement; any later version change is an explicit migration command, never done on open.

*Amended in planning (ADR or review): schema version is held at 3 so Python and Rust can share a file (review PLAN-CRIT-003).*

**Source:** BD-CACHE.md Storage
**Acceptance evidence:** `test_newer_cache_schema_is_refused`

## REQ-BDC-0104: Wrong database is refused

**Status:** Draft

The first open records `dolt_database` and `project_id` in `meta`; a later open whose metadata `project_id` differs is `BD_CACHE.WRONG_DATABASE`.

**Source:** BD-CACHE.md Storage (meta keys)
**Acceptance evidence:** `test_file_from_another_project_is_refused`

## REQ-BDC-0105: Backend check and rebuild rule

**Status:** Draft

At the start of every sync and every non-empty drain the cache runs `bd version` and `bd schema` and hashes the schema output. The first check records it without dropping rows. A recorded sha that differs deletes every bead, edge and sync cursor in one transaction, keeps the journal and records the new sha. The check reports `rebuilt` and `model_current`.

**Source:** BD-CACHE.md Storage (backend check)
**Acceptance evidence:** `test_schema_change_rebuilds_rows_and_keeps_the_journal`, `test_first_backend_check_records_schema_without_dropping_cached_rows`, `test_model_current_when_live_schema_matches_pin`

## REQ-BDC-0106: No bd context

**Status:** Draft

Identity comes from `.beads/metadata.json`; `bd context` is never called.

**Source:** BD-CACHE.md Storage
**Acceptance evidence:** `test_backend_check_does_not_need_bd_context`

## REQ-BDC-0107: Cache schema

**Status:** Draft

The file holds the `meta`, `beads`, `edges`, `journal` and `sync_state` tables and indexes given in BD-CACHE.md, unchanged; extensions are additional tables and nullable columns only. `phase` is the suffix of the bead's first `phase-<x>` label; timestamps are RFC 3339 UTC, second precision.

*Amended in planning (ADR or review): extensions are additive only (ADR-BDC-0001, 0002, 0004, 0005).*

**Source:** BD-CACHE.md Storage (DDL)

## REQ-BDC-0108: Stored row normalization

**Status:** Draft

Stored rows drop `dependency_count` and `dependent_count` and default `labels` and `dependencies` to `[]` and `metadata` to `{}`.

**Source:** BD-CACHE.md Bead shape

## REQ-BDC-0109: Edge index

**Status:** Draft

Edges are stored as outgoing `{issue_id, depends_on_id, type}` rows indexed by target, so a bead's cached dependents are found without a backend call.

**Source:** BD-CACHE.md Storage (edges)
**Acceptance evidence:** `test_dependents_use_the_edge_index`

## REQ-BDC-0201: One backend read shape

**Status:** Draft

Every backend read is `bd list --id a,b,... --status open,in_progress,blocked,deferred,closed,pinned,hooked -n 0 --json` or its label form; `bd show` is never called.

**Source:** BD-CACHE.md Bead shape
**Acceptance evidence:** sc-bd-cli recorded-argv tests

## REQ-BDC-0202: get

**Status:** Draft

`get(bead)` returns the cached row, or on a miss makes one `bd list --id` call and stores the row; a bead absent from `bd` too is `BD_CACHE.NOT_FOUND`. Pending entries for that bead are replayed on top in `seq` order; escalated entries are not.

**Source:** BD-CACHE.md Algorithms (get)
**Acceptance evidence:** `test_miss_fetches_once_then_serves_from_cache`, `test_miss_on_unknown_bead_is_typed`

## REQ-BDC-0203: list

**Status:** Draft

`list --phase X [--status S] [--label L]` returns the cached rows of that phase with the overlay applied.

**Source:** BD-CACHE.md CLI

## REQ-BDC-0301: Journal operations

**Status:** Draft

The journal supports `close {reason?}`, `update` (a nonempty subset of status, assignee, add_labels, remove_labels, set_metadata, unset_metadata, append_notes, applied in that order) and `dep_add {depends_on, type}`, each with the overlay and `bd` command in BD-CACHE.md's table, passing `--actor`.

**Source:** BD-CACHE.md Journal operations
**Acceptance evidence:** `test_dep_add_overlays_then_lands_as_a_real_edge`, `test_bad_ops_are_typed`

## REQ-BDC-0302: Write-time precondition

**Status:** Draft

A write may carry a precondition `{status?, assignee?}` (`""` means unassigned or empty). An unmet precondition is `BD_CACHE.PRECONDITION` with `unmet {key: {expected, actual}}` and journals nothing.

**Source:** BD-CACHE.md Journal operations, Failure handling
**Acceptance evidence:** `test_precondition_checked_at_write_time`

## REQ-BDC-0303: Metadata values survive bd coercion

**Status:** Draft

A metadata value is accepted only as an int, a bool, or a string without surrounding whitespace that does not parse as a JSON number, boolean or null; anything else is `BD_CACHE.BAD_ARGS`. Bools are passed to `bd` as `true`/`false`.

**Source:** BD-CACHE.md Journal operations
**Acceptance evidence:** `test_metadata_values_round_trip_through_bd_coercion`

## REQ-BDC-0304: Dependency types

**Status:** Draft

`dep_add`'s type must be one of bd's dependency types, else `BD_CACHE.BAD_ARGS`.

**Source:** BD-CACHE.md Journal operations
**Acceptance evidence:** `test_bad_ops_are_typed`

## REQ-BDC-0305: achieved

**Status:** Draft

`achieved(row, op, args)` holds when: close, status is closed; dep_add, the edge is present; update, every requested field already holds (labels added present, removed absent, metadata keys equal or absent, notes contain the appended text).

**Source:** BD-CACHE.md Journal operations

## REQ-BDC-0401: Single drain writer

**Status:** Draft

The drain takes an exclusive non-blocking lock on `<cache file>.drain.lock`, next to the cache file; when it is held the drain returns `{busy: true}`.

*Amended in planning: the lock follows the configured cache path (REQ-BDC-0101).*

**Source:** BD-CACHE.md Algorithms (drain)
**Acceptance evidence:** `test_drain_is_single_writer`

## REQ-BDC-0402: Drain deadline

**Status:** Draft

The drain has two deadlines: a start gate (entries not started by it are reported in `deferred`, stay pending and keep their `attempts`) and a hard deadline that caps every `bd` call. A call cut short by the hard deadline leaves its entry pending with `attempts` unchanged, is reported as `deferred_in_flight`, and is neither an outage nor an attempt; the next drain's achieved check catches a write that landed.

*Amended in planning (ADR or review): resolves the conflict between "a started entry runs to completion" and capping calls at the time left (review PLAN-CRIT-013).*

**Source:** BD-CACHE.md Algorithms (drain), Performance
**Acceptance evidence:** `test_drain_deadline_defers_unstarted_entries_without_attempts`

## REQ-BDC-0403: Batched pre-read

**Status:** Draft

With pending entries, the drain runs the backend check, then one batched `bd list --id` of every bead with a pending entry. If the backend is unreachable at either step, the cache records an outage, counts no attempt and stops the drain.

*Amended in planning: v3 at 05831df: a failed pre-read is an outage, not a retry of every entry.*

**Source:** BD-CACHE.md Algorithms (drain)
**Acceptance evidence:** `test_drain_pre_reads_all_pending_beads_in_one_call`, `test_failed_pre_read_retries_every_entry`

## REQ-BDC-0404: Per-entry drain steps

**Status:** Draft

For each pending entry by `seq`, skipping beads whose earlier entry failed in this pass or whose row is stale: a bead absent from the batch escalates; an achieved write is marked applied; an unmet precondition stores the fresh row and escalates; `bd` exit 13 escalates; any other failure or timeout of the write itself adds one attempt, keeps the entry pending and holds that bead for the pass, and reaching `MAX_ATTEMPTS` (5) escalates. An unreachable backend at the backend check or the pre-read is an outage: the drain stops there and no attempt is counted. A call cut short by the drain's hard deadline is neither (REQ-BDC-0402).

*Amended in planning: aligned with v3 at atm-core 05831df, where Rand approved the outage rule: outages are detected at the check, pre-read and sync lists; a write that fails or times out still counts an attempt.*

**Source:** BD-CACHE.md Algorithms (drain steps 1-4)
**Acceptance evidence:** `test_drain_on_absent_bead_escalates_not_found`, `test_concurrent_backend_change_escalates_instead_of_overwriting`, `test_bd_guard_refusal_exit_13_escalates`, `test_transient_write_failure_stays_pending_and_holds_later_entries_of_that_bead_only`

## REQ-BDC-0405: Read-back

**Status:** Draft

After a write the drain reads the bead back with `bd list --id` and in one transaction stores the row (forced) and marks the entry applied. A failed read-back marks the entry applied with the error, sets `stale = 1` and holds the bead's later entries until a sync re-reads it.

**Source:** BD-CACHE.md Algorithms (drain step 5)
**Acceptance evidence:** `test_drain_writes_in_order_reads_back_and_marks_applied`, `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`

## REQ-BDC-0406: Drain report

**Status:** Draft

The drain report lists `applied`, `retrying`, `escalated` (`seq, bead, op, actor, phase, reason`) and `deferred`, plus the additive `busy`, `deferred_in_flight` and `outage`.

*Amended in planning: additive fields for REQ-BDC-0402 and ADR-BDC-0005.*

**Source:** BD-CACHE.md Algorithms (drain)

## REQ-BDC-0501: Incremental sync

**Status:** Draft

`sync(phase)` runs the backend check, then `bd list -l phase-<x> -n 0 --status <all seven> --json`, with `--updated-after (last_ok_at - 2 min)` once a sync has succeeded; every row is validated before any is stored.

**Source:** BD-CACHE.md Algorithms (sync)
**Acceptance evidence:** `test_sync_is_incremental_and_never_replaces_a_newer_row`

## REQ-BDC-0502: Newer-row guard

**Status:** Draft

Sync upserts each row unless the stored `updated_at` is newer; drain read-backs are forced.

**Source:** BD-CACHE.md Algorithms (sync)
**Acceptance evidence:** `test_sync_is_incremental_and_never_replaces_a_newer_row`

## REQ-BDC-0503: Stale rows re-read

**Status:** Draft

After the phase list, sync re-reads `stale` rows with one `bd list --id`.

**Source:** BD-CACHE.md Algorithms (sync)
**Acceptance evidence:** `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`

## REQ-BDC-0504: Sync state

**Status:** Draft

`last_ok_at` is the sync's start time. A failure is `BD_CACHE.SYNC_FAILED`, recorded in `sync_state.last_error`.

**Source:** BD-CACHE.md Algorithms (sync)
**Acceptance evidence:** `test_sync_failure_is_typed_and_recorded`

## REQ-BDC-0601: resolve

**Status:** Draft

`resolve(seq, note, actor)` turns an escalated entry into `resolved` (never replayed) and appends the note to `last_error`; any other entry is `BD_CACHE.NOT_ESCALATED`.

**Source:** BD-CACHE.md Algorithms (resolve)
**Acceptance evidence:** `test_resolve_closes_only_escalated_entries_and_unblocks_prune`, `test_resolve_keeps_the_note_when_the_entry_had_no_error`

## REQ-BDC-0602: prune

**Status:** Draft

`prune(phase)` is refused (`BD_CACHE.PRUNE_BLOCKED`) while the phase has pending or escalated entries; otherwise it deletes the phase's beads, edges, journal and sync state.

**Source:** BD-CACHE.md Algorithms (prune)
**Acceptance evidence:** `test_prune_refuses_with_unapplied_writes`

## REQ-BDC-0603: status

**Status:** Draft

`status` returns the `meta` table, journal counts per `phase:state`, per-phase sync state and bead counts, and every escalated entry until it is resolved.

**Source:** BD-CACHE.md CLI, Failure handling
**Acceptance evidence:** a-6 status tests

## REQ-BDC-0604: Escalation bookkeeping

**Status:** Draft

An escalated entry stays unannounced until it is marked announced; marking records `announced_at`.

**Source:** BD-CACHE.md Storage (journal), Cron module
**Acceptance evidence:** `test_escalations_are_unannounced_until_marked`

## REQ-BDC-0701: One schedule per repository

**Status:** Draft

The sync task runs as one schedule per repository started from the beads root with a `team` (the ATM team of the phase leads); a missing team is refused.

**Source:** BD-CACHE.md Cron module
**Acceptance evidence:** `test_missing_team_is_refused`

## REQ-BDC-0702: Active phases

**Status:** Draft

Phases come from `.atm-bd/phase-*.toml` (`root`, `lead`), optionally limited by a configured phase list; a phase is active while its root bead is not closed, with uncached roots read in one `bd list --id`. A malformed phase list is refused.

**Source:** BD-CACHE.md Cron module
**Acceptance evidence:** `test_tick_syncs_only_phases_with_an_open_root`, `test_custom_phases_limits_sync_and_stop`, `test_malformed_custom_phases_is_refused`

## REQ-BDC-0703: Tick budget

**Status:** Draft

A tick runs within a 25 s budget: drain with a deadline 5 s before the budget ends, then sync each active phase, deferring any not started 2 s before it.

**Source:** BD-CACHE.md Cron module
**Acceptance evidence:** `test_tick_budget_defers_drain_entries`

## REQ-BDC-0704: Sync failures announced once per cause

**Status:** Draft

A sync failure whose cause differs from `sync_alert:<phase>` is sent to that phase's lead and recorded; a success clears it. An outage is announced the same way under the cause key `backend_unavailable`.

*Amended in planning: outage announcement uses the per-scope alert (ADR-BDC-0005).*

**Source:** BD-CACHE.md Cron module
**Acceptance evidence:** `test_sync_failure_is_announced_once_per_cause`

## REQ-BDC-0705: Escalations announced once

**Status:** Draft

Every escalated entry with no `announced_at` is sent to its phase's lead with the `resolve` command, then marked.

**Source:** BD-CACHE.md Cron module
**Acceptance evidence:** `test_escalation_is_sent_once_to_the_phase_lead`

## REQ-BDC-0706: Undelivered messages retried

**Status:** Draft

Any message not delivered, or an entry whose phase has no lead, makes the tick reply `refused` with the list, and is retried next tick; otherwise the reply is `pass-through`.

**Source:** BD-CACHE.md Cron module
**Acceptance evidence:** `test_undelivered_escalation_is_retried_and_the_tick_refuses`

## REQ-BDC-0707: Stop rule

**Status:** Draft

The schedule stops when no phase is active and no entry is pending or escalated-unannounced.

**Source:** BD-CACHE.md Cron module
**Acceptance evidence:** `test_stop_only_when_no_active_phase_and_no_open_work`

## REQ-BDC-0801: CLI commands

**Status:** Draft

The CLI offers `get`, `list`, `write`, `drain`, `sync`, `prune`, `resolve` and `status` with the arguments in BD-CACHE.md, printing one JSON object on stdout.

**Source:** BD-CACHE.md CLI

## REQ-BDC-0802: Exit codes and typed errors

**Status:** Draft

Exit 0 is ok, 2 is usage, 3 is a typed error `{ok: false, code, detail, fix, ...}`.

**Source:** BD-CACHE.md CLI

## REQ-BDC-0803: Error codes

**Status:** Draft

The error codes are those in BD-CACHE.md's table, each with its fix text, plus the additive `BD_CACHE.SCOPE_UNKNOWN`; `BD_CACHE.NO_TOML` (Python < 3.11 only) is never emitted.

*Amended in planning: SCOPE_UNKNOWN added for REQ-BDC-0902; NO_TOML retired.*

**Source:** BD-CACHE.md CLI
**Acceptance evidence:** `test_bad_ops_are_typed`

## REQ-BDC-0804: Drop-in compatibility

**Status:** Draft

The SQLite file is shared with the Python implementation: a file written by either is read by the other. The Rust store writes the v3 tables unchanged plus only additive tables and nullable columns.

*Amended in planning (ADR or review): split from the CLI half, now REQ-BDC-0806 (review PLAN-CRIT-003).*

**Source:** BD-CACHE.md Rust shape

## REQ-BDC-0805: Command name

**Status:** Draft

The primary command is `bd-cache`, with the alias `bdc`.

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0806: CLI and JSON drop-in

**Status:** Draft

The `bd-cache` CLI, its JSON replies and exit codes are unchanged from the Python implementation, so hooks can switch binaries without edits.

**Source:** BD-CACHE.md Rust shape (split from REQ-BDC-0804)

## REQ-BDC-0901: Several phases at once

**Status:** Draft

One cache file holds several `phase-*` scopes at once.

**Source:** user, 2026-10-23 planning discussion; BD-CACHE.md Scope

## REQ-BDC-0902: Filtered cache

**Status:** Draft

Sync holds only the beads in the configured scopes, never a whole database. A row fetched by `get` or `write` outside every scope is kept with no scope membership, refreshed by the drain's read-back, and swept by `prune` once it has no pending or escalated entry.

*Amended in planning (ADR or review): defines out-of-scope rows (review PLAN-CRIT-008).*

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0903: Slow or unreachable backend

**Status:** Draft

A slow or unreachable backend does not slow or fail local reads and writes of cached beads; reads report staleness and the outage, writes stay journaled, and drain and sync catch up when the backend responds. A write to an uncached bead during an outage returns `BD_UNAVAILABLE` within the miss-fill timeout.

*Amended in planning: narrowed to cached beads (critical review round 2).*

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0904: Blocked state across the cache edge

**Status:** Draft

Cached blocked state and `ready` stay correct when a blocker inside or outside the cache is closed or reopened, within one sync of the change; a bead whose blocked state is unknown is never reported ready.

*Amended in planning: P-6 probe showed list rows carry no `is_blocked`; blocked state comes from `bd blocked` per scope (ADR-BDC-0002).*

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0907: Fresh read

A caller can read beads fresh from the backend, bypassing cached rows: one `bd list --id` of the requested beads (all statuses), stored with force, with pending journal entries replayed on top, under a caller-supplied timeout.

**Source:** fenix, 2026-10-24 (atm-bd close-vars and the sanity hook's batched refresh; a cached row can lag a direct `bd update`)

## REQ-BDC-0908: Journal entry by seq

A caller can read one journal entry by `seq`, in any state.

**Source:** fenix, 2026-10-24 (atm-task-close.py reads the entry after its synchronous drain)

## REQ-BDC-0909: On-demand scopes

Syncing a phase that is not configured registers it as an on-demand scope, so a later `list` of it works; only a scope that has never been synced or configured is refused.

**Source:** fenix, 2026-10-24 (close_fields_backfill syncs then lists arbitrary phases)

## REQ-BDC-0910: Python bindings

The cache is callable in process from Python (maturin / PyO3), with the verbs atm-bd uses today, so atm-bd can replace `bd_cache.py` without the `bdc` binary or sc-runtime.

**Source:** user, 2026-10-24 (atm already provides tokio and cron; the bindings need nothing from sc-runtime)

## REQ-BDC-0905: bd release compatibility check

**Status:** Draft

Each beads release triggers a CI run that checks the new `bd` against the pinned fingerprint, the contract tests and the live probes.

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0906: Shared backend crates

**Status:** Draft

The beads backend is shared as published crates (`sc-beads` traits, `sc-bd-cli` implementation) that keep every trait and feature beads-task-issue-tracker uses today.

**Source:** user, 2026-10-23 planning discussion

## Non-Functional Requirements

## NFR-BDC-0001: In-process get latency

**Status:** Draft

An in-process cached `get` takes under 1 ms.

**Source:** BD-CACHE.md Performance

## NFR-BDC-0002: CLI get latency

**Status:** Draft

A new-process CLI `get` takes under 50 ms.

**Source:** BD-CACHE.md Performance

## NFR-BDC-0003: Incremental sync time

**Status:** Draft

An incremental sync of a 1000-bead phase takes under 1 s.

**Source:** BD-CACHE.md Performance

## NFR-BDC-0004: bd calls bounded by the deadline

**Status:** Draft

Each `bd` call has a 30 s timeout and is capped at the time left before the caller's deadline, so a tick never exceeds its budget.

**Source:** BD-CACHE.md Performance (drain bounds), Open items

## NFR-BDC-0005: Unit tests without bd

**Status:** Draft

Unit tests run with no `bd` and no dolt, using a deterministic fake runner; each ported test fails against at least one of the planted bugs listed in BD-CACHE.md.

**Source:** BD-CACHE.md Tests

## NFR-BDC-0006: Live tests on the testbed only

**Status:** Draft

Live integration tests run only on the testbed or in the release-triggered bd compatibility workflow (REQ-BDC-0905), never in unit or pull-request suites.

*Amended in planning (ADR or review): the compat workflow is the one permitted CI use of live probes (review PLAN-CRIT-024).*

**Source:** BD-CACHE.md Tests

## NFR-BDC-0007: Bundled SQLite

**Status:** Draft

SQLite is bundled (`rusqlite` with `bundled`); one connection per process.

**Source:** BD-CACHE.md Rust shape

## Per-Crate Requirements

Each crate's requirements live in `docs/<crate>/requirements.md` (see
`.claude/project/quality-policy.md`) and refine the ids above:

- [`sc-beads`](sc-beads/requirements.md) (`REQ-BEADS-*`)
- [`bd-cache-model`](bd-cache-model/requirements.md) (`REQ-MODEL-*`)
- [`bd-cache`](bd-cache/requirements.md) (`REQ-CORE-*`)
- [`sc-bd-cli`](sc-bd-cli/requirements.md) (`REQ-BDCLI-*`)
- [`sc-cron`](sc-cron/requirements.md) (`REQ-CRON-*`)
- [`bd-cache-store`](bd-cache-store/requirements.md) (`REQ-STORE-*`)
- [`bd-cache-sync`](bd-cache-sync/requirements.md) (`REQ-SYNC-*`)
- [`bd-cache-py`](bd-cache-py/requirements.md) (`REQ-PY-*`)
