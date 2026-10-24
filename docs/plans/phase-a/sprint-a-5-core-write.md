---
id: a-5
phase: a
status: in-review
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
  owns: [REQ-BDC-0002, REQ-BDC-0004, REQ-BDC-0005, REQ-BDC-0302, REQ-BDC-0401, REQ-BDC-0402, REQ-BDC-0403, REQ-BDC-0404, REQ-BDC-0405, REQ-BDC-0406, REQ-CORE-0101, REQ-CORE-0102, REQ-CORE-0103]
  constrained_by: [REQ-BDC-0003, REQ-BDC-0105, REQ-BDC-0202, REQ-BDC-0301, REQ-BDC-0305, REQ-BDC-0803, REQ-BDC-0902, REQ-BDC-0903, NFR-BDC-0004, NFR-BDC-0005, REQ-CORE-0001, REQ-CORE-0002, REQ-CORE-0003, REQ-CORE-0004]
adrs: [ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005, ADR-MODEL-0002, ADR-MODEL-0003, ADR-CORE-0001, ADR-CORE-0002, ADR-CORE-0101, ADR-CORE-0102, ADR-CORE-0103]
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

## Requirement scope notes

`constrained_by` lists ids whose behaviour another sprint owns. a-5 relies
on each of them or applies it:

| Id | What a-5 does | Owner of the rest |
|---|---|---|
| REQ-BDC-0003 | the drain ends each entry applied, escalated or pending (never dropped, never retried past the cap) | a-6 (`resolve`), a-7 (announcement) |
| REQ-BDC-0105, REQ-CORE-0003 | calls `backend_check::run` at the start of every non-empty drain | a-1 (helper) |
| REQ-BDC-0202, REQ-CORE-0004 | `write` uses `view::current_view` and `miss::fill` | a-1 (helpers), a-6 (`get`) |
| REQ-BDC-0301, 0305 | uses `Op::parse`, `overlay` through `replay`, and `achieved` | a-1 (model) |
| REQ-BDC-0803 | returns and escalates with the typed codes | a-1 (`CacheError`) |
| REQ-BDC-0902 | a write's miss fill stores the row with no membership; the read-back keeps membership | a-6 (`prune` sweep) |
| REQ-BDC-0903 | writes to cached beads make no bd call; an uncached write is `BD_UNAVAILABLE` within `miss_fill_timeout` | a-6 (reads) |
| NFR-BDC-0004, REQ-CORE-0001, 0002 | every drain bd call goes through `call` under the hard deadline | a-1 (helper, cap) |
| NFR-BDC-0005 | planted bugs for the write path (below) | a-1 (policy, fakes) |

REQ-BDC-0405's store side (one transaction, stale mark) is a-4's
`apply_with_readback`; a-5 calls it. Clearing the stale mark (the sync's
stale re-read, REQ-BDC-0503) is a-6's. a-5's tests clear it with a store
upsert, as a sync would.

## Acceptance evidence

Every test is in `crates/bd-cache/tests/`. Rows without a named test
in the criteria table map as follows:

| Criterion | Tests |
|---|---|
| Retry of a journaled or applied precondition write is `duplicate: true` | `write_path.rs`: `test_retry_of_a_journaled_precondition_write_is_a_duplicate_not_a_precondition_error` |
| Outage: cached write journaled, uncached `BD_UNAVAILABLE` within `miss_fill_timeout` | `write_path.rs`: `test_write_during_an_outage_journals_cached_beads_and_refuses_uncached_ones_in_time` |
| Failed read-back: applied with the error, stale, later drains skip the bead until the mark clears | `drain_failures.rs`: `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync` |
| Cut entry: pending, unchanged attempts, `deferred_in_flight`, no outage, found achieved next drain | `drain_budget.rs`: `a_write_cut_by_the_hard_deadline_is_deferred_in_flight_and_found_achieved_next_drain`, `a_read_back_cut_by_the_hard_deadline_leaves_the_landed_write_pending`, `a_cut_backend_check_defers_the_whole_drain`, `a_cut_pre_read_defers_the_whole_drain` |
| Unreachable at check or pre-read for N drains, then recovery; reachable write failures count and escalate at 5 | `drain_outage.rs`: `test_unreachable_backend_is_an_outage_that_counts_no_attempt` (7 drains, alternating check and pre-read), `a_pre_read_timing_out_at_the_call_cap_is_an_outage_not_a_cut`; `drain_failures.rs`: `test_a_reachable_backend_refusing_the_write_still_counts_attempts`, `test_retries_exhausted_escalate_and_never_silently_drop` |

REQ-BDC-0403's listed evidence `test_failed_pre_read_retries_every_entry`
is in `drain_failures.rs`. It covers a reachable pre-read failure:
entries `retrying`, no attempt and no outage (ADR-CORE-0101).

The contract suites in `bd_cache_model::testing` and `sc_beads::testing`
test `CacheStore` / `DrainLock` and `CacheBackend` implementations. a-5 adds
none, so it has no new suite to run. The existing suites pass in the
workspace run.

## Decisions for the lead

The decisions are recorded as Draft entries in `docs/bd-cache/write-path.md`
(REQ-CORE-0101..0103, ADR-CORE-0101..0103) and need a ruling:

1. **Reachable check or pre-read failure** (ADR-CORE-0101). The pass stops
   with no attempt and no outage, and its entries are reported in
   `retrying`. This follows a-1's `call` classification and ADR-BDC-0005.
   BD-CACHE v3 instead treats `BD_VERSION_FAILED`, `BD_SCHEMA_FAILED` and
   `BD_LIST_FAILED` at those steps as unreachable and records an outage.
2. **No report field for that error** (ADR-CORE-0101). `DrainReport`
   cannot carry the error that stopped such a pass. An additive field
   would be an a-1 amendment.
3. **Held entries are reported in `retrying`** (REQ-CORE-0102, 0103). This
   covers stale beads and beads with a failed write in the pass, so every
   pending entry is in exactly one report list.

No a-1 contract is changed. `write` and `drain` keep the delegated
signatures in `lib.rs`.

## Evidence: planted bugs (NFR-BDC-0005)

Run on commit `b780b704f6c40855110473b17fc35280996d3900`, the last commit
that changes code (later commits change this document only). The script
applied each mutation to the working tree on its own, ran the whole suite
and restored the file:

```text
cargo test --workspace --all-features --no-fail-fast
git checkout -- <file>
```

The unmutated tree passes the same command. After the run, `git diff --
crates` was empty.

| Planted bug | Mutation | File | Exit | Compile error | Failing tests |
|---|---|---|---|---|---|
| no write precondition | `write` skips the `unmet` check. | `crates/bd-cache/src/write.rs` | 101 | no | `test_precondition_checked_at_write_time` |
| no drain precondition | the drain skips the fresh-row `unmet` check. | `crates/bd-cache/src/drain.rs` | 101 | no | `test_concurrent_backend_change_escalates_instead_of_overwriting` |
| no per-bead hold | a failed write does not hold its bead. | `crates/bd-cache/src/drain.rs` | 101 | no | `every_pending_entry_is_in_exactly_one_report_list`, `test_transient_write_failure_stays_pending_and_holds_later_entries_of_that_bead_only` |
| no achieved check | the drain skips `Op::achieved`. | `crates/bd-cache/src/drain.rs` | 101 | no | `a_read_back_cut_by_the_hard_deadline_leaves_the_landed_write_pending`, `a_write_cut_by_the_hard_deadline_is_deferred_in_flight_and_found_achieved_next_drain`, `test_write_that_landed_before_a_crash_is_not_repeated` |
| no retry cap | the drain never escalates at `max_attempts`. | `crates/bd-cache/src/drain.rs` | 101 | no | `test_retries_exhausted_escalate_and_never_silently_drop`, `the_retry_cap_is_the_configured_max_attempts` |
| no stale flag | a failed read-back stores the pre-read row, so no stale mark is set. | `crates/bd-cache/src/drain.rs` | 101 | no | `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync` |
| stale rows not held | the drain ignores `stale_rows`. | `crates/bd-cache/src/drain.rs` | 101 | no | `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync` |
| no drain lock | the drain does not take the drain lock. | `crates/bd-cache/src/drain.rs` | 101 | no | `test_drain_is_single_writer` |
| per-entry pre-read | the batched pre-read is empty; each entry lists its own bead. | `crates/bd-cache/src/drain.rs` | 101 | no | `a_cut_pre_read_defers_the_whole_drain`, `a_pre_read_timing_out_at_the_call_cap_is_an_outage_not_a_cut`, `a_read_back_cut_by_the_hard_deadline_leaves_the_landed_write_pending`, `test_drain_pre_reads_all_pending_beads_in_one_call`, `test_drain_writes_in_order_reads_back_and_marks_applied`, `test_failed_pre_read_retries_every_entry`, `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`, `test_unreachable_backend_is_an_outage_that_counts_no_attempt`, `test_write_that_landed_before_a_crash_is_not_repeated` |
| no not-found escalation | a bead absent from the pre-read is left pending. | `crates/bd-cache/src/drain.rs` | 101 | no | `test_drain_on_absent_bead_escalates_not_found` |
| deferred not reported | entries skipped at the start gate are not put in `deferred`. | `crates/bd-cache/src/drain.rs` | 101 | no | `test_drain_deadline_defers_unstarted_entries_without_attempts` |
| drain without deadline | the start gate never passes. | `crates/bd-cache/src/drain.rs` | 101 | no | `a_drain_that_calls_no_bd_still_reports_the_standing_outage`, `a_start_gate_already_passed_defers_everything_without_a_bd_call`, `test_drain_deadline_defers_unstarted_entries_without_attempts` |
| dedupe after the precondition | `write` checks the precondition before the idempotency key. | `crates/bd-cache/src/write.rs` | 101 | no | `test_retry_of_a_journaled_precondition_write_is_a_duplicate_not_a_precondition_error` |

Every bug fails at least one test, and none of them by a compile error.
The table has the twelve bugs from the acceptance criteria plus one extra,
`stale rows not held`, which plants the read side of the stale rule.
