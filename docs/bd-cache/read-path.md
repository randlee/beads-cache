# bd-cache Read Path

Status: approved (REQ-CORE-0201..0207 and ADR-CORE-0201..0206, lead
ruling, bc-lead, 2026-10-24; ADR-CORE-0206 is the a-7 amendment). Crate-local requirements and decisions (domain
code `CORE`) for the read path of `bd-cache`: `get`, `entry`, `refresh`,
`list`, `ready`, `sync`, `resolve`, `prune` and `status`. Indexed from
[`architecture.md`](architecture.md); the shared helpers they call (`call`,
`outage`, `view`, `backend_check`, `miss`) are REQ-CORE-0001..0004 and
ADR-CORE-0001..0002 there. Repo-level context:
[`../requirements.md`](../requirements.md),
[`../architecture.md`](../architecture.md).

| File | Contents |
|---|---|
| `src/get.rs` | `get`, `cached` (a-7: no bd call, no miss fill), `entry` |
| `src/list.rs` | `list` |
| `src/refresh.rs` | `refresh`, `check_requested` |
| `src/sync.rs` | `sync` |
| `src/ready.rs` | `ready`, the adjustment engine (`adjusted`), `fetch_base`, `row_view`, `blocked_state` (a-7: the bead's blocked state without adjusting the whole base), `scope_for`, `require_known` |
| `src/resolve.rs`, `src/prune.rs`, `src/status.rs` | store delegations |

Tests: `tests/read_get.rs`, `tests/read_ready.rs`, `tests/read_journal.rs`,
`tests/sync_rows.rs`, `tests/sync_contract.rs`.

## Requirements

### REQ-CORE-0201: get and entry

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`get(id, miss_timeout)` answers a cached bead from the store with its
pending entries replayed and starts no backend call. A miss makes one
`miss::fill` under `miss_timeout` (else `miss_fill_timeout`) and stores the
row with no scope membership; a bead bd does not know is `NOT_FOUND`. The
reply carries `stale`, `blocked` from the bead's scope's adjusted base, the
scope's `last_ok_at`, the number of pending entries and the outage.
`entry(seq)` returns the entry in any state, `NOT_FOUND` for an unknown seq.

**Refines:** REQ-BDC-0001, 0202, 0908, 0902; ADR-BDC-0005
**Acceptance evidence:** `test_miss_fetches_once_then_serves_from_cache`,
`test_miss_on_unknown_bead_is_typed`, `a_miss_is_bounded_by_the_callers_timeout`,
`get_and_list_replies_carry_stale_per_row_and_the_scopes_last_ok_at`,
`ready_replies_carry_the_scopes_last_ok_at`,
`get_replays_pending_entries_and_counts_them`,
`replies_carry_the_outage_while_one_holds`,
`entry_returns_an_entry_in_any_state` (`tests/read_get.rs`).

### REQ-CORE-0202: list

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`list(scope, status, label)` returns the scope's cached rows by id, each
with its pending entries replayed and filtered on that view, with `stale`,
`blocked`, the scope's `last_ok_at` and the outage. A scope never
configured or synced is `SCOPE_UNKNOWN` naming the known scopes. Starts no
backend call.

**Refines:** REQ-BDC-0203, 0902, 0909
**Acceptance evidence:** `list_filters_by_status_and_label_on_the_view`,
`list_of_a_scope_never_configured_or_synced_is_scope_unknown_naming_the_known_scopes`,
`sync_then_list_of_an_unconfigured_phase_works`.

### REQ-CORE-0203: refresh

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`refresh(ids, timeout)` makes one `list_by_ids` of all the ids (all
statuses) through `call(Refresh)` under `timeout`, checks the answer as a
whole, stores it forced with membership unchanged, and returns the views in
request order without repeats; ids bd does not know are absent. A cut or an
unreachable backend is `BD_UNAVAILABLE`; an empty `ids` makes no call.

**Refines:** REQ-BDC-0907, 0008
**Acceptance evidence:** `refresh_reads_fresh_rows_past_a_stale_cache_and_returns_them_with_the_overlay`,
`refresh_is_bounded_by_its_timeout_and_an_empty_refresh_calls_nothing`,
`a_bad_row_in_a_refresh_or_miss_fill_stores_nothing`.

### REQ-CORE-0204: sync

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`sync(scope, d)` registers the scope, runs `backend_check::run`, lists the
scope with the cursor `last_ok_at - 2 min` once a sync has succeeded,
checks the answer as a whole and stores it unforced, re-reads stale rows
forced, refreshes the ready base, and records `last_ok_at` as the sync's
start time. Failures follow ADR-CORE-0202.

**Refines:** REQ-BDC-0501, 0502, 0503, 0504, 0008, 0909; ADR-BDC-0005
**Acceptance evidence:** `tests/sync_rows.rs`, `tests/sync_contract.rs`.

### REQ-CORE-0205: Ready base and local adjustment

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Each sync stores the scope's base from `list_blocked` and `list_ready`, with
the instant taken before the first of the two; a failed or cut fetch keeps
the previous base, marked stale. `ready(scope, None)` makes no backend
call: it applies the rules of ADR-CORE-0201 to the base and reports
`unknown` when there is none. Every `ready` reply carries the scope's
`last_ok_at` (`sync_state(s).last_ok_at`; only a sync moves it, a fresh
answer does not). `get`, `list` and `ready` rows take `blocked`
from the same adjusted base; ready ids with no cached row are only in
`missing_rows`.

**Refines:** REQ-BDC-0904, 0911; ADR-BDC-0002
**Acceptance evidence:** the `rule_*` tests and
`with_bd_unreachable_a_journaled_close_of_the_only_blocker_makes_the_depender_ready_at_once`,
`only_entries_pending_or_applied_at_or_after_fetched_at_adjust_the_base`,
`a_scope_with_no_ready_base_reports_unknown`,
`rows_carry_blocked_from_the_adjusted_base_and_a_ready_id_without_a_row_is_only_in_missing_rows`
(`tests/read_ready.rs`); `each_sync_stores_the_ready_base_fetched_after_the_rows`,
`a_failed_list_blocked_or_list_ready_keeps_the_previous_base_marked_stale`,
`after_a_cached_blocker_closes_in_bd_one_sync_makes_its_depender_ready_and_reopen_blocks_it`,
`after_a_blocker_outside_every_scope_closes_in_bd_one_sync_makes_its_depender_ready`,
`a_child_of_a_blocked_parent_is_not_ready` (`tests/sync_rows.rs`).

### REQ-CORE-0206: Bounded fresh ready

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`ready(scope, Some(limit))` makes both base calls under one deadline
`limit` from the call. If both answer, the base is replaced and the reply is
`Fresh { fetched_at }` with the new base's fetch instant, still adjusted
by the journal; if not, the reply is the cached
adjusted answer (or `unknown`) with `read_fallback` naming the failure, and
it returns by the limit. An expired limit is a cut and records no outage.

**Refines:** REQ-BDC-0912; ADR-BDC-0002, 0005
**Acceptance evidence:** `bounded_fresh_ready_returns_bds_answer_and_refreshes_the_base`,
`bounded_fresh_ready_falls_back_to_the_cached_answer_at_the_limit_without_an_outage`,
`bounded_fresh_ready_on_an_unreachable_backend_falls_back_and_records_the_outage`,
`no_base_and_no_fresh_answer_is_unknown`.

### REQ-CORE-0207: resolve, prune, status and the announcement state

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`resolve`, `prune` and `status` apply the store's rules at the clock's
time (ADR-CORE-0205); the announcement passthroughs on `Cache` (a-1's
`lib.rs`) reach the store unchanged.

**Refines:** REQ-BDC-0601, 0602, 0603, 0902
**Acceptance evidence:** `tests/read_journal.rs`;
`a_get_miss_outside_every_scope_is_stored_with_no_membership_and_swept_by_prune_once_it_has_no_open_entry`.

## ADR-CORE-0201: Ready adjustment rules

**Status:** Approved (lead ruling, bc-lead, 2026-10-24). Refines ADR-BDC-0002,
decision 2; it does not amend it. ADR-BDC-0002 was clarified to match
(reopen from `closed` to any other status; children only through the
parent-block rule; lead ruling, bc-lead, 2026-10-24, `integrate/phase-a`
at `60e630f`).

Context: ADR-BDC-0002 makes bd's `bd blocked` / `bd ready` answer the base
and lets reads adjust it for our journal without a bd call. It states the
rules in prose; the edge cases (what "was closed" means for an entry already
applied, children of a reopened bead, a bead that becomes unblocked but is
not `open`) need one precise reading.

Decision: entries are every `Pending` entry plus every entry applied at or
after `fetched_at`, in seq order (escalated and resolved entries never
adjust). The engine (`ready::adjusted`) keeps the base's blocked map and
ready list and applies, for an entry on bead X:

| Op and dep type | Effect | Test |
|---|---|---|
| `close` | X leaves `ready` and the blocked map; X is dropped from every `blocked_by`; each bead whose list empties is unblocked (below) | `with_bd_unreachable_a_journaled_close_of_the_only_blocker_makes_the_depender_ready_at_once` |
| `update` to `closed` | as `close` | `rule_update_to_closed_is_a_close` |
| `update` to another status S | X leaves `ready`, unless X's prior status is known to be S | `rule_close_or_status_change_of_the_bead_itself_leaves_ready` |
| `update` to S ≠ `closed` when X's prior status is `closed` or unknown (reopen) | X joins `blocked_by` of every bead with a `blocks`, `conditional-blocks` or `waits-for` edge onto X; X's own outgoing edges are re-applied as `dep_add`s (its blockers come back) | `rule_reopen_from_closed_blocks_the_dependers_again`, `rule_reopen_brings_back_the_beads_own_blockers` |
| `dep_add` X → T, `blocks`, `conditional-blocks` or `waits-for` | T joins X's `blocked_by` unless T's view is `closed`; an uncached T blocks | `rule_blocking_dep_add_onto_an_open_target_blocks_the_bead`, `rule_blocking_dep_add_onto_an_uncached_target_blocks_the_bead` |
| `dep_add` X → T, `parent-child` | T joins X's `blocked_by` when T is in the blocked map | `rule_parent_block_blocks_the_children` |
| `dep_add` of a non-blocking type, or onto a closed T | no effect | `rule_dep_add_onto_a_closed_target_or_of_a_non_blocking_type_changes_nothing` |
| a bead Y becomes blocked (parent block) | Y joins `blocked_by` of each `parent-child` child of Y, recursively | `rule_parent_block_blocks_the_children` |
| a bead Y becomes unblocked (parent unblock) | Y joins `ready` when its view is `open`, carries the scope's label and is not deferred; Y leaves `blocked_by` of each `parent-child` child that has no other blocking edge onto Y, recursively | `rule_parent_unblock_releases_the_children`, `a_child_that_also_blocks_on_its_parent_stays_blocked_when_the_parent_is_unblocked` |

Details:

- **Prior status.** The first entry of X in a pass reads X's status from
  the stored row when the entry is `Pending` (all earlier entries of X are
  applied, so in the row); for an entry applied after `fetched_at` the row
  already shows the write, so the prior is taken from the base: `open` when
  X is ready, "not closed" when X is a blocked row or a blocker, else
  unknown. Later entries of X see the status the earlier ones set.
- **Reopen** covers any status other than `closed` (bd's `reopen` sets
  `open`; an `update` to `in_progress` of a closed bead reopens it as
  well). An unknown prior is treated as closed: re-adding X where bd
  already lists it changes nothing, so the rule errs only towards "not
  ready".
- **Children of a reopened bead** are not blocked by X directly (bd lists a
  child as blocked by its parent only while the parent is blocked, P-6); if
  X's restored blockers make X blocked, the parent-block rule blocks them.
- A reopened X itself never joins `ready`: whether bd would list it
  (deferral, custom statuses) is bd's call, settled at the next sync.
- Every view used (status, labels, `defer_until`, edges) is the current
  view: the stored row with the bead's pending entries replayed. Incoming
  edges are the store's edge index (`dependents_of`) plus the edges of
  pending `dep_add` entries onto the bead, so the rules see the same edges
  as the views (fix-a-6-1, QA-001).
- The rules are idempotent, so an entry already reflected in the base (one
  applied at or after `fetched_at` that bd saw) changes nothing.
- Recursion terminates: each step of parent block turns one more bead from
  unblocked into blocked with nothing unblocked meanwhile, and the reverse
  for parent unblock.

`RowView.blocked` is the bead's list in the adjusted map, `Some([])` when
it has none, and `None` when there is no base or the bead's view does not
carry the scope's label. `locally_adjusted` is true when the adjusted map
or ready list differs from the base.

Consequences: a journaled close releases its dependers at once with bd
unreachable (REQ-BDC-0911); a bead the rules cannot settle stays out of
`ready` until the next sync (REQ-BDC-0904).

## ADR-CORE-0202: Sync failure outcomes

**Status:** Approved (lead ruling, bc-lead, 2026-10-24). Refines ADR-BDC-0005 for
`sync`.

Context: ADR-BDC-0005 says a sync that meets an outage records the outage
instead of a `SYNC_FAILED`, and a call cut by the caller's deadline is
neither; the reply shape for those cases is not fixed by a-1.

Decision:

| Failure in the check, scope list or stale re-read | Reply | `sync_state` |
|---|---|---|
| reachable backend (`CallFailure::Error`) | `Err(SYNC_FAILED)` with `phase`, `cause` = the underlying code and `source` = the backend error; a contract violation names bead and field in `detail` | `last_started_at`, `last_error` = the error |
| unreachable (`CallFailure::Outage`, recorded by `call`) | `Ok(SyncReport { outage: Some(..) })` | `last_started_at` only |
| cut by `d` (`CallFailure::Cut`) | `Ok(SyncReport { deferred: true })` | `last_started_at` only |
| `d` expired before the start | `Ok(SyncReport { deferred: true })`, no call | none |
| store failure | the store's error, unchanged | none |

`last_ok_at` and `last_error` are only changed by a sync that completes
(`last_ok_at` = start, `last_error` cleared) or by `SYNC_FAILED`. No sync
reads or writes a sync alert: announcing is the scheduler's (a-7). The
scope is registered before the first call, so a sync of an unconfigured
phase registers it even when it fails (REQ-BDC-0909: "never synced or
configured" is refused). The ready base is fetched after the rows; its
failure (any kind) keeps the previous base marked stale and does not fail
the sync, since the rows did sync; an outage it meets is recorded by `call`
and reported in `SyncReport.outage`.

Consequences: a scheduler tells the three cases apart from the reply type
and fields alone. A base fetch failure is not in `SyncReport` (lead ruling,
bc-lead, 2026-10-24: no a-1 change); it is visible as the stale mark, as
`ready`'s basis, and as a `tracing` warning (ADR-CORE-0203).

## ADR-CORE-0203: Bounded fresh ready

**Status:** Approved (lead ruling, bc-lead, 2026-10-24). Refines ADR-BDC-0002,
decision 3.

Decision: `ready(scope, Some(limit))` takes `hard = Deadline::after(limit)`
once and hands it to both `call(Ready, hard, ..)`s, so `call` caps each at
`min(hard, call_timeout)`, an expired limit is a `Cut` (no outage), and the
second call only gets what the first left. On success the fetched base
replaces the stored one (its `at` taken before the first call) and the
reply is that base adjusted by the journal, basis
`Fresh { fetched_at: at }` (a-1 amendment 2; fix-a-6-1 ARCH-002). On any failure
the stored base is left as it is, not marked stale (a missed fresh read is
not a failed refresh; only a sync's base fetch marks it), and the reply is
the cached adjusted answer, basis `Cached`, with `read_fallback` =
`"<code>: <detail>; retry, or run sync"`. The code is the error's stable
`BD_CACHE.*` code, and `BD_CACHE.BD_UNAVAILABLE` for a cut, whose detail is
`"bd blocked / bd ready: no answer within <limit>ms"` (fix-a-6-1, RBP-F002). An unreachable backend is still an outage and recorded (REQ-CORE-0001).

When a sync's base fetch fails (any `CallFailure`, ADR-CORE-0202), the
sync emits one `tracing` warning with fields `phase`, `code` and `detail`
(a cut is reported as `BD_CACHE.BD_UNAVAILABLE`) before marking the base
stale. It goes through `sc_beads::logging::tracing` (sc-beads re-exports
`tracing`; ADR-BDC-0007), so `bd-cache` gains no dependency, and it is not
gated on sc-beads' `LOGGING_ENABLED` switch: the subscriber decides.

## ADR-CORE-0204: Read shapes and answer checks

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Decision:

- `list` filters `status` and `label` on the view (stored row with pending
  entries replayed), so a pending close leaves `--status open` at once and
  agrees with `get`.
- A bead's scope for `get` and `refresh` is its v3 phase
  (`ScopeId::from_labels`, the first `phase-<x>` label) when that scope is
  registered, else the first registered scope that holds the view
  (`ScopeId::holds`). A bead with none (out of scope) has no
  `scope_last_ok_at` and `blocked: None`.
- `refresh` returns rows in request order without repeats; ids bd does not
  know are absent (not an error), and the rows are stored with membership
  unchanged, like a miss fill.
- Before storing any row of an answer the cache checks what it can beyond
  bd's strict row validation (REQ-BDC-0008): a `list_by_ids` answer holding
  a bead that was not requested, or a scope list holding a row without the
  scope's label, is `BD_CONTRACT` (in a sync, `SYNC_FAILED` with cause
  `BD_CONTRACT`) and stores nothing.

## ADR-CORE-0205: resolve, prune and status delegate to the store's rules

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Decision: `resolve` calls `CacheStore::resolve` at the clock's UTC time,
`prune` calls `CacheStore::prune_scope`, and `status` returns
`CacheStore::status` (which carries the outage). The state rules
(`NOT_ESCALATED`, `PRUNE_BLOCKED`, the out-of-scope sweep, the note format)
are the store port's, checked by a-1's contract suite; the core does not
repeat them, so there is one implementation of each rule. The planted bugs
"prune ignores escalated" and "resolve accepts any state" are therefore
planted in `FakeStore` and caught by the core's tests.

Consequences: `status` reports what `StoreStatus` holds, including the
stale rows (`StoreStatus.stale`, filled by the store from
`CacheStore::stale_rows`; a-1 amendment 1, lead ruling, bc-lead,
2026-10-24), tested by `status_reports_stale_rows_until_a_sync_rereads_them`.

## ADR-CORE-0206: `get` reads the bead's own entry of the ready base (a-7 amendment)

**Status:** Approved (lead ruling S4, bc-lead, 2026-10-24: an a-6 amendment made in sprint a-7)

Context: `get` computed a row's `blocked` state by adjusting the scope's whole
ready base (ADR-CORE-0201) on every call, so its cost grew with the scope:
1.4 ms at 1000 beads in a release build, against NFR-BDC-0001's 1 ms.

Decision: `get` asks the store for the bead's own entry of the base
(`CacheStore::base_entry`: the base's fetch instant, stale mark and the bead's
`blocked_by`). When `CacheStore::has_entries_since(fetched_at)` is false (nothing
pending and nothing applied at or after the base's fetch instant; two index
lookups, whatever the journal's history), no rule of ADR-CORE-0201 has anything to apply, so the
stored entry is the answer. Otherwise (a write is in flight, for the few
seconds until the next sync; a blocker's close can unblock any bead) the whole
base is adjusted as before. `list`, `ready` and `refresh` are unchanged. The
dedupe of the base's ready ids no longer scans a vector per id.

`Cache::cached(id)` is the same read without the miss fill: `None` for a bead the
cache does not hold, no bd call, which is how the sync task reads a phase's root.

Consequences: a cached `get` costs the same at 10, 100 and 1000 beads, and with
thousands of old applied entries, when no entry is pending or recent (a deterministic test counts what it reads); the ready rules
are untouched, every `read_ready` test passes unchanged. `Cache::clock()` is
added (lead ruling S2): the clock the cache and its backend share.
