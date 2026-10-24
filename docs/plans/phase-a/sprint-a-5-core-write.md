---
id: a-5
phase: a
status: planned
closure_type: boundary
target_boundary: bd-cache (write path)
track: core
wave: 2
branch: sprint/a-5-core-write
worktree: sprint/a-5-core-write
recommended_agent: rust-developer
recommended_model: opus
must_follow: [a-1]
parallel_safe: [a-2, a-3, a-4, a-6]
requirements:
  owns: [REQ-BDC-0002, REQ-BDC-0004, REQ-BDC-0005, REQ-BDC-0302, REQ-BDC-0401, REQ-BDC-0402, REQ-BDC-0403, REQ-BDC-0404, REQ-BDC-0405, REQ-BDC-0406]
  constrained_by: [REQ-BDC-0003, REQ-BDC-0105, REQ-BDC-0202, REQ-BDC-0903, NFR-BDC-0004]
adrs: [ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005]
owned_paths:
  - "crates/bd-cache/src/write.rs"
  - "crates/bd-cache/src/drain.rs"
  - "crates/bd-cache/src/outage.rs"
  - "crates/bd-cache/tests/write_*.rs"
  - "crates/bd-cache/tests/drain_*.rs"
  - "docs/bd-cache/write-path.md"
---

# Sprint a-5 — core write path

## Outcome and closure

Fill the write-path modules of the `bd-cache` skeleton from a-1, tested only
against `FakeStore` and `FakeBackend`. Closes when its cases and planted-bug
checks pass.

## must_follow rationale

Consumes the `Cache` skeleton and its `view`, `backend_check` and `miss`
helpers, `CacheStore`, `DrainLock`, `CacheBackend` and model types from a-1.
It uses no code from a-4 or a-6.

## Deliverables

1. `write` (REQ-BDC-0002, 0302, 0004): validate, idempotency (duplicate and
   conflict), write-time precondition against `view::current_view`, insert a
   pending entry; a cache miss goes through `miss::fill`.
2. `drain` (REQ-BDC-0401..0406): `DrainLock::try_acquire` (`busy: true`), the
   start gate and hard deadline with `deferred` and `deferred_in_flight`,
   `backend_check::run`, one batched pre-read, per-entry steps (absent,
   achieved, precondition, command, exit 13, retry cap of 5, per-bead hold),
   read-back with `apply_with_readback` and the stale mark, the drain
   report. Beads whose row is stale are skipped until a sync clears it. A
   close or reopen that lands calls `mark_blocker_changed` (ADR-BDC-0002).
3. `outage` (ADR-BDC-0005): `BeadsError::Unavailable` stops the drain at the
   first such failure without counting attempts and records the outage
   (`set_outage`); the next successful call clears it. Only failures from a
   reachable backend count toward the cap.
4. `docs/bd-cache/write-path.md`: crate ADRs for the write path
   (ADR-CORE-01xx).

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_write_is_visible_to_reads_but_cache_row_is_untouched`, `test_precondition_checked_at_write_time`, `test_duplicate_idem_key_is_one_entry_and_a_conflicting_reuse_is_refused`, `test_dep_add_overlays_then_lands_as_a_real_edge` (REQ-BDC-0002, 0302, 0004) | boundary:bd-cache |
| `test_drain_writes_in_order_reads_back_and_marks_applied`, `test_drain_pre_reads_all_pending_beads_in_one_call`, `test_failed_pre_read_retries_every_entry` (REQ-BDC-0403, 0405) | boundary:bd-cache |
| `test_transient_write_failure_stays_pending_and_holds_later_entries_of_that_bead_only`, `test_retries_exhausted_escalate_and_never_silently_drop`, `test_concurrent_backend_change_escalates_instead_of_overwriting`, `test_bd_guard_refusal_exit_13_escalates`, `test_drain_on_absent_bead_escalates_not_found` (REQ-BDC-0404) | boundary:bd-cache |
| `test_write_that_landed_before_a_crash_is_not_repeated` (REQ-BDC-0005) | boundary:bd-cache |
| `test_drain_is_single_writer`, `test_drain_deadline_defers_unstarted_entries_without_attempts` (REQ-BDC-0401, 0402) | boundary:bd-cache |
| A failed read-back marks the entry applied with the error and sets the stale mark; with `FakeStore` holding that row stale, later drains skip the bead's entries until the stale mark is cleared (REQ-BDC-0405) | boundary:bd-cache |
| An entry whose call is cut by the hard deadline stays pending with unchanged `attempts`, is reported in `deferred_in_flight`, records no outage, and a write that landed is found achieved by the next drain (REQ-BDC-0402) | boundary:bd-cache |
| With `FakeBackend` returning `Unavailable` for N drains, entries stay pending with unchanged `attempts` and the outage is recorded, then cleared on recovery; a reachable-backend failure still escalates at 5 | ADR-BDC-0005 |
| A landed close or reopen marks its bead as a changed blocker | ADR-BDC-0002 |
| Planted bugs: no write precondition, no drain precondition, no per-bead hold, no achieved check, no retry cap, no stale flag, no drain lock, per-entry pre-read, no not-found escalation, deferred not reported, drain without deadline each fail a test | boundary:bd-cache |

## This sprint does not close

Sync re-reading stale rows and the depender refresh (a-6); the scheduled
drain with the real store and backend (a-7).

## Required validation

Crate tests and lints; `cargo build --workspace`.
