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
  constrained_by: [REQ-BDC-0105, REQ-BDC-0401, REQ-BDC-0502, REQ-BDC-0602, REQ-BDC-0902]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0004, ADR-BDC-0005]
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

1. File location from `.beads/metadata.json` (`$BEADS_DIR` aware), starting
   no `bd` process (REQ-BDC-0101); `NO_DATABASE` (REQ-BDC-0102).
2. Open: WAL, 5 s busy timeout, `user_version` check and `SCHEMA_VERSION`
   (REQ-BDC-0103); project id record and `WRONG_DATABASE` (REQ-BDC-0104);
   bundled rusqlite, one connection per process (NFR-BDC-0007).
3. Schema: the baseline v3 tables and indexes (REQ-BDC-0107) plus the
   extensions: `scope` and `scope_member` (REQ-BDC-0901, ADR-BDC-0001), a per-scope `blocked_set` table (bead, `blocked_by`, fetched_at, stale;
   ADR-BDC-0002), nullable
   `revision` and `sync_state` columns `backend_kind`, `endpoint`,
   `replica_id`, `journal_seq` (ADR-BDC-0004).
4. Every `CacheStore` method and every rule in a-1's `CacheStore` doc comment
   (edges written from `Bead.dependencies` and rebuilt on read, membership,
   stale clearing, rebuild scope), including `ensure_scope`, membership from
   labels on scoped upserts, the stale-clearing rule, the outage record,
   out-of-scope rows stored with no membership and swept by `prune_scope`, the newer-row guard and forced
   upsert, row normalization (REQ-BDC-0108), the edges index by target
   (REQ-BDC-0109), `apply_with_readback` in one transaction,
   `record_fingerprint` with the rebuild rule (deletes beads, edges and
   cursors in one transaction, keeps the journal), announce marks
   (REQ-BDC-0604), sync alerts, and `prune_scope` keeping beads another scope
   holds.
5. `DrainLock` with `flock` on `.atm-bd/<dolt_database>.bd-cache.drain.lock`
   (cross-process; REQ-BDC-0401 constraint).
6. Compatibility both ways (REQ-BDC-0804): the reference `bd_cache.py`,
   `bd_models.py` and a v3 fixture, vendored at atm-core `015bdfb` under
   `crates/bd-cache-store/tests/compat/`, run with `uv` (installed by a-1's CI) to read a file the Rust store wrote;
   and the Rust store reads a Python-written one.
7. Per-crate docs `docs/bd-cache-store/requirements.md` and `architecture.md`.

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
