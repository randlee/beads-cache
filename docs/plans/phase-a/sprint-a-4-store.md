---
id: a-4
phase: a
status: planned
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
  owns: [REQ-BDC-0101, REQ-BDC-0102, REQ-BDC-0103, REQ-BDC-0104, REQ-BDC-0107, REQ-BDC-0108, REQ-BDC-0109, REQ-BDC-0604, REQ-BDC-0804, REQ-BDC-0901, NFR-BDC-0007]
  constrained_by: [REQ-BDC-0105, REQ-BDC-0502, REQ-BDC-0602, REQ-BDC-0902]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0004, ADR-BDC-0005]
owned_paths:
  - "crates/bd-cache-store/**"
  - "docs/plans/phase-a/notes/a-4.md"
---

# Sprint a-4 — SQLite store

## Outcome and closure

Implement a-1's `CacheStore` port over SQLite. Closes when `SqliteStore`
passes a-1's store contract suite and its own cases, and reads a v3 file
written by the Python implementation.

## must_follow rationale

Implements the `CacheStore` trait and stores the `Bead`, `Edge` and
`JournalEntry` types defined in a-1.

## Decision prerequisites

P-2. If open at dispatch: ship the extensions as schema version 4 with a v3
read-and-migrate path. Either way, record the schema decision in
`docs/plans/phase-a/notes/a-4.md`; a-7 folds it into ADR-BDC-0001.

## Deliverables

1. File location from `.beads/metadata.json` (`$BEADS_DIR` aware), starting
   no `bd` process (REQ-BDC-0101); `NO_DATABASE` (REQ-BDC-0102).
2. Open: WAL, 5 s busy timeout, `user_version` check and `SCHEMA_VERSION`
   (REQ-BDC-0103); project id record and `WRONG_DATABASE` (REQ-BDC-0104);
   bundled rusqlite, one connection per process (NFR-BDC-0007).
3. Schema: the baseline v3 tables and indexes (REQ-BDC-0107) plus the
   extensions: `scope` and `scope_member` (REQ-BDC-0901, ADR-BDC-0001), the
   changed-blocker set and outside-blocker derivation (ADR-BDC-0002), nullable
   `revision` and `sync_state` columns `backend_kind`, `endpoint`,
   `replica_id`, `journal_seq` (ADR-BDC-0004).
4. Every `CacheStore` method, including the newer-row guard and forced
   upsert, row normalization (REQ-BDC-0108), the edges index by target
   (REQ-BDC-0109), `apply_with_readback` in one transaction,
   `record_fingerprint` with the rebuild rule (deletes beads, edges and
   cursors in one transaction, keeps the journal), announce marks
   (REQ-BDC-0604), sync alerts, and `prune_scope` keeping beads another scope
   holds.
5. Compatibility: a Python-written v3 file opens and reads (REQ-BDC-0804).

## Acceptance criteria

| Criterion | Root |
|---|---|
| a-1's store contract suite passes against `SqliteStore` | ADR-BDC-0004 |
| `test_cache_file_is_named_for_the_dolt_database`, `test_beads_dir_env_names_the_database`, `test_missing_metadata_is_typed` (REQ-BDC-0101, 0102) | boundary:bd-cache-store |
| `test_newer_cache_schema_is_refused`, `test_file_from_another_project_is_refused` (REQ-BDC-0103, 0104) | boundary:bd-cache-store |
| `test_dependents_use_the_edge_index` (REQ-BDC-0109) | boundary:bd-cache-store |
| `test_escalations_are_unannounced_until_marked`, `test_sync_alert_round_trip`, `test_pending_work_counts_as_open` (REQ-BDC-0604) | boundary:bd-cache-store |
| A v3 file written by the Python implementation opens, and its rows, edges and journal read back equal (REQ-BDC-0804) | boundary:bd-cache-store |
| Pruning a scope deletes only beads no other scope holds (REQ-BDC-0901) | ADR-BDC-0001 |
| Planted bugs: no newer-row guard, skip project-id check, rebuild deletes journal, no `mark_announced`, no set/clear sync alert each fail a test | boundary:bd-cache-store |

## This sprint does not close

Calling the store from drain, sync or refresh (a-5, a-6), or composing it
with the real backend (a-7).

## Required validation

Crate tests and lints; `cargo build --workspace`.
