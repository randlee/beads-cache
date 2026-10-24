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

Consumes the `Cache` skeleton and its `call`, `outage`, `view`,
`backend_check` and `miss` helpers, `CacheStore`, `DrainLock`, `CacheBackend` and model types from a-1.
It uses no code from a-4 or a-6.

## Deliverables

1. `write` (REQ-BDC-0002, 0302, 0004), in this order: `Op::parse`;
   `entry_by_key` (an equal entry in any state returns `duplicate: true`, a
   different one `IDEM_CONFLICT`); `view::current_view`, or `miss::fill` on a
   miss; the precondition against that view; `insert_entry` with the scope
   from `ScopeId::from_labels`. During an outage a write to an uncached bead
   returns `BD_UNAVAILABLE` (REQ-BDC-0903 covers cached beads).
2. `drain(&DrainBudget)` (REQ-BDC-0401..0406): `DrainLock::try_acquire`
   (`busy: true`), the start gate and hard deadline with `deferred` and
   `deferred_in_flight`, every backend call through `call`,
   `backend_check::run` (a cut check defers the whole drain), one batched pre-read, per-entry steps (absent,
   achieved, precondition, command, exit 13, retry cap of 5, per-bead hold),
   read-back with `apply_with_readback` and the stale mark, the drain
   report. Beads whose row is stale are skipped until a sync clears it. 
3. Outage stop policy in `drain.rs` (ADR-BDC-0005): a `CallOutcome::Outage`
   stops the drain without counting an attempt (the `call` helper records
   it); `Cut` leaves the entry in `deferred_in_flight`. Only errors from a
   reachable backend count toward the cap.
4. `docs/bd-cache/write-path.md`: crate ADRs for the write path
   (ADR-CORE-01xx).

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_write_is_visible_to_reads_but_cache_row_is_untouched`, `test_precondition_checked_at_write_time`, `test_duplicate_idem_key_is_one_entry_and_a_conflicting_reuse_is_refused`, `test_dep_add_overlays_then_lands_as_a_real_edge` (REQ-BDC-0002, 0302, 0004) | boundary:bd-cache |
| `test_drain_writes_in_order_reads_back_and_marks_applied`, `test_drain_pre_reads_all_pending_beads_in_one_call`, `test_unreachable_backend_is_an_outage_that_counts_no_attempt`, `test_a_reachable_backend_refusing_the_write_still_counts_attempts` (REQ-BDC-0403, 0404, 0405) | boundary:bd-cache |
| `test_transient_write_failure_stays_pending_and_holds_later_entries_of_that_bead_only`, `test_retries_exhausted_escalate_and_never_silently_drop`, `test_concurrent_backend_change_escalates_instead_of_overwriting`, `test_bd_guard_refusal_exit_13_escalates`, `test_drain_on_absent_bead_escalates_not_found` (REQ-BDC-0404) | boundary:bd-cache |
| `test_write_that_landed_before_a_crash_is_not_repeated` (REQ-BDC-0005) | boundary:bd-cache |
| `test_drain_is_single_writer`, `test_drain_deadline_defers_unstarted_entries_without_attempts` (REQ-BDC-0401, 0402) | boundary:bd-cache |
| A retry of a precondition-carrying write whose first attempt is already journaled (or applied) returns `duplicate: true`, not `PRECONDITION` (REQ-BDC-0004) | boundary:bd-cache |
| During an outage a write to a cached bead is journaled; a write to an uncached bead returns `BD_UNAVAILABLE` within `miss_fill_timeout` | ADR-BDC-0005 |
| A failed read-back marks the entry applied with the error and sets the stale mark; with `FakeStore` holding that row stale, later drains skip the bead's entries until the stale mark is cleared (REQ-BDC-0405) | boundary:bd-cache |
| An entry whose call is cut by the hard deadline stays pending with unchanged `attempts`, is reported in `deferred_in_flight`, records no outage, and a write that landed is found achieved by the next drain (REQ-BDC-0402) | boundary:bd-cache |
| With `FakeBackend` unreachable at the check or pre-read for N drains, entries stay pending with unchanged `attempts` and the outage is recorded, then cleared when a list answers; a write that fails or times out on a reachable backend counts attempts and escalates at 5 | ADR-BDC-0005 |

| Planted bugs: no write precondition, no drain precondition, no per-bead hold, no achieved check, no retry cap, no stale flag, no drain lock, per-entry pre-read, no not-found escalation, deferred not reported, drain without deadline, dedupe after the precondition each fail a test | boundary:bd-cache |

## This sprint does not close

Sync re-reading stale rows and the ready base (a-6); the scheduled
drain with the real store and backend (a-7).

## Required validation

Crate tests and lints; `cargo build --workspace`.
