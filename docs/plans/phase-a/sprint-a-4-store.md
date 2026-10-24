---
id: a-4
phase: a
status: planned
closure_type: boundary
target_boundary: bd-cache-store
track: cache
wave: 2
branch: sprint/a-4-store
worktree: sprint/a-4-store
recommended_agent: rust-developer
recommended_model: opus
must_follow: [a-1]
parallel_safe: [a-2, a-3]
baseline_sections:
  - "BD-CACHE.md: Storage"
  - "BD-CACHE.md: Scope"
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0004, ADR-BDC-0005]
owned_paths:
  - "crates/bd-cache-store/**"
---

# Sprint a-4 — SQLite store

## Outcome and closure

The cache file and its transactional API. Closes when the store reads and
writes BD-CACHE v3 files, carries the additive extensions, and passes its
ported cases.

## must_follow rationale

Stores `Bead`, `Edge` and `JournalEntry` values typed in a-1.

## Decision prerequisites

- P-2: the additive extensions agreed (or the divergence recorded):
  `scope` and `scope_member` tables (ADR-BDC-0001), nullable `revision` and
  `sync_state` columns `backend_kind`, `endpoint`, `replica_id`,
  `journal_seq` (ADR-BDC-0004), outage state (ADR-BDC-0005). Decide whether
  the extensions keep `user_version` 3 or move to 4 with a migration, and
  whether the Python implementation must read the result.

## Deliverables

1. File location from `.beads/metadata.json` (`$BEADS_DIR` aware), no `bd`
   process; WAL, busy timeout 5 s, `user_version` check.
2. Schema: BD-CACHE v3 DDL plus the extensions; `phase` column kept.
3. Meta: project id check, backend fingerprint record, rebuild rule (delete
   beads, edges and cursors in one transaction, keep the journal).
4. Transactional API used by the core: row upsert with the newer-row guard
   and forced read-back, journal insert with idempotency key, state
   transitions, announce marks, scope membership and prune, edges index by
   target, outside-blocker set (ADR-BDC-0002), sync state and alerts.

## Acceptance criteria

| Criterion | Root |
|---|---|
| Ported: cache_file_is_named_for_the_dolt_database, missing_metadata_is_typed, beads_dir_env_names_the_database, file_from_another_project_is_refused, newer_cache_schema_is_refused | BD-CACHE.md Storage |
| Ported: dependents_use_the_edge_index, escalations_are_unannounced_until_marked, sync_alert_round_trip, pending_work_counts_as_open | BD-CACHE.md Storage |
| A Python-written v3 file opens and reads correctly | BD-CACHE.md Rust shape |
| Pruning a scope deletes only beads no other scope holds | ADR-BDC-0001 |

## This sprint does not close

- Drain, sync and refresh behaviour that uses the API (a-5).

## Required validation

Crate tests and lints; `cargo build --workspace`.
