---
id: a-6
phase: a
status: planned
closure_type: boundary
target_boundary: bd-cache (read path)
track: core
wave: 2
branch: sprint/a-6-core-read
worktree: sprint/a-6-core-read
recommended_agent: rust-developer
recommended_model: opus
must_follow: [a-1]
parallel_safe: [a-2, a-3, a-4, a-5]
requirements:
  owns: [REQ-BDC-0001, REQ-BDC-0008, REQ-BDC-0105, REQ-BDC-0202, REQ-BDC-0203, REQ-BDC-0501, REQ-BDC-0502, REQ-BDC-0503, REQ-BDC-0504, REQ-BDC-0601, REQ-BDC-0602, REQ-BDC-0603, REQ-BDC-0902, REQ-BDC-0904, NFR-BDC-0001]
  constrained_by: [REQ-BDC-0903, REQ-BDC-0901, NFR-BDC-0004]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005]
owned_paths:
  - "crates/bd-cache/src/get.rs"
  - "crates/bd-cache/src/list.rs"
  - "crates/bd-cache/src/sync.rs"
  - "crates/bd-cache/src/refresh.rs"
  - "crates/bd-cache/src/resolve.rs"
  - "crates/bd-cache/src/prune.rs"
  - "crates/bd-cache/src/status.rs"
  - "crates/bd-cache/tests/read_*.rs"
  - "crates/bd-cache/tests/sync_*.rs"
  - "crates/bd-cache/benches/**"
---

# Sprint a-6 — core read path

## Outcome and closure

Fill the read-path modules of the `bd-cache` skeleton from a-1, tested only
against `FakeStore` and `FakeBackend`. Closes when its cases and planted-bug
checks pass and the in-process `get` meets its latency target.

## must_follow rationale

Consumes the `Cache` skeleton, `CacheStore`, `BeadsBackend` and model types
from a-1. It uses no code from a-4 or a-5.

## Deliverables

1. `get` (REQ-BDC-0001, 0202): cached row with pending entries replayed in
   `seq` order (escalated skipped), miss-once fetch, `NOT_FOUND`; out of
   every scope, a `get` may fetch once.
2. `list` (REQ-BDC-0203, 0902): rows of a scope with the overlay; a scope
   that is not configured is refused with the configured scopes named.
3. `sync` per scope (REQ-BDC-0105, 0501..0504, 0008): backend check through
   `record_fingerprint` (rebuilt, model_current), incremental cursor with the
   2-minute overlap, validate every row before storing any, newer-row guard,
   stale re-read, `SYNC_FAILED` recorded.
4. `refresh` (REQ-BDC-0904, ADR-BDC-0002): after the scope lists, re-read only
   the cached dependers of changed blockers (`take_changed_blockers`, plus
   beads whose status changed in this sync) and of outside blockers returned
   by `list_changed`, to a fixpoint over parent-child.
5. `resolve`, `prune`, `status` (REQ-BDC-0601..0603); `status` also reports
   each scope's `last_ok_at`, stale rows and the outage state
   (ADR-BDC-0005).
6. Benchmark for in-process `get` (NFR-BDC-0001).

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_miss_fetches_once_then_serves_from_cache`, `test_miss_on_unknown_bead_is_typed` (REQ-BDC-0001, 0202) | boundary:bd-cache |
| `test_contract_violation_is_typed_and_sync_stores_nothing` (REQ-BDC-0008) | boundary:bd-cache |
| `test_schema_change_rebuilds_rows_and_keeps_the_journal`, `test_model_current_when_live_schema_matches_pin`, `test_first_backend_check_records_schema_without_dropping_cached_rows` (REQ-BDC-0105) | boundary:bd-cache |
| `test_sync_is_incremental_and_never_replaces_a_newer_row`, `test_sync_failure_is_typed_and_recorded` (REQ-BDC-0501, 0502, 0504) | boundary:bd-cache |
| A stale row is re-read by the next sync and its bead's held entries become drainable (REQ-BDC-0503) | boundary:bd-cache |
| `test_prune_refuses_with_unapplied_writes`, `test_resolve_closes_only_escalated_entries_and_unblocks_prune`, `test_resolve_keeps_the_note_when_the_entry_had_no_error` (REQ-BDC-0601, 0602) | boundary:bd-cache |
| `list` of an unconfigured scope is refused, naming the configured scopes (REQ-BDC-0902) | boundary:bd-cache |
| Closing or reopening a blocker, cached or outside, flips its cached dependers' `is_blocked` after one sync, and the sync re-reads only those dependers (REQ-BDC-0904) | ADR-BDC-0002 |
| In-process cached `get` under 1 ms on the benchmark (NFR-BDC-0001) | boundary:bd-cache |
| Planted bugs: no overlay, skip rebuild, rebuild on first check, prune ignores escalated, resolve accepts any state each fail a test | boundary:bd-cache |

## This sprint does not close

Scheduled sync with the real store and backend, and escalation delivery
(a-7).

## Required validation

Crate tests, lints and the `get` benchmark; `cargo build --workspace`.
