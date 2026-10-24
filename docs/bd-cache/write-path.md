# bd-cache write path (`write`, `drain`)

Status: approved (lead ruling, bc-lead, 2026-10-24, rulings R1-R4 on sprint
a-5; written by sprint a-5). Crate-local requirements
and decisions (domain code `CORE`) for the write path, indexed from
[`architecture.md`](architecture.md). They refine the repo-level ids in
[`../requirements.md`](../requirements.md) and
[`../architecture.md`](../architecture.md) and use the a-1 helpers
(REQ-CORE-0001..0004, ADR-CORE-0001, 0002). Code: `crates/bd-cache/src/write.rs`,
`crates/bd-cache/src/drain.rs`. Tests: `crates/bd-cache/tests/write_*.rs`,
`crates/bd-cache/tests/drain_*.rs`.

## REQ-CORE-0101: Write checks in a fixed order

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`write` runs these steps in order, and stops at the first that fails:

1. `Op::parse` (`BAD_OP`, `BAD_ARGS`).
2. Dedupe on the idempotency key, in any journal state.
3. `view::current_view`, or on a miss one `miss::fill` under
   `miss_fill_timeout` (`NOT_FOUND`, `BD_UNAVAILABLE`).
4. The precondition against that view (`PRECONDITION` with `unmet`).
5. `insert_entry`, with the scope taken from the view's first `phase-<x>` label.

An invalid write therefore starts no bd call. A write to a cached bead makes
no bd call at all. A retry of a write that is already journaled or applied
is a duplicate, even when its precondition no longer holds against the view
the first attempt changed.

**Refines:** REQ-BDC-0002, 0004, 0302, 0903
**Acceptance evidence:** `tests/write_path.rs`:
`test_write_is_visible_to_reads_but_cache_row_is_untouched`,
`test_precondition_checked_at_write_time`,
`test_duplicate_idem_key_is_one_entry_and_a_conflicting_reuse_is_refused`,
`test_retry_of_a_journaled_precondition_write_is_a_duplicate_not_a_precondition_error`,
`test_write_during_an_outage_journals_cached_beads_and_refuses_uncached_ones_in_time`,
`invalid_writes_are_refused_before_any_bd_call`.

## REQ-CORE-0102: One drain pass and its report

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`drain` takes the drain lock (`busy: true` when it is held elsewhere) and
holds it until the pass returns. It then checks the start gate, runs the
backend check and one batched pre-read of every bead with a runnable
entry, and takes each entry through the per-entry steps of REQ-BDC-0404
and the read-back of REQ-BDC-0405. Every pending entry the pass sees is
named in exactly one of `applied`, `retrying`, `escalated`, `deferred` and
`deferred_in_flight`. `outage` is the store's outage when the pass returns.
A pass with no pending entry, or with only held entries, makes no bd call.

**Refines:** REQ-BDC-0401, 0403, 0404, 0405, 0406
**Acceptance evidence:** `tests/drain_apply.rs`
(`every_pending_entry_is_in_exactly_one_report_list`,
`an_outage_defers_every_pending_entry_including_stale_held_ones` in
`tests/drain_outage.rs`,
`test_drain_writes_in_order_reads_back_and_marks_applied`,
`test_drain_pre_reads_all_pending_beads_in_one_call`, `test_drain_is_single_writer`),
`tests/drain_failures.rs`.

## REQ-CORE-0103: Held beads

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A bead is held, meaning its later entries are skipped for the rest of the
pass and stay pending with unchanged `attempts`, in two cases. It is held
from the start of the pass when its row is stale, and stays held across
passes until a store of the row clears the stale mark (a sync, a-6). It
is also held for the rest of the pass after one of its writes fails on a
reachable backend or its read-back fails. The pass takes entries in seq
order. A held entry the pass reaches is reported in `retrying`. A held
entry the pass never reaches is reported in `deferred`, like any unreached
entry. That happens when the pass stops first: the start gate passes, a
call is cut, or an outage stops it (ADR-CORE-0101).

**Refines:** REQ-BDC-0404, 0405
**Acceptance evidence:**
`test_transient_write_failure_stays_pending_and_holds_later_entries_of_that_bead_only`,
`test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`,
`a_held_beads_unreached_entry_is_deferred_when_the_gate_passes_or_a_call_is_cut`
(`tests/drain_budget.rs`),
`an_outage_defers_every_pending_entry_including_stale_held_ones`
(`tests/drain_outage.rs`).

## ADR-CORE-0101: Drain stop policy

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: ADR-BDC-0005 says an unreachable backend stops the drain without
counting an attempt, and only failures from a reachable backend count
towards `max_attempts`. It also says the Rust and Python implementations
share the journal and must apply the same rules. The `call` helper
(REQ-CORE-0001) classifies each failure as `Cut`, `Outage` (recorded) or
`Error`, and records an outage only for a backend that cannot be reached.
BD-CACHE v3 (baseline, Algorithms) counts more codes as unreachable at the
drain's backend check and pre-read (`UNREACHABLE`).

Decision (ruling R1): the drain follows v3 at the backend check and the
batched pre-read, and the `call` classification everywhere else:

| Where | `Cut` | `BD_UNAVAILABLE`, `BD_VERSION_FAILED`, `BD_SCHEMA_FAILED`, `BD_LIST_FAILED` | any other error from bd (e.g. `BD_CONTRACT`) | `Error` without a backend cause |
|---|---|---|---|---|
| backend check, pre-read | stop; every pending entry `deferred`; no outage | stop; outage recorded; every pending entry `deferred` | stop; `tracing` warning; every pending entry `retrying` | returned as the drain's error (a store failure) |
| write | entry `deferred_in_flight`; stop | `call` returns `BD_UNAVAILABLE` on a write as an `Error`: it counts an attempt, like the rest | `PRECONDITION` (bd guard, exit 13): escalate. Anything else: one attempt, hold the bead, escalate at `max_attempts` | returned |
| read-back | entry `deferred_in_flight` (still pending); stop | applied with the error, stale, bead held | applied with the error, stale, bead held | returned |

"Every pending entry" includes entries held for a stale row, so each
pending entry stays in exactly one report list (REQ-CORE-0102). No attempt
is counted in any row of the check and pre-read line. After a stop, every
entry not yet started is `deferred`.

The codes are matched on the typed `CacheError` variant (`is_unreachable`
in `drain.rs`), never on error text. An unreachable backend that `call` has
already recorded (`CallFailure::Outage`) is not recorded again. For the
other codes the drain records the outage itself through
`outage::note_unavailable`, which keeps the outage's start and stores the
latest error. As with any outage, only a list that answers clears it. The
drain tells a store failure from a backend failure by
`CacheError::backend_cause()`, because every error mapped from bd carries
its `BackendCause` (ADR-MODEL-0002).

Consequences (ruling R2):

- `DrainReport` has no error field; no a-1 amendment is made. An outage is
  visible through the report's `outage` and `status`. Any other check or
  pre-read failure is emitted as a `tracing` warning with fields `code` and
  `detail`. It goes through the re-export `sc_beads::logging::tracing`
  (ADR-BEADS-0001), so bd-cache gains no dependency.
- A long `BD_LIST_FAILED` or `BD_SCHEMA_FAILED` leaves writes pending
  rather than escalated, as an outage does in v3.
- A cut read-back leaves an entry pending whose write landed. The next
  pass's achieved check marks it applied without a second write
  (REQ-BDC-0402, 0005).

## ADR-CORE-0102: Write identity and dedupe

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Decision: two writes are the same when they have the same bead and an
equal `Op`, which includes its args. The precondition and the actor are not
part of a write's identity (REQ-BDC-0004: `(bead, op, args)`). Equal args
written differently, such as with other whitespace or key order, are the
same write. If `insert_entry` returns `IDEM_CONFLICT` because another
process took the key after the lookup, `write` re-reads the winning entry
by seq and dedupes against it: the same write is a duplicate, and a
different one keeps the conflict.

## ADR-CORE-0103: Escalation reasons and drain-time rows

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Decision:

- An escalation's `reason`, which the store keeps as `last_error`, is the
  typed error's display text, `BD_CACHE.<CODE>: <detail>`. This applies to
  `NOT_FOUND` (bead absent from the pre-read), `PRECONDITION` (unmet on the
  fresh row, or bd's guard refusal) and the retry cap. The retry cap reason
  is `retries exhausted after <n> attempts; last error <error>`.
- An unmet precondition at drain time stores the fresh row, forced and
  with membership unchanged (`upsert_rows(None, .., true)`), before it
  escalates. An achieved entry stores the fresh row through
  `apply_with_readback`.
- `dep_add` passes no guards to bd, because bd's `dep add` takes none, so
  its precondition is checked only against the fresh row.
- The start gate is checked first, before any bd call, and again before
  each entry, held or not. If it has passed at the start, every pending
  entry is `deferred` and no bd call is made. If it passes mid-pass, every
  entry not yet reached is `deferred`, including a held bead's later
  entries (REQ-CORE-0103).
