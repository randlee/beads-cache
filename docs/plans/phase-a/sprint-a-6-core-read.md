---
id: a-6
phase: a
status: in-review
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
  owns: [REQ-BDC-0001, REQ-BDC-0008, REQ-BDC-0202, REQ-BDC-0203, REQ-BDC-0501, REQ-BDC-0502, REQ-BDC-0503, REQ-BDC-0504, REQ-BDC-0601, REQ-BDC-0602, REQ-BDC-0603, REQ-BDC-0902, REQ-BDC-0904, REQ-BDC-0907, REQ-BDC-0908, REQ-BDC-0909, REQ-BDC-0911, REQ-BDC-0912, REQ-CORE-0201, REQ-CORE-0202, REQ-CORE-0203, REQ-CORE-0204, REQ-CORE-0205, REQ-CORE-0206, REQ-CORE-0207]
  constrained_by: [REQ-BDC-0105, REQ-BDC-0109, REQ-BDC-0405, REQ-BDC-0901, REQ-BDC-0903, REQ-BDC-0604, REQ-BDC-0704, REQ-BDC-0705, REQ-BDC-0707, NFR-BDC-0001, NFR-BDC-0004, NFR-BDC-0005, REQ-CORE-0001, REQ-CORE-0002, REQ-CORE-0003, REQ-CORE-0004]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0007, ADR-BEADS-0001, ADR-BEADS-0002, ADR-BEADS-0003, ADR-BEADS-0004, ADR-MODEL-0002, ADR-MODEL-0003, ADR-CORE-0001, ADR-CORE-0002, ADR-CORE-0201, ADR-CORE-0202, ADR-CORE-0203, ADR-CORE-0204, ADR-CORE-0205]
owned_paths:
  - "crates/bd-cache/src/get.rs"
  - "crates/bd-cache/src/refresh.rs"
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
   unknown scope is `SCOPE_UNKNOWN` naming the known scopes.
3. `sync(scope, deadline)` (REQ-BDC-0501..0504, 0008): `backend_check::run`,
   cursor minted from the sync start time and sent as `last_ok_at - 2 min`,
   validate every row before storing any, newer-row guard, stale re-read
   with `force = true`,
   `SYNC_FAILED` recorded for a reachable backend; an outage records the
   outage instead; a cut sets `SyncReport.deferred` with no `SYNC_FAILED` and
   no alert change (ADR-BDC-0005).
4. Ready base and `ready` (REQ-BDC-0904, 0911, ADR-BDC-0002): each sync of a
   scope ends with `list_blocked` and `list_ready` through `call` and
      `replace_ready_base` (its `at` taken before the first call); on failure
   the previous base is kept and marked stale. Bounded fresh reads use
   `CallKind::Ready` with one deadline for both calls. `ready(scope, fresh_within)` applies the journal rules in ADR-BDC-0002 to
   the base; with no limit it makes no backend call, and with a limit it
   tries bd's answer under that limit and falls back to the cached answer
   with `read_fallback` (REQ-BDC-0912). No base and no fresh answer is
   `unknown`. `get`, `list` and `ready` rows fill `blocked` from the same
   adjusted base (`None` when there is no base or the bead is not in it,
   `Some([])` when not blocked). Ready ids with no cached row are listed in
   `missing_rows`. The local rules are written as a table in
   `docs/bd-cache/read-path.md` (op and dep type × effect).
5. `refresh(ids, timeout)` (REQ-BDC-0907): one `list_by_ids` with all
   statuses, stored with force, overlay applied; `entry(seq)` (REQ-BDC-0908);
   `get` takes an optional caller miss timeout. A `sync` of an unconfigured
   phase registers it as an on-demand scope (REQ-BDC-0909).
6. Announcement state on `Cache` (`unannounced_escalations`,
   `mark_announced`, `sync_alert`, `set_sync_alert`, `open_work`) as store
   passthroughs, for external schedulers.
7. `resolve`, `prune` (with the out-of-scope sweep), `status`
   (REQ-BDC-0601..0603); `status` reports each scope's `last_ok_at`, stale
   rows and the outage (ADR-BDC-0005).
8. `docs/bd-cache/read-path.md`: crate ADRs for the read path
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
| A failed `list_blocked` or `list_ready` keeps the previous base, marked stale | ADR-BDC-0002 |
| After a blocker (cached or outside every scope) is closed in the backend, one sync makes its depender ready; after it is reopened, one sync makes the depender blocked again; a child of a blocked parent is not ready (REQ-BDC-0904) | ADR-BDC-0002 |
| With the backend unreachable, a journaled close of a bead's only blocker makes it ready at once; a journaled reopen blocks it again (REQ-BDC-0911) | ADR-BDC-0002 |
| A scope with no ready base reports `unknown` | ADR-BDC-0002 |
| Rows carry `blocked` from the adjusted base (`None` without a base); a ready id with no cached row appears in `missing_rows` only | ADR-BDC-0002 |
| Only entries Pending or applied at or after `fetched_at` adjust the base; one applied before it does not | ADR-BDC-0002 |
| Each row of the read-path rules table has a test (close, `Update` to closed, reopen from closed, blocking dep_add onto open and onto uncached targets, parent unblock and block) | ADR-BDC-0002 |
| `ready(scope, 500 ms)` returns bd's answer and refreshes the base when `FakeBackend` answers in 100 ms; when it takes 5 s, returns the cached adjusted answer with `read_fallback` set, within 500 ms by the `ManualClock`; the expired limit records no outage (REQ-BDC-0912) | ADR-BDC-0005 |
| Planted bugs: prune ignores escalated, resolve accepts any state each fail a test | boundary:bd-cache |

## This sprint does not close

Scheduled sync with the real store and backend, and escalation delivery
(a-7).

## Required validation

Crate tests and lints; `cargo build --workspace`.

## Requirement scope notes

- REQ-CORE-0201..0207 and ADR-CORE-0201..0205 are new crate ids in
  [`docs/bd-cache/read-path.md`](../../bd-cache/read-path.md), Approved
  (lead ruling, bc-lead, 2026-10-24, R1). ADR-CORE-0201..0203 refine
  ADR-BDC-0002 and ADR-BDC-0005; none amends an Approved ADR. ADR-BDC-0002's
  reopen rule was clarified to match ADR-CORE-0201 (R3, `integrate/phase-a`
  at `60e630f`, merged into this branch).
- A failed ready-base fetch during sync is not added to `SyncReport` (R4,
  declined); sync emits a `tracing` warning with the error's code and
  detail instead (ADR-CORE-0203).
- REQ-BDC-0503's acceptance case
  `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`
  is a-7's (it spans drain and sync). a-6 tests the sync half: the stale
  row is re-read forced and its mark cleared, which is what lets a-5's drain
  take the bead's held entries
  (`a_stale_row_is_reread_by_the_next_sync_and_its_held_entries_become_drainable`).
  `drain` itself is a-5's and still a stub on this branch.
- REQ-BDC-0601 and 0602: the state rules are the store port's (a-1's
  contract suite; a-4 implements them over SQLite). a-6 owns the verbs and
  delegates (ADR-CORE-0205), so its planted bugs for these two are planted
  in `FakeStore`.
- REQ-BDC-0603 and deliverable 7 ("`status` reports each scope's
  `last_ok_at`, stale rows and the outage"): `last_ok_at` and the outage are
  reported, and so are stale rows: a-1 amendment 1 (R2, lead ruling,
  bc-lead, 2026-10-24; merged into `integrate/phase-a` at `f3a4a40` and
  into this branch) added `StoreStatus.stale: Vec<BeadId>`, which the store
  fills from `CacheStore::stale_rows`. `status` passes it through
  (ADR-CORE-0205). Closed by
  `status_reports_stale_rows_until_a_sync_rereads_them`.
- Deliverable 6 (announcement passthroughs) was implemented by a-1 in
  `lib.rs` (ADR-CORE-0002); a-6 adds its tests
  (`announcement_state_passes_through_to_the_store`). The passthroughs serve
  REQ-BDC-0604 (an escalation stays unannounced until `mark_announced`,
  which records `announced_at` at the clock's time), and the scheduler-side
  requirements built on them: REQ-BDC-0704 (`sync_alert` /
  `set_sync_alert`), REQ-BDC-0705 (`unannounced_escalations`,
  `mark_announced`) and REQ-BDC-0707 (`open_work`). Those are listed in
  `constrained_by`; announcing is a-7's, the store side a-4's.
- ADR-BEADS-0001 is listed because the R4 warning goes through the
  `tracing` re-export of `sc_beads::logging` (ADR-BEADS-0001, port btit
  verbatim; logging through tracing).
- fix-a-6-1 ARCH-001 and ARCH-002 (closed): a-1 amendment 2 (merged into
  `integrate/phase-a` at `454b802` and into this branch) added
  `ReadyReply.last_ok_at` and `ReadyBasis::Fresh { fetched_at }`. `ready`
  fills `last_ok_at` from `sync_state(s).last_ok_at`
  (`ready_replies_carry_the_scopes_last_ok_at`) and `fetched_at` from the
  fresh base's `at`, the instant before its first call
  (`bounded_fresh_ready_returns_bds_answer_and_refreshes_the_base`).
- REQ-BDC-0109 (edge index) and REQ-BDC-0405 (read-back stale mark) are
  constraints: the ready rules read incoming edges through `dependents_of`,
  and sync clears the stale mark the drain sets.
- NFR-BDC-0001 (cached `get` under 1 ms) is measured by a-7 on the real
  store; a-6 keeps a cached `get` free of backend calls.
- The cache's own answer checks (an unrequested bead in a `list --id`
  answer, a scope row without the scope's label) are tested through a
  wrapper that runs every row through `sc_beads::parse_bd_rows`, the
  `sc-bd-cli` validation path (`tests/sync_contract.rs`), because
  `FakeBackend` returns typed rows and has no contract-error injection.

## Acceptance evidence

| Criterion | Tests |
|---|---|
| REQ-BDC-0001, 0202 | `test_miss_fetches_once_then_serves_from_cache`, `test_miss_on_unknown_bead_is_typed` (`tests/read_get.rs`) |
| REQ-BDC-0008 | `test_contract_violation_is_typed_and_sync_stores_nothing`, `a_scope_list_row_without_the_scope_label_stores_nothing`, `a_bad_row_in_a_refresh_or_miss_fill_stores_nothing` (`tests/sync_contract.rs`) |
| REQ-BDC-0501, 0502, 0504 | `test_sync_is_incremental_and_never_replaces_a_newer_row`, `test_sync_failure_is_typed_and_recorded`, `a_rebuild_drops_the_cursor_so_the_next_list_is_full` (`tests/sync_rows.rs`) |
| REQ-BDC-0503 | `a_stale_row_is_reread_by_the_next_sync_and_its_held_entries_become_drainable` |
| REQ-BDC-0601, 0602 | `test_prune_refuses_with_unapplied_writes`, `test_resolve_closes_only_escalated_entries_and_unblocks_prune`, `test_resolve_keeps_the_note_when_the_entry_had_no_error`, `prune_deletes_the_scopes_beads_journal_sync_state_and_ready_base` (`tests/read_journal.rs`) |
| REQ-BDC-0907, 0908, 0909 | `refresh_reads_fresh_rows_past_a_stale_cache_and_returns_them_with_the_overlay`, `refresh_is_bounded_by_its_timeout_and_an_empty_refresh_calls_nothing`, `entry_returns_an_entry_in_any_state`, `sync_then_list_of_an_unconfigured_phase_works` |
| REQ-BDC-0902 | `list_of_a_scope_never_configured_or_synced_is_scope_unknown_naming_the_known_scopes`, `ready_of_an_unknown_scope_is_scope_unknown`, `a_get_miss_outside_every_scope_is_stored_with_no_membership_and_swept_by_prune_once_it_has_no_open_entry` |
| REQ-BDC-0001, 0203 | `get_and_list_replies_carry_stale_per_row_and_the_scopes_last_ok_at`, `list_filters_by_status_and_label_on_the_view`, `get_replays_pending_entries_and_counts_them`, `a_miss_is_bounded_by_the_callers_timeout` |
| ADR-BDC-0005 (sync) | `test_sync_records_an_outage_and_a_passing_sync_clears_it`, `an_unreachable_backend_check_is_an_outage_too`, `a_cut_sync_reports_deferred_and_changes_no_alert`, `replies_carry_the_outage_while_one_holds` |
| ADR-BDC-0002 (base kept stale) | `a_failed_list_blocked_or_list_ready_keeps_the_previous_base_marked_stale`, `each_sync_stores_the_ready_base_fetched_after_the_rows` |
| REQ-BDC-0904 | `after_a_cached_blocker_closes_in_bd_one_sync_makes_its_depender_ready_and_reopen_blocks_it`, `after_a_blocker_outside_every_scope_closes_in_bd_one_sync_makes_its_depender_ready`, `a_child_of_a_blocked_parent_is_not_ready` |
| REQ-BDC-0911 | `with_bd_unreachable_a_journaled_close_of_the_only_blocker_makes_the_depender_ready_at_once` (`tests/read_ready.rs`) |
| No base is `unknown` | `a_scope_with_no_ready_base_reports_unknown`, `no_base_and_no_fresh_answer_is_unknown` |
| `blocked` and `missing_rows` | `rows_carry_blocked_from_the_adjusted_base_and_a_ready_id_without_a_row_is_only_in_missing_rows` |
| Entries vs `fetched_at` | `only_entries_pending_or_applied_at_or_after_fetched_at_adjust_the_base`, `escalated_entries_do_not_adjust_the_base` |
| Rules table rows (ADR-CORE-0201) | `a_pending_blocks_dep_add_keeps_a_child_blocked_when_its_parent_is_unblocked`, `a_pending_parent_child_dep_add_carries_a_later_parent_block_to_the_child`, `rule_update_to_closed_is_a_close`, `rule_close_or_status_change_of_the_bead_itself_leaves_ready`, `rule_reopen_from_closed_blocks_the_dependers_again`, `rule_reopen_brings_back_the_beads_own_blockers`, `rule_blocking_dep_add_onto_an_open_target_blocks_the_bead`, `rule_blocking_dep_add_onto_an_uncached_target_blocks_the_bead`, `rule_dep_add_onto_a_closed_target_or_of_a_non_blocking_type_changes_nothing`, `rule_parent_unblock_releases_the_children`, `rule_parent_block_blocks_the_children`, `a_child_that_also_blocks_on_its_parent_stays_blocked_when_the_parent_is_unblocked` |
| `ready` carries `last_ok_at` (ARCH-001) | `ready_replies_carry_the_scopes_last_ok_at`, `a_scope_with_no_ready_base_reports_unknown` |
| REQ-BDC-0912 (with `Fresh { fetched_at }`, ARCH-002) | `bounded_fresh_ready_returns_bds_answer_and_refreshes_the_base`, `bounded_fresh_ready_falls_back_to_the_cached_answer_at_the_limit_without_an_outage`, `bounded_fresh_ready_on_an_unreachable_backend_falls_back_and_records_the_outage` |
| Deliverable 6 (REQ-BDC-0604, 0704, 0705, 0707) | `announcement_state_passes_through_to_the_store` |
| Deliverable 7 (REQ-BDC-0603) | `status_reports_each_scopes_last_ok_at_the_journal_escalations_and_the_outage`, `status_reports_stale_rows_until_a_sync_rereads_them` |
| Planted bugs | below |

## Evidence: planted bugs (NFR-BDC-0005)

Run on commit `ae32b4606fc72a70ac0e15d3ac588017f48b308e` (fix-a-6-1, after merging
`integrate/phase-a` at `454b802`), the last commit that changes code (later commits change documents only). Each mutation was
applied to the working tree by a script (one literal replacement, checked to
match exactly once), the whole `bd-cache` suite was run, and the file was
restored:

```text
cargo test -p bd-cache --all-features --no-fail-fast
git checkout -- <file>
```

The two baseline bugs for a-6 come first. Their rules are the store's
(ADR-CORE-0205), so they are planted in `FakeStore` and caught by the core's
tests. The rest are planted in this sprint's own rules, one or more per
deliverable.

| Planted bug | File | Exit | Compile error | Failing tests |
|---|---|---|---|---|
| prune ignores escalated | `crates/bd-cache-model/src/testing/store.rs` | 101 | no | `test_prune_refuses_with_unapplied_writes`, `test_resolve_closes_only_escalated_entries_and_unblocks_prune` |
| resolve accepts any state | `crates/bd-cache-model/src/testing/store.rs` | 101 | no | `test_resolve_closes_only_escalated_entries_and_unblocks_prune` |
| sync: no newer-row guard | `crates/bd-cache/src/sync.rs` | 101 | no | `test_sync_is_incremental_and_never_replaces_a_newer_row` |
| sync: cursor without the 2 min margin | `crates/bd-cache/src/sync.rs` | 101 | no | `test_sync_is_incremental_and_never_replaces_a_newer_row` |
| sync: no stale re-read | `crates/bd-cache/src/sync.rs` | 101 | no | `a_stale_row_is_reread_by_the_next_sync_and_its_held_entries_become_drainable` |
| sync: rows stored before the whole result is checked | `crates/bd-cache/src/sync.rs` | 101 | no | `a_scope_list_row_without_the_scope_label_stores_nothing` |
| sync: an outage recorded as a sync failure | `crates/bd-cache/src/sync.rs` | 101 | no | `an_unreachable_backend_check_is_an_outage_too`, `test_sync_records_an_outage_and_a_passing_sync_clears_it` |
| sync: a cut not reported deferred | `crates/bd-cache/src/sync.rs` | 101 | no | `a_cut_sync_reports_deferred_and_changes_no_alert` |
| sync: a failed base fetch not marked stale | `crates/bd-cache/src/sync.rs` | 101 | no | `a_failed_list_blocked_or_list_ready_keeps_the_previous_base_marked_stale` |
| sync: no on-demand scope registration | `crates/bd-cache/src/sync.rs` | 101 | no | `sync_then_list_of_an_unconfigured_phase_works` |
| ready: close does not release dependers | `crates/bd-cache/src/ready.rs` | 101 | no | `a_pending_blocks_dep_add_keeps_a_child_blocked_when_its_parent_is_unblocked`, `bounded_fresh_ready_falls_back_to_the_cached_answer_at_the_limit_without_an_outage`, `only_entries_pending_or_applied_at_or_after_fetched_at_adjust_the_base`, `rule_parent_unblock_releases_the_children`, `rule_update_to_closed_is_a_close`, `with_bd_unreachable_a_journaled_close_of_the_only_blocker_makes_the_depender_ready_at_once` |
| ready: Update to closed is not a close | `crates/bd-cache/src/ready.rs` | 101 | no | `rule_update_to_closed_is_a_close` |
| ready: status change keeps the bead ready | `crates/bd-cache/src/ready.rs` | 101 | no | `rule_close_or_status_change_of_the_bead_itself_leaves_ready` |
| ready: no reopen rule | `crates/bd-cache/src/ready.rs` | 101 | no | `rule_reopen_brings_back_the_beads_own_blockers`, `rule_reopen_from_closed_blocks_the_dependers_again`, `with_bd_unreachable_a_journaled_close_of_the_only_blocker_makes_the_depender_ready_at_once` |
| ready: no dep_add rule | `crates/bd-cache/src/ready.rs` | 101 | no | `a_pending_parent_child_dep_add_carries_a_later_parent_block_to_the_child`, `rule_blocking_dep_add_onto_an_open_target_blocks_the_bead`, `rule_blocking_dep_add_onto_an_uncached_target_blocks_the_bead`, `rule_parent_block_blocks_the_children` |
| ready: an uncached dep_add target does not block | `crates/bd-cache/src/ready.rs` | 101 | no | `rule_blocking_dep_add_onto_an_uncached_target_blocks_the_bead` |
| ready: no parent block | `crates/bd-cache/src/ready.rs` | 101 | no | `a_pending_parent_child_dep_add_carries_a_later_parent_block_to_the_child`, `rule_parent_block_blocks_the_children` |
| ready: no parent unblock | `crates/bd-cache/src/ready.rs` | 101 | no | `rule_parent_unblock_releases_the_children` |
| ready: pending dep_add edges missing from the incoming edges | `crates/bd-cache/src/ready.rs` | 101 | no | `a_pending_blocks_dep_add_keeps_a_child_blocked_when_its_parent_is_unblocked`, `a_pending_parent_child_dep_add_carries_a_later_parent_block_to_the_child` |
| ready: entries applied after the fetch ignored | `crates/bd-cache/src/ready.rs` | 101 | no | `only_entries_pending_or_applied_at_or_after_fetched_at_adjust_the_base` |
| ready: entries applied before the fetch replayed | `crates/bd-cache/src/ready.rs` | 101 | no | `only_entries_pending_or_applied_at_or_after_fetched_at_adjust_the_base` |
| ready: a fresh failure is an error, no fallback | `crates/bd-cache/src/ready.rs` | 101 | no | `bounded_fresh_ready_falls_back_to_the_cached_answer_at_the_limit_without_an_outage`, `bounded_fresh_ready_on_an_unreachable_backend_falls_back_and_records_the_outage`, `no_base_and_no_fresh_answer_is_unknown` |
| ready: the second fresh call gets its own deadline | `crates/bd-cache/src/ready.rs` | 101 | no | `bounded_fresh_ready_falls_back_to_the_cached_answer_at_the_limit_without_an_outage` |
| ready: a fresh answer does not refresh the base | `crates/bd-cache/src/ready.rs` | 101 | no | `bounded_fresh_ready_returns_bds_answer_and_refreshes_the_base` |
| ready/list: an unknown scope is answered | `crates/bd-cache/src/ready.rs` | 101 | no | `list_of_a_scope_never_configured_or_synced_is_scope_unknown_naming_the_known_scopes`, `ready_of_an_unknown_scope_is_scope_unknown`, `sync_then_list_of_an_unconfigured_phase_works` |
| rows outside the scope get a blocked state | `crates/bd-cache/src/ready.rs` | 101 | no | `rows_carry_blocked_from_the_adjusted_base_and_a_ready_id_without_a_row_is_only_in_missing_rows` |
| status: stale rows dropped | `crates/bd-cache/src/status.rs` | 101 | no | `status_reports_stale_rows_until_a_sync_rereads_them` |
| ready: last_ok_at not reported | `crates/bd-cache/src/ready.rs` | 101 | no | `ready_replies_carry_the_scopes_last_ok_at` |
| ready: Fresh fetched_at not the fetch instant | `crates/bd-cache/src/ready.rs` | 101 | no | `bounded_fresh_ready_returns_bds_answer_and_refreshes_the_base` |
| refresh: not forced | `crates/bd-cache/src/refresh.rs` | 101 | no | `refresh_reads_fresh_rows_past_a_stale_cache_and_returns_them_with_the_overlay` |
| refresh: an unrequested bead is stored | `crates/bd-cache/src/refresh.rs` | 101 | no | `a_bad_row_in_a_refresh_or_miss_fill_stores_nothing` |
| list: no overlay | `crates/bd-cache/src/list.rs` | 101 | no | `list_filters_by_status_and_label_on_the_view`, `rows_carry_blocked_from_the_adjusted_base_and_a_ready_id_without_a_row_is_only_in_missing_rows` |
| list: status filter dropped | `crates/bd-cache/src/list.rs` | 101 | no | `list_filters_by_status_and_label_on_the_view` |

Every one of the 33 bugs fails at least one test, none by a compile error,
and every mutated source file was restored after the run (only this
document's uncommitted edits remained). In a dry run before the evidence
commit, "rows outside the scope get a blocked state" survived, which showed
a missing case (a listed row whose view a pending write moves out of the
scope). The case was added to
`rows_carry_blocked_from_the_adjusted_base_and_a_ready_id_without_a_row_is_only_in_missing_rows`.
A first "status filter dropped" mutant was equivalent to the original code
and was replaced by the one above. fix-a-6-1 added two: "pending dep_add
edges missing from the incoming edges" (QA-001) and "status: stale rows
dropped" (R2), then "ready: last_ok_at not reported" (ARCH-001) and "ready:
Fresh fetched_at not the fetch instant" (ARCH-002; the fetch instant moved
back one second, since substituting the adjusted base's `fetched_at` would
be equivalent after a fresh success).

Contract suites: `tests/sync_contract.rs` runs a-1's `CacheBackend`
contract suite (`backend_contract_tests!`, 9 rules) against
`StrictBackend`, the strict-validation wrapper the REQ-BDC-0008 tests use. The
store contract suite does not apply, because a-6 implements no `CacheStore`.

## Validation

At `ae32b46`, locally (macOS), after merging `integrate/phase-a` at `454b802`:

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo clippy --workspace --lib -- -D warnings` | pass |
| `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps` | pass |
| `cargo test --workspace --all-features` | pass (398 tests; `bd-cache` 86) |
| `cargo build --workspace` | pass |

CI: recorded on the PR.
