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
  owns: [REQ-BDC-0001, REQ-BDC-0008, REQ-BDC-0202, REQ-BDC-0203, REQ-BDC-0501, REQ-BDC-0502, REQ-BDC-0503, REQ-BDC-0504, REQ-BDC-0601, REQ-BDC-0602, REQ-BDC-0603, REQ-BDC-0902, REQ-BDC-0904, REQ-BDC-0907, REQ-BDC-0908, REQ-BDC-0909]
  constrained_by: [REQ-BDC-0105, REQ-BDC-0901, REQ-BDC-0903, NFR-BDC-0001, NFR-BDC-0004]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005]
owned_paths:
  - "crates/bd-cache/src/get.rs"
  - "crates/bd-cache/src/list.rs"
  - "crates/bd-cache/src/sync.rs"
  - "crates/bd-cache/src/ready.rs"
  - "crates/bd-cache/src/resolve.rs"
  - "crates/bd-cache/src/prune.rs"
  - "crates/bd-cache/src/status.rs"
  - "crates/bd-cache/tests/read_*.rs"
  - "crates/bd-cache/tests/sync_*.rs"
  - "docs/bd-cache/read-path.md"
---

# Sprint a-6 — core read path

## Outcome and closure

Fill the read-path modules of the `bd-cache` skeleton from a-1, tested only
against `FakeStore` and `FakeBackend`. Closes when its cases and planted-bug
checks pass. The `get` latency target is measured by a-7 on the real store.

## must_follow rationale

Consumes the `Cache` skeleton and its `call`, `outage`, `view`,
`backend_check` and `miss` helpers, `CacheStore`, `CacheBackend` and model types from a-1. It uses no
code from a-4 or a-5.

## Deliverables

1. `get` (REQ-BDC-0001, 0202): `view::current_view`, else `miss::fill`
   (stored with no membership), else `NOT_FOUND`; the reply carries `stale`
   and the scope's `last_ok_at`.
2. `list` (REQ-BDC-0203, 0902): rows of a scope with the overlay, `stale`
   per row, `last_ok_at`, and the `--status` / `--label` filters; an
   unconfigured scope is `SCOPE_UNKNOWN` naming the configured scopes.
3. `sync(scope, deadline)` (REQ-BDC-0501..0504, 0008): `backend_check::run`,
   cursor minted from the sync start time and sent as `last_ok_at - 2 min`,
   validate every row before storing any, newer-row guard, stale re-read
   with `force = true`,
   `SYNC_FAILED` recorded for a reachable backend; an outage records the
   outage instead; a cut sets `SyncReport.deferred` with no `SYNC_FAILED` and
   no alert change (ADR-BDC-0005).
4. Blocked set and `ready` (REQ-BDC-0904, ADR-BDC-0002): each sync of a scope
   ends with `list_blocked(scope.filter())` through `call` and
   `replace_blocked`; on failure the previous set is kept and
   `mark_blocked_stale`. `ready(scope)` returns the scope's open rows (with
   the overlay) minus the blocked set; `unknown` when the scope has never
   fetched one. `get`/`list` rows carry `blocked`.
5. `refresh(ids, timeout)` (REQ-BDC-0907): one `list_by_ids` with all
   statuses, stored with force, overlay applied; `entry(seq)` (REQ-BDC-0908);
   `get` takes an optional caller miss timeout. A `sync` of an unconfigured
   phase registers it as an on-demand scope (REQ-BDC-0909).
6. `resolve`, `prune` (with the out-of-scope sweep), `status`
   (REQ-BDC-0601..0603); `status` reports each scope's `last_ok_at`, stale
   rows and the outage (ADR-BDC-0005).
7. `docs/bd-cache/read-path.md`: crate ADRs for the read path
   (ADR-CORE-02xx).

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_miss_fetches_once_then_serves_from_cache`, `test_miss_on_unknown_bead_is_typed` (REQ-BDC-0001, 0202) | boundary:bd-cache |
| `test_contract_violation_is_typed_and_sync_stores_nothing` (REQ-BDC-0008) | boundary:bd-cache |

| `test_sync_is_incremental_and_never_replaces_a_newer_row`, `test_sync_failure_is_typed_and_recorded` (REQ-BDC-0501, 0502, 0504) | boundary:bd-cache |
| A stale row is re-read by the next sync and its bead's held entries become drainable (REQ-BDC-0503) | boundary:bd-cache |
| `test_prune_refuses_with_unapplied_writes`, `test_resolve_closes_only_escalated_entries_and_unblocks_prune`, `test_resolve_keeps_the_note_when_the_entry_had_no_error` (REQ-BDC-0601, 0602) | boundary:bd-cache |
| `refresh` reads fresh rows past a stale cache and returns them with the overlay; `entry(seq)` returns an entry in any state; `sync` then `list` of an unconfigured phase works (REQ-BDC-0907, 0908, 0909) | boundary:bd-cache |
| `list` of a scope never configured or synced is `SCOPE_UNKNOWN` naming the known scopes; a `get` miss outside every scope is stored with no membership and swept by `prune` once it has no open entry (REQ-BDC-0902) | boundary:bd-cache |
| `get` and `list` replies carry `stale` per row and the scope's `last_ok_at`; a cached `get` starts no backend call; `list --status` and `--label` filter (REQ-BDC-0001, 0203) | boundary:bd-cache |
| `test_sync_records_an_outage_and_a_passing_sync_clears_it`; a sync that meets an outage records it and no `SYNC_FAILED`; a cut sync reports `deferred` and changes no alert | ADR-BDC-0005 |
| A failed `list_blocked` keeps the previous set, marked stale | ADR-BDC-0002 |
| After a blocker (cached or outside every scope) is closed in the backend, one sync makes its depender ready; after it is reopened, one sync makes the depender blocked again; a child of a blocked parent is not ready (REQ-BDC-0904) | ADR-BDC-0002 |
| A scope that has never fetched its blocked set reports `unknown` and no ready beads | ADR-BDC-0002 |
| Planted bugs: prune ignores escalated, resolve accepts any state each fail a test | boundary:bd-cache |

## This sprint does not close

Scheduled sync with the real store and backend, and escalation delivery
(a-7).

## Required validation

Crate tests and lints; `cargo build --workspace`.
