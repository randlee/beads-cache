# beads-cache Requirements

Status: approved. Requirements below were approved by lead ruling on 2026-10-24 with the phase-a plan (merged by the owner in PR #1); amendments are marked inline.

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
in the same directory. Acceptance suite: `tests/test_bd_cache.py` (44 tests)
and `tests/test_bd_cache_cron.py` (13), with fixtures in
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

**Status:** Approved

A read of a cached bead is answered from the local SQLite file and starts no `bd` process.

**Source:** BD-CACHE.md Goals 1, How it works (Read)
**Acceptance evidence:** `test_miss_fetches_once_then_serves_from_cache`

## REQ-BDC-0002: Writes are journaled locally

**Status:** Approved

`write` validates the operation, checks its precondition against the current view and inserts a `pending` journal entry; it calls `bd` only to fill a cache miss.

**Source:** BD-CACHE.md How it works (Write), Algorithms (write)
**Acceptance evidence:** `test_write_is_visible_to_reads_but_cache_row_is_untouched`

## REQ-BDC-0003: Every write ends applied, refused or escalated

**Status:** Approved

Every write either lands in `bd` (verified by a read-back), is refused at write time with a typed error, is escalated to the phase lead with the fix, or stays pending while the backend is unreachable; no write is dropped or guessed, and a reachable backend never retries one past `MAX_ATTEMPTS`.

*Amended in planning (ADR or review): an unreachable backend leaves writes pending without spending attempts (ADR-BDC-0005); additive to the baseline, needs randlee/atm-bd-orchestration#557.*

**Source:** BD-CACHE.md Goals 2, Failure handling
**Acceptance evidence:** `test_retries_exhausted_escalate_and_never_silently_drop`

## REQ-BDC-0004: Idempotent writes

**Status:** Approved

`idem_key` is required. Re-writing the same key with the same `(bead, op, args)` returns the existing entry with `duplicate: true`; the same key with a different write is `BD_CACHE.IDEM_CONFLICT`.

**Source:** BD-CACHE.md Goals 3, Journal operations
**Acceptance evidence:** `test_duplicate_idem_key_is_one_entry_and_a_conflicting_reuse_is_refused`

## REQ-BDC-0005: A landed write is never repeated

**Status:** Approved

When the backend already holds a write's effect (`achieved`), the drain stores the fresh row and marks the entry applied without running the `bd` command again.

**Source:** BD-CACHE.md Goals 3, Algorithms (drain step 2)
**Acceptance evidence:** `test_write_that_landed_before_a_crash_is_not_repeated`

## REQ-BDC-0006: Backend access only through bd

**Status:** Approved

All backend access goes through `bd` (the CLI; `bd serve` under ADR-BDC-0004), so the cache works with every backend `bd` supports; it never reads dolt tables.

**Source:** BD-CACHE.md Goals 4

## REQ-BDC-0007: Strict row contract

**Status:** Approved

Every row is validated against types generated from `bd schema` plus the `--json` extras (`comment_count`, `dependency_count`, `dependent_count`, `parent`). An unknown key, missing required key or wrong type is `BD_CACHE.BD_CONTRACT` naming the bead and field.

**Source:** BD-CACHE.md Goals 5, Bead shape
**Acceptance evidence:** `test_recorded_bd_rows_validate`, `test_pinned_model_matches_recorded_schema`

## REQ-BDC-0008: A bad row stores nothing

**Status:** Approved

A sync or fetch whose result contains one row that fails validation stores no row from that result.

**Source:** BD-CACHE.md Bead shape
**Acceptance evidence:** `test_contract_violation_is_typed_and_sync_stores_nothing`

## REQ-BDC-0101: Cache file location without bd

**Status:** Approved

The cache file's location is set by `cache_path` in `.beads/bd-cache.toml`, defaulting to `<repo>/.bd-cache/<dolt_database>.bd-cache.sqlite3`; `dolt_database` and `project_id` come from `.beads/metadata.json` (`$BEADS_DIR` when set), so locating the file starts no `bd` process. A consumer such as btit finds the cache by reading that config.

*Amended in planning: the cache is not atm-bd-specific, so it moves from `.atm-bd/` to `.bd-cache/` by default; atm-bd can point `cache_path` at its existing `.atm-bd/` file during cutover (user, 2026-10-24).*

**Source:** BD-CACHE.md Storage
**Acceptance evidence:** `test_cache_file_is_named_for_the_dolt_database`, `test_beads_dir_env_names_the_database`

## REQ-BDC-0102: Missing metadata is typed

**Status:** Approved

Missing or invalid metadata is `BD_CACHE.NO_DATABASE`.

**Source:** BD-CACHE.md Storage
**Acceptance evidence:** `test_missing_metadata_is_typed`

## REQ-BDC-0103: SQLite settings and schema version

**Status:** Approved

The file uses WAL mode, a 5 s busy timeout and `PRAGMA user_version` 3. A file with another nonzero version is refused (`BD_CACHE.SCHEMA_VERSION`); version 0 is initialized. The Rust store never changes `user_version` without P-2 agreement; any later version change is an explicit migration command, never done on open.

*Amended in planning (ADR or review): schema version is held at 3 so Python and Rust can share a file (review PLAN-CRIT-003).*

**Source:** BD-CACHE.md Storage
**Acceptance evidence:** `test_newer_cache_schema_is_refused`

## REQ-BDC-0104: Wrong database is refused

**Status:** Approved

The first open records `dolt_database` and `project_id` in `meta`; a later open whose metadata `project_id` differs is `BD_CACHE.WRONG_DATABASE`.

**Source:** BD-CACHE.md Storage (meta keys)
**Acceptance evidence:** `test_file_from_another_project_is_refused`

## REQ-BDC-0105: Backend check and rebuild rule

**Status:** Approved

At the start of every sync and every non-empty drain the cache runs `bd version` and `bd schema` and hashes the schema output. The first check records it without dropping rows. A recorded sha that differs deletes every bead, edge and sync cursor in one transaction, keeps the journal and records the new sha. The check reports `rebuilt` and `model_current`.

**Source:** BD-CACHE.md Storage (backend check)
**Acceptance evidence:** `test_schema_change_rebuilds_rows_and_keeps_the_journal`, `test_first_backend_check_records_schema_without_dropping_cached_rows`, `test_model_current_when_live_schema_matches_pin`

## REQ-BDC-0106: No bd context

**Status:** Approved

Identity comes from `.beads/metadata.json`; `bd context` is never called.

**Source:** BD-CACHE.md Storage
**Acceptance evidence:** `test_backend_check_does_not_need_bd_context`

## REQ-BDC-0107: Cache schema

**Status:** Approved

The file holds the `meta`, `beads`, `edges`, `journal` and `sync_state` tables and indexes given in BD-CACHE.md, unchanged; extensions are additional tables and nullable columns only. `phase` is the suffix of the bead's first `phase-<x>` label; timestamps are RFC 3339 UTC, second precision.

*Amended in planning (ADR or review): extensions are additive only (ADR-BDC-0001, 0002, 0004, 0005).*

**Source:** BD-CACHE.md Storage (DDL)

## REQ-BDC-0108: Stored row normalization

**Status:** Approved

Stored rows drop `dependency_count` and `dependent_count` and default `labels` and `dependencies` to `[]` and `metadata` to `{}`.

**Source:** BD-CACHE.md Bead shape

## REQ-BDC-0109: Edge index

**Status:** Approved

Edges are stored as outgoing `{issue_id, depends_on_id, type}` rows indexed by target, so a bead's cached dependents are found without a backend call.

**Source:** BD-CACHE.md Storage (edges)
**Acceptance evidence:** `test_dependents_use_the_edge_index`

## REQ-BDC-0201: One backend read shape

**Status:** Approved

Every backend read is `bd list --id a,b,... --status open,in_progress,blocked,deferred,closed,pinned,hooked -n 0 --json` or its label form; `bd show` is never called.

**Source:** BD-CACHE.md Bead shape
**Acceptance evidence:** sc-bd-cli recorded-argv tests

## REQ-BDC-0202: get

**Status:** Approved

`get(bead)` returns the cached row, or on a miss makes one `bd list --id` call and stores the row; a bead absent from `bd` too is `BD_CACHE.NOT_FOUND`. Pending entries for that bead are replayed on top in `seq` order; escalated entries are not.

**Source:** BD-CACHE.md Algorithms (get)
**Acceptance evidence:** `test_miss_fetches_once_then_serves_from_cache`, `test_miss_on_unknown_bead_is_typed`

## REQ-BDC-0203: list

**Status:** Approved

`list --phase X [--status S] [--label L]` returns the cached rows of that phase with the overlay applied.

**Source:** BD-CACHE.md CLI

## REQ-BDC-0301: Journal operations

**Status:** Approved

The journal supports `close {reason?}`, `update` (a nonempty subset of status, assignee, add_labels, remove_labels, set_metadata, unset_metadata, append_notes, applied in that order) and `dep_add {depends_on, type}`, each with the overlay and `bd` command in BD-CACHE.md's table, passing `--actor`.

**Source:** BD-CACHE.md Journal operations
**Acceptance evidence:** `test_dep_add_overlays_then_lands_as_a_real_edge`, `test_bad_ops_are_typed`

## REQ-BDC-0302: Write-time precondition

**Status:** Approved

A write may carry a precondition `{status?, assignee?}` (`""` means unassigned or empty). An unmet precondition is `BD_CACHE.PRECONDITION` with `unmet {key: {expected, actual}}` and journals nothing.

**Source:** BD-CACHE.md Journal operations, Failure handling
**Acceptance evidence:** `test_precondition_checked_at_write_time`

## REQ-BDC-0303: Metadata values survive bd coercion

**Status:** Approved

A metadata value is accepted only as an int, a bool, or a string without surrounding whitespace that does not parse as a JSON number, boolean or null; anything else is `BD_CACHE.BAD_ARGS`. Bools are passed to `bd` as `true`/`false`.

**Source:** BD-CACHE.md Journal operations
**Acceptance evidence:** `test_metadata_values_round_trip_through_bd_coercion`

## REQ-BDC-0304: Dependency types

**Status:** Approved

`dep_add`'s type must be one of bd's dependency types, else `BD_CACHE.BAD_ARGS`.

**Source:** BD-CACHE.md Journal operations
**Acceptance evidence:** `test_bad_ops_are_typed`

## REQ-BDC-0305: achieved

**Status:** Approved

`achieved(row, op, args)` holds when: close, status is closed; dep_add, the edge is present; update, every requested field already holds (labels added present, removed absent, metadata keys equal or absent, notes contain the appended text).

**Source:** BD-CACHE.md Journal operations

## REQ-BDC-0401: Single drain writer

**Status:** Approved

The drain takes an exclusive non-blocking lock on `<cache file>.drain.lock`, next to the cache file; when it is held the drain returns `{busy: true}`.

*Amended in planning: the lock follows the configured cache path (REQ-BDC-0101).*

**Source:** BD-CACHE.md Algorithms (drain)
**Acceptance evidence:** `test_drain_is_single_writer`

## REQ-BDC-0402: Drain deadline

**Status:** Approved

The drain has two deadlines: a start gate (entries not started by it are reported in `deferred`, stay pending and keep their `attempts`) and a hard deadline that caps every `bd` call. A call cut short by the hard deadline leaves its entry pending with `attempts` unchanged, is reported as `deferred_in_flight`, and is neither an outage nor an attempt; the next drain's achieved check catches a write that landed.

*Amended in planning (ADR or review): resolves the conflict between "a started entry runs to completion" and capping calls at the time left (review PLAN-CRIT-013).*

**Source:** BD-CACHE.md Algorithms (drain), Performance
**Acceptance evidence:** `test_drain_deadline_defers_unstarted_entries_without_attempts`

## REQ-BDC-0403: Batched pre-read

**Status:** Approved

With pending entries, the drain runs the backend check, then one batched `bd list --id` of every bead with a pending entry. If the backend is unreachable at either step, the cache records an outage, counts no attempt and stops the drain.

*Amended in planning: v3 at 05831df: an unreachable-class failure (`BD_UNAVAILABLE`, `BD_VERSION_FAILED`, `BD_SCHEMA_FAILED`, `BD_LIST_FAILED`) at the check or pre-read is an outage; any other pre-read failure retries every entry with no attempt counted (lead ruling, bc-lead, 2026-10-24, a-5 R1).*

**Source:** BD-CACHE.md Algorithms (drain)
**Acceptance evidence:** `test_drain_pre_reads_all_pending_beads_in_one_call`, `test_failed_pre_read_retries_every_entry`

## REQ-BDC-0404: Per-entry drain steps

**Status:** Approved

For each pending entry by `seq`, skipping beads whose earlier entry failed in this pass or whose row is stale: a bead absent from the batch escalates; an achieved write is marked applied; an unmet precondition stores the fresh row and escalates; `bd` exit 13 escalates; any other failure or timeout of the write itself adds one attempt, keeps the entry pending and holds that bead for the pass, and reaching `MAX_ATTEMPTS` (5) escalates. An unreachable backend at the backend check or the pre-read is an outage: the drain stops there and no attempt is counted. A call cut short by the drain's hard deadline is neither (REQ-BDC-0402).

*Amended in planning: aligned with v3 at atm-core 05831df, where Rand approved the outage rule: outages are detected at the check, pre-read and sync lists; a write that fails or times out still counts an attempt.*

**Source:** BD-CACHE.md Algorithms (drain steps 1-4)
**Acceptance evidence:** `test_drain_on_absent_bead_escalates_not_found`, `test_concurrent_backend_change_escalates_instead_of_overwriting`, `test_bd_guard_refusal_exit_13_escalates`, `test_transient_write_failure_stays_pending_and_holds_later_entries_of_that_bead_only`

## REQ-BDC-0405: Read-back

**Status:** Approved

After a write the drain reads the bead back with `bd list --id` and in one transaction stores the row (forced) and marks the entry applied. A failed read-back marks the entry applied with the error, sets `stale = 1` and holds the bead's later entries until a sync re-reads it.

**Source:** BD-CACHE.md Algorithms (drain step 5)
**Acceptance evidence:** `test_drain_writes_in_order_reads_back_and_marks_applied`, `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`

## REQ-BDC-0406: Drain report

**Status:** Approved

The drain report lists `applied`, `retrying`, `escalated` (`seq, bead, op, actor, phase, reason`) and `deferred`, plus the additive `busy`, `deferred_in_flight` and `outage`.

*Amended in planning: additive fields for REQ-BDC-0402 and ADR-BDC-0005.*

**Source:** BD-CACHE.md Algorithms (drain)

## REQ-BDC-0501: Incremental sync

**Status:** Approved

`sync(phase)` runs the backend check, then `bd list -l phase-<x> -n 0 --status <all seven> --json`, with `--updated-after (last_ok_at - 2 min)` once a sync has succeeded; every row is validated before any is stored.

**Source:** BD-CACHE.md Algorithms (sync)
**Acceptance evidence:** `test_sync_is_incremental_and_never_replaces_a_newer_row`

## REQ-BDC-0502: Newer-row guard

**Status:** Approved

Sync upserts each row unless the stored `updated_at` is newer; drain read-backs are forced.

**Source:** BD-CACHE.md Algorithms (sync)
**Acceptance evidence:** `test_sync_is_incremental_and_never_replaces_a_newer_row`

## REQ-BDC-0503: Stale rows re-read

**Status:** Approved

After the phase list, sync re-reads `stale` rows with one `bd list --id`.

**Source:** BD-CACHE.md Algorithms (sync)
**Acceptance evidence:** `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`

## REQ-BDC-0504: Sync state

**Status:** Approved

`last_ok_at` is the sync's start time. A failure is `BD_CACHE.SYNC_FAILED`, recorded in `sync_state.last_error`.

**Source:** BD-CACHE.md Algorithms (sync)
**Acceptance evidence:** `test_sync_failure_is_typed_and_recorded`

## REQ-BDC-0601: resolve

**Status:** Approved

`resolve(seq, note, actor)` turns an escalated entry into `resolved` (never replayed) and appends the note to `last_error`; any other entry is `BD_CACHE.NOT_ESCALATED`.

**Source:** BD-CACHE.md Algorithms (resolve)
**Acceptance evidence:** `test_resolve_closes_only_escalated_entries_and_unblocks_prune`, `test_resolve_keeps_the_note_when_the_entry_had_no_error`

## REQ-BDC-0602: prune

**Status:** Approved

`prune(phase)` is refused (`BD_CACHE.PRUNE_BLOCKED`) while the phase has pending or escalated entries; otherwise it deletes the phase's beads, edges, journal and sync state.

**Source:** BD-CACHE.md Algorithms (prune)
**Acceptance evidence:** `test_prune_refuses_with_unapplied_writes`

## REQ-BDC-0603: status

**Status:** Approved

`status` returns the `meta` table, journal counts per `phase:state`, per-phase sync state and bead counts, and every escalated entry until it is resolved.

*Amended (lead ruling, bc-lead, 2026-10-24): `status` also lists the stale rows (beads whose read-back failed, REQ-BDC-0405) until they are re-read; carried by `StoreStatus.stale` (a-1 amendment 1).*

**Source:** BD-CACHE.md CLI, Failure handling
**Acceptance evidence:** a-6 status tests

## REQ-BDC-0604: Escalation bookkeeping

**Status:** Approved

An escalated entry stays unannounced until it is marked announced; marking records `announced_at`.

**Source:** BD-CACHE.md Storage (journal), Cron module
**Acceptance evidence:** `test_escalations_are_unannounced_until_marked`

## REQ-BDC-0701: One schedule per repository

**Status:** Superseded (when the phase-B planning PR merges)

*Superseded (phase-B boundary extension, `docs/plans/phase-b/phase-b-boundary.md`; lead ruling S82, Rand's rulings of 2026-10-25; takes effect when the planning PR merges): replaced by REQ-BDC-0720. The tick is a row in the cache file's sc-cron table, at most one per repository (A3), with no consumer team or lead configuration.*

The superseded text is in git history (before the phase-B planning PR).

## REQ-BDC-0702: Active phases

**Status:** Superseded (when the phase-B planning PR merges)

*Superseded (phase-B boundary extension, `docs/plans/phase-b/phase-b-boundary.md`; lead ruling S82, Rand's rulings of 2026-10-25; takes effect when the planning PR merges): replaced by REQ-BDC-0721. Scopes are configured in the cache as a name plus a bd filter (A5); no phase files are read.*

The superseded text is in git history (before the phase-B planning PR).

## REQ-BDC-0703: Tick budget

**Status:** Approved

A tick runs within a 25 s budget: drain with a deadline 5 s before the budget ends, then sync each active phase, deferring any not started 2 s before it.

*Amended (phase-B boundary extension; takes effect when the planning PR merges): the budget is the tick row's `budget_ms` (default 25000, range 1000–600000), with a `hook_reserve_ms` (default 5000) the drain and sync cannot use, so the post-sync hook always runs. The tick drains, then syncs each configured scope, deferring any not started in time (REQ-BDC-0720).*

**Source:** BD-CACHE.md Cron module
**Acceptance evidence:** `test_tick_budget_defers_drain_entries`

## REQ-BDC-0704: Sync failures announced once per cause

**Status:** Superseded (when the phase-B planning PR merges)

*Superseded (phase-B boundary extension, `docs/plans/phase-b/phase-b-boundary.md`; lead ruling S82, Rand's rulings of 2026-10-25; takes effect when the planning PR merges): replaced by REQ-BDC-0727 and REQ-BDC-0728. The cache records failure state; a registered post-sync hook decides what to tell anyone. The cache sends nothing (ADR-BDC-0010).*

The superseded text is in git history (before the phase-B planning PR).

## REQ-BDC-0705: Escalations announced once

**Status:** Superseded (when the phase-B planning PR merges)

*Superseded (phase-B boundary extension, `docs/plans/phase-b/phase-b-boundary.md`; lead ruling S82, Rand's rulings of 2026-10-25; takes effect when the planning PR merges): replaced by REQ-BDC-0727. Unnotified escalations are offered to the post-sync hook, which marks them with `journal announce`.*

The superseded text is in git history (before the phase-B planning PR).

## REQ-BDC-0706: Undelivered messages retried

**Status:** Superseded (when the phase-B planning PR merges)

*Superseded (phase-B boundary extension, `docs/plans/phase-b/phase-b-boundary.md`; lead ruling S82, Rand's rulings of 2026-10-25; takes effect when the planning PR merges): replaced by REQ-BDC-0727. Anything the hook does not mark is offered again on the next tick; there is no `refused` or `pass-through` reply.*

The superseded text is in git history (before the phase-B planning PR).

## REQ-BDC-0707: Stop rule

**Status:** Superseded (when the phase-B planning PR merges)

*Superseded (phase-B boundary extension, `docs/plans/phase-b/phase-b-boundary.md`; lead ruling S82, Rand's rulings of 2026-10-25; takes effect when the planning PR merges): removed. There is no consumer stop rule; a schedule is paused with sc-cron's `enabled` flag, and an sc-cron stop hook may continue, disable or delete it (A2).*

The superseded text is in git history (before the phase-B planning PR).

## REQ-BDC-0720: The tick on the cache's cron table

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

beads-cache syncs bd on a tick scheduled in the sc-cron tables of the cache file, at most one tick row per repository (A3), set with `bdc cache schedule set|show|enable|disable|clear`; `enable` and `disable` flip only the row's `enabled` flag (A4). `bdc cache cron run`, `cron run --once`, `bdc cache tick` and PyO3 `Cache.tick()` run the same Rust tick: lease, drain, sync, post-sync hook, reply (S83, S90).

**Source:** phase-b-boundary.md §Scheduling, §One tick; S83, S90, A3, A4

## REQ-BDC-0721: Scopes are configured bd filters

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

A scope is a name plus a bd filter (labels all or any, status, type, assignee, or `--all`) with a method, `builtin` by default, `python` or `external`, managed with `bdc cache scopes add|set|remove|list`. Removing a scope deletes its membership rows, not bead rows another scope still holds. A tick with no scopes still drains and runs the hook.

**Source:** phase-b-boundary.md §Scopes; A5

## REQ-BDC-0722: One tick at a time

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

Ticks are single-flight across processes through a `tick_lease` row taken under `BEGIN IMMEDIATE`, expiring at the deadline plus 10 s. A tick that finds a live lease replies `skipped: in_flight`; an expired lease is taken over.

**Source:** phase-b-boundary.md §Single flight; S90

## REQ-BDC-0723: Registered methods run isolated

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

A registered method runs through sc-cron's Python runner as `<absolute interpreter> -I -m bd_cache_rs.run <kind> <module:function>`, from a private temp cwd, with only the allowlisted environment, its output capped, and killed with its process group or job object at its deadline (NFR-BDC-0004). There is no in-process call. The tick refuses to run registered code from a cache file not owned by the current user, or group- or world-writable.

**Source:** phase-b-boundary.md §Calling a Python method; S89, S90, A2

## REQ-BDC-0724: Registration lives in the cache file

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

The `sync` and `post_sync` methods are registered in the cache file's `registration` table (`bdc cache hooks register|show|clear`, PyO3 `Cache.hooks_*`) with an absolute interpreter and a `module:function`; `.beads/bd-cache.toml` cannot register code (`CONFIG_INVALID`). A relative interpreter or a function that does not import is `REGISTRATION_INVALID`.

**Source:** phase-b-boundary.md §Registration; S89

## REQ-BDC-0725: A consumer's Python sync replaces the built-in one per scope

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

`builtin` scopes are synced with `bd list` and `bd blocked`; `python` scopes go to the registered `sync` method once per tick; `external` scopes are never synced. A python scope counts as ingested only when an `ingest_scope` call stamped with the tick's `tick_id` committed for it; one neither ingested nor reported failed is `TICK_SYNC_MISSING`.

**Source:** phase-b-boundary.md §One tick, §Request and result shapes; S87, decision D1

## REQ-BDC-0726: Ingest is as strict as the built-in sync

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

`ingest_scope(scope, rows, blocked, revision=None, allow_empty=False)` (Rust, `bdc cache scopes ingest`, PyO3) applies the strict row decoder, declared membership, the built-in `blocked` rule, the newer-row guard and one transaction; an empty snapshot is refused unless allowed; at most 50 000 rows and 64 MiB; any violation is `INGEST_INVALID` with the stored scope untouched.

**Source:** phase-b-boundary.md §Ingest; S87

## REQ-BDC-0727: The post-sync hook gets every result, and delivery is at-least-once

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

After the sync, a registered `post_sync` method gets the drain result, each scope's result and error with `failing_since` and `notified`, the escalated entries not yet notified with `first_offered_at`, and the pending count, with at least `hook_reserve_ms` left. The tick marks nothing; anything the hook does not mark is offered again on the next tick. The cache sends nothing (ADR-BDC-0010).

**Source:** phase-b-boundary.md §Request and result shapes, §Failure state and marking; S88

## REQ-BDC-0728: Failure state and compare-and-set marking

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

`failing_since` is set on the first non-ok result after an ok, kept across consecutive failures, cleared on ok, and opaque to callers. `scope_announce(scope, since)` / `bdc cache scopes announce <scope> --since <token>` marks only when `since` equals the current `failing_since`, otherwise `PRECONDITION`. `journal_announce` is unchanged.

**Source:** phase-b-boundary.md §Failure state and marking; S88

## REQ-BDC-0729: Outage is classified, and the new codes are typed

**Status:** Proposed (phase-B boundary extension; approved when the planning PR merges)

Errors that can come from bd carry `details.outage: bool`, true only when bd could not be reached (spawn failure, timeout, lock, transport); a `BD_LIST_FAILED` from a bd that answered has `outage: false`. The codes `TICK_SYNC_MISSING`, `TICK_RUN_FAILED`, `INGEST_INVALID` and `REGISTRATION_INVALID` have the kinds and exits of the codes table (REQ-BDC-0803).

**Source:** phase-b-boundary.md §New codes; ADR-BDC-0005

## REQ-BDC-0801: CLI commands

**Status:** Approved

The CLI offers the commands of `docs/bd-cache-cli/contract.md`: a beads surface shaped like `/v0`, and a `cache` management surface. Each command prints one sc-command envelope on stdout. The BD-CACHE verbs map to these commands as the contract's §5 and §6 list (ADR-BDC-0009).

*Amended by lead ruling S60 (bc-lead, 2026-10-25) and ADR-BDC-0009: was `get`, `list`, `write`, `drain`, `sync`, `prune`, `resolve` and `status` with the BD-CACHE.md arguments; the CLI contract is `docs/bd-cache-cli/contract.md`.*

**Source:** BD-CACHE.md CLI

## REQ-BDC-0802: Exit codes and typed errors

**Status:** Approved

Exit 0 is `ok: true`. Exit 2 is a usage error (`BD_CACHE.USAGE`), reported in the envelope. Exit 3 is any other typed error, with `{kind, code, message, details, suggested_action}` in `error`.

*Amended by lead ruling S60 (bc-lead, 2026-10-25) and ADR-BDC-0009: was exit 3 with `{ok: false, code, detail, fix, ...}`; the CLI contract is `docs/bd-cache-cli/contract.md`.*

**Source:** BD-CACHE.md CLI

## REQ-BDC-0803: Error codes

**Status:** Approved

The error codes are those of BD-CACHE.md, plus the additive `SCOPE_UNKNOWN`, `STORE_FAILED`, `STORE_IN_USE`, `USAGE`, `UNSUPPORTED`, `CONFIG_INVALID`, `NOT_CACHED`, `INTERNAL` and `TICK_*`. Each code has one fixed kind, given in the contract's error table. `NO_TOML` is never emitted.

*Amended in planning: SCOPE_UNKNOWN added for REQ-BDC-0902; NO_TOML retired.*

*Amended by lead ruling S60 (bc-lead, 2026-10-25) and ADR-BDC-0009: adds the codes after `SCOPE_UNKNOWN` and one fixed kind per code; the CLI contract is `docs/bd-cache-cli/contract.md`.*

**Source:** BD-CACHE.md CLI
**Acceptance evidence:** `test_bad_ops_are_typed`

## REQ-BDC-0804: Drop-in compatibility

**Status:** Approved

The SQLite file is shared with the Python implementation: a file written by either is read by the other. The Rust store writes the v3 tables unchanged plus only additive tables and nullable columns.

*Amended (phase-B boundary extension, S84; takes effect when the planning PR merges): the store file stays readable and writable by any v3 reader; the format is v3 plus the additive extensions of ADR-STORE-0006, with `user_version` 3. Compatibility with a consumer's own implementation is the consumer's test, run against the published wheel; beads-cache vendors no consumer code.*

*Amended in planning (ADR or review): split from the CLI half, now REQ-BDC-0806 (review PLAN-CRIT-003).*

**Source:** BD-CACHE.md Rust shape

## REQ-BDC-0805: Command name

**Status:** Approved

The primary command is `bd-cache`, with the alias `bdc`.

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0806: CLI and JSON drop-in

**Status:** Superseded

The `bd-cache` CLI, its JSON replies and exit codes are unchanged from the Python implementation, so hooks can switch binaries without edits.

*Superseded by lead ruling S59 and, with the CLI contract, by lead ruling S60 (bc-lead, 2026-10-25) and ADR-BDC-0009: parity with `bd_cache.py` is behavioural, and the CLI follows sc-command and `docs/bd-cache-cli/contract.md`.*

**Source:** BD-CACHE.md Rust shape (split from REQ-BDC-0804)

## REQ-BDC-0807: One request vocabulary

**Status:** Proposed (phase-b plan; approved when the plan PR merges)

The CLI, the Python bindings and the later MCP server decode every request through one `bd-cache-api` decoder, so the same input gives the same reply or the same error code on every front end. A `/v0` operation, parameter or member the cache does not serve is `BD_CACHE.UNSUPPORTED`; an unknown member is `BD_CACHE.BAD_ARGS`; neither is ignored.

**Source:** contract §1 rules 5–6, §9; phase-b plan, from `docs/bd-cache-cli/contract.md` (approved, lead ruling S60, 2026-10-25)

## REQ-BDC-0808: Stated order and paging

**Status:** Proposed (phase-b plan; approved when the plan PR merges)

Every list has a stated total order. `issues list` pages with an opaque keyset cursor; inside a known scope the default is no limit.

**Source:** contract §1 rule 7, §5.3, §6; phase-b plan, from `docs/bd-cache-cli/contract.md` (approved, lead ruling S60, 2026-10-25)

## REQ-BDC-0809: A write reply names its journal entry

**Status:** Proposed (phase-b plan; approved when the plan PR merges)

A journaled write returns its entry (`seq`, `state`, `duplicate`, idempotency key) and the views before and after it, all from one store transaction. `changed` is stored with the entry, and a duplicate returns the stored value.

**Source:** contract §1 rule 4, §5.1, §9; phase-b plan, from `docs/bd-cache-cli/contract.md` (approved, lead ruling S60, 2026-10-25)

## REQ-BDC-0810: CLI configuration

**Status:** Proposed (phase-b plan; approved when the plan PR merges)

Each setting is resolved as flag, then `BD_CACHE__*` env, then `.beads/bd-cache.toml`, then the default. The bd binary is never read from TOML, `--format` is read from the flag only, there is no upward search, and the legacy `.atm-bd/` file is refused as the contract states.

**Source:** contract §2; phase-b plan, from `docs/bd-cache-cli/contract.md` (approved, lead ruling S60, 2026-10-25)

## REQ-BDC-0811: Contract version

**Status:** Proposed (phase-b plan; approved when the plan PR merges)

`context`, `cache status`, `--version` and `bd_cache_rs.CONTRACT_VERSION` report the contract's semver version.

**Source:** contract §11; phase-b plan, from `docs/bd-cache-cli/contract.md` (approved, lead ruling S60, 2026-10-25)

## REQ-BDC-0812: Python 0.2 is additive

**Status:** Proposed (phase-b plan; approved when the plan PR merges)

Wheel 0.2 adds the `cache.beads` and `cache.manage` namespaces. Every 0.1 verb, argument and dict is unchanged, and 0.2.x issues no `DeprecationWarning`.

**Source:** contract §10; fenix designs against 0.1; phase-b plan, from `docs/bd-cache-cli/contract.md` (approved, lead ruling S60, 2026-10-25)

## REQ-BDC-0813: Binary distribution

**Status:** Proposed (phase-b plan; approved when the plan PR merges)

`bd-cache` and `bdc` are released as binaries for Linux, macOS (arm and intel) and Windows: on the GitHub Release and through Homebrew, Scoop and WinGet. A GitHub prerelease carries them.

**Source:** phase-b plan; sc-publish getting-started; phase-b plan, from `docs/bd-cache-cli/contract.md` (approved, lead ruling S60, 2026-10-25)

## REQ-BDC-0901: Several phases at once

**Status:** Approved

One cache file holds several `phase-*` scopes at once.

**Source:** user, 2026-10-23 planning discussion; BD-CACHE.md Scope

## REQ-BDC-0902: Filtered cache

**Status:** Approved

Sync holds only the beads in the configured scopes, never a whole database. A row fetched by `get` or `write` outside every scope is kept with no scope membership, refreshed by the drain's read-back, and swept by `prune` once it has no pending or escalated entry.

*Amended in planning (ADR or review): defines out-of-scope rows (review PLAN-CRIT-008).*

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0903: Slow or unreachable backend

**Status:** Approved

A slow or unreachable backend does not slow or fail local reads and writes of cached beads; reads report staleness and the outage, writes stay journaled, and drain and sync catch up when the backend responds. A write to an uncached bead during an outage returns `BD_UNAVAILABLE` within the miss-fill timeout.

*Amended in planning: narrowed to cached beads (critical review round 2).*

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0904: Blocked state across the cache edge

**Status:** Approved

Cached blocked state and `ready` stay correct when a blocker inside or outside the cache is closed or reopened, within one sync of the change; a bead whose blocked state is unknown is never reported ready.

*Amended in planning: P-6 probe showed list rows carry no `is_blocked`; blocked state comes from `bd blocked` per scope (ADR-BDC-0002).*

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0907: Fresh read

**Status:** Approved

A caller can read beads fresh from the backend, bypassing cached rows: one `bd list --id` of the requested beads (all statuses), stored with force, with pending journal entries replayed on top, under a caller-supplied timeout.

**Source:** fenix, 2026-10-24 (atm-bd close-vars and the sanity hook's batched refresh; a cached row can lag a direct `bd update`)

## REQ-BDC-0908: Journal entry by seq

**Status:** Approved

A caller can read one journal entry by `seq`, in any state.

**Source:** fenix, 2026-10-24 (atm-task-close.py reads the entry after its synchronous drain)

## REQ-BDC-0909: On-demand scopes

**Status:** Approved

Syncing a phase that is not configured registers it as an on-demand scope, so a later `list` of it works; only a scope that has never been synced or configured is refused.

**Source:** fenix, 2026-10-24 (close_fields_backfill syncs then lists arbitrary phases)

## REQ-BDC-0910: Python bindings

**Status:** Approved

The cache is callable in process from Python (maturin / PyO3), with the verbs atm-bd uses today, so atm-bd can replace `bd_cache.py` without the `bdc` binary or sc-runtime.

**Source:** user, 2026-10-24 (atm already provides tokio and cron; the bindings need nothing from sc-runtime)

## REQ-BDC-0911: Ready reflects our own writes immediately

**Status:** Approved

After a write journaled through the cache that releases or blocks a bead (for example a dev close releasing its sanity bead), `ready` read through the cache reflects it at once, with no `bd` call, even when the backend is unreachable.

**Source:** user, 2026-10-24 (the sanity agent's `atm task open` hook checks ready through the cache one or two turns after assignment and escalates if not ready)

## REQ-BDC-0912: Bounded fresh ready

**Status:** Approved

A caller can ask `ready` for a fresh answer within a time limit it gives (for example 500 ms, inside a hook's 2.5 s budget). If bd answers in time, the reply uses bd's answer and refreshes the stored base; if not, it returns the cached, locally adjusted answer by the limit, naming the fallback error. The call never exceeds the limit.

**Source:** user, 2026-10-24 (atm task hook: 2.5 s budget shared by several bd calls); same pattern as the sanity hook's capped refresh with `read_fallback` in BD-CACHE.md

## REQ-BDC-0905: bd release compatibility check

**Status:** Approved

Each beads release triggers a CI run that checks the new `bd` against the pinned fingerprint, the contract tests and the live probes.

**Source:** user, 2026-10-23 planning discussion

## REQ-BDC-0906: Shared backend crates

**Status:** Approved

The beads backend is shared as published crates (`sc-beads` traits, `sc-bd-cli` implementation) that keep every trait and feature beads-task-issue-tracker uses today.

**Source:** user, 2026-10-23 planning discussion

## Non-Functional Requirements

## NFR-BDC-0001: In-process get latency

**Status:** Approved

An in-process cached `get` takes under 1 ms when at most `READ_DATING_THRESHOLD` (2000) undated journal entries exist and no foreign writer holds the write lock. The one `get` that dates a larger burst, and gets while a foreign writer holds the lock over more than 2000 undated entries, are bounded excursions recorded in ADR-STORE-0004 (lead rulings S26 and S31).

**Source:** BD-CACHE.md Performance

## NFR-BDC-0002: CLI get latency

**Status:** Approved

A new-process CLI `get` takes under 50 ms.

**Source:** BD-CACHE.md Performance

## NFR-BDC-0003: Incremental sync time

**Status:** Approved

An incremental sync of a 1000-bead phase takes under 1 s.

**Source:** BD-CACHE.md Performance

## NFR-BDC-0004: bd calls bounded by the deadline

**Status:** Approved

Each `bd` call has a 30 s timeout and is capped at the time left before the caller's deadline, so a tick never exceeds its budget.

**Source:** BD-CACHE.md Performance (drain bounds), Open items

## NFR-BDC-0005: Unit tests without bd

**Status:** Approved

Unit tests run with no `bd` and no dolt, using a deterministic fake runner; each ported test fails against at least one of the planted bugs listed in BD-CACHE.md.

**Source:** BD-CACHE.md Tests

## NFR-BDC-0006: Live tests on the testbed only

**Status:** Approved

Live integration tests run only on the testbed or in the release-triggered bd compatibility workflow (REQ-BDC-0905), never in unit or pull-request suites.

*Amended in planning (ADR or review): the compat workflow is the one permitted CI use of live probes (review PLAN-CRIT-024).*

**Source:** BD-CACHE.md Tests

## NFR-BDC-0007: Bundled SQLite

**Status:** Approved

SQLite is bundled (`rusqlite` with `bundled`). The cache file has one connection per process. Other stores in the same process (for example `sc-cron`'s schedule store) use their own database file and their own connection, never a second connection to the cache file.

*Amended in planning: scoped to the cache file (a-3 QA-1, REQ-QA-002).*

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
