# bd-cache write path (`write`, `drain`)

Status: Draft (sprint a-5; awaiting lead ruling). Crate-local requirements
and decisions (domain code `CORE`) for the write path, indexed from
[`architecture.md`](architecture.md). They refine the repo-level ids in
[`../requirements.md`](../requirements.md) and
[`../architecture.md`](../architecture.md) and use the a-1 helpers
(REQ-CORE-0001..0004, ADR-CORE-0001, 0002). Code: `crates/bd-cache/src/write.rs`,
`crates/bd-cache/src/drain.rs`. Tests: `crates/bd-cache/tests/write_*.rs`,
`crates/bd-cache/tests/drain_*.rs`.

## REQ-CORE-0101: Write checks in a fixed order

**Status:** Draft (sprint a-5)

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

**Status:** Draft (sprint a-5)

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
`test_drain_writes_in_order_reads_back_and_marks_applied`,
`test_drain_pre_reads_all_pending_beads_in_one_call`, `test_drain_is_single_writer`),
`tests/drain_failures.rs`.

## REQ-CORE-0103: Held beads

**Status:** Draft (sprint a-5)

A bead is held, meaning its later entries are skipped for the rest of the
pass and stay pending with unchanged `attempts`, in two cases. It is held
from the start of the pass when its row is stale, and stays held across
passes until a store of the row clears the stale mark (a sync, a-6). It
is also held for the rest of the pass after one of its writes fails on a
reachable backend or its read-back fails. Held entries are reported in
`retrying`.

**Refines:** REQ-BDC-0404, 0405
**Acceptance evidence:**
`test_transient_write_failure_stays_pending_and_holds_later_entries_of_that_bead_only`,
`test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`.

## ADR-CORE-0101: Drain stop policy

**Status:** Draft (sprint a-5)

Context: ADR-BDC-0005 says an unreachable backend stops the drain without
counting an attempt, and only failures from a reachable backend count
towards `max_attempts`. The `call` helper (REQ-CORE-0001) already
classifies each failure as `Cut`, `Outage` (recorded) or `Error`.

Decision: the drain applies one rule to every `CallFailure`:

| Where | `Cut` | `Outage` | `Error` from a reachable bd | `Error` without a backend cause |
|---|---|---|---|---|
| backend check, pre-read | stop; every runnable entry `deferred` | stop; every runnable entry `deferred` | stop; every runnable entry `retrying`, no attempt | returned as the drain's error (a store failure) |
| write | entry `deferred_in_flight`; stop | entry `deferred`; stop (`call` never returns it for a write) | `PRECONDITION` (bd guard, exit 13): escalate. Anything else: one attempt, hold the bead, escalate at `max_attempts` | returned |
| read-back | entry `deferred_in_flight` (still pending); stop | applied with the error, stale, bead held | applied with the error, stale, bead held | returned |

After a stop, every entry not yet started is `deferred`. The drain tells a
store failure from a backend failure by `CacheError::backend_cause()`,
because every error mapped from bd carries its `BackendCause`
(ADR-MODEL-0002). It never inspects error text.

Consequences:

- A reachable check or pre-read failure (`BD_VERSION_FAILED`,
  `BD_SCHEMA_FAILED`, `BD_LIST_FAILED`, `BD_CONTRACT`) counts no attempt
  and records no outage, because `call` records only unreachable backends.
  BD-CACHE v3 (baseline, Algorithms) records an outage for
  `BD_VERSION_FAILED`, `BD_SCHEMA_FAILED` and `BD_LIST_FAILED` there.
  Phase a follows the a-1 classification and ADR-BDC-0005's definition of
  unreachable. **Open for the lead's ruling.**
- `DrainReport` has no field for the error that stopped such a pass, so
  the error is visible only through the call's effects. **Open for the
  lead's ruling** (an additive field is an a-1 amendment).
- A cut read-back leaves an entry pending whose write landed. The next
  pass's achieved check marks it applied without a second write
  (REQ-BDC-0402, 0005).

## ADR-CORE-0102: Write identity and dedupe

**Status:** Draft (sprint a-5)

Decision: two writes are the same when they have the same bead and an
equal `Op`, which includes its args. The precondition and the actor are not
part of a write's identity (REQ-BDC-0004: `(bead, op, args)`). Equal args
written differently, such as with other whitespace or key order, are the
same write. If `insert_entry` returns `IDEM_CONFLICT` because another
process took the key after the lookup, `write` re-reads the winning entry
by seq and dedupes against it: the same write is a duplicate, and a
different one keeps the conflict.

## ADR-CORE-0103: Escalation reasons and drain-time rows

**Status:** Draft (sprint a-5)

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
- The start gate is checked before the backend check and before each
  entry. A gate that has already passed defers the whole pass with no bd
  call.
