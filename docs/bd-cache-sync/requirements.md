# bd-cache-sync Requirements

Status: Approved (lead ruling, bc-lead, 2026-10-24), with the corrections of QA round 1 (fix-a-7-1). Crate-local requirements (domain code `SYNC`)
refining [`../requirements.md`](../requirements.md). Architecture:
[`architecture.md`](architecture.md). Created by sprint a-7.

Test files are under `crates/bd-cache-sync/tests/`. The ported cases of
`test_bd_cache_cron.py` keep their Python names.

## REQ-SYNC-0001: The tick is a library entry point

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`tick(&mut Cache, &TickConfig, &dyn EscalationSender, Deadline)`
runs one tick that must finish by the deadline `T`, on the cache's own clock, and returns a `TickReply`:
`pass-through`, or `refused` with the list of what was refused. It is
independent of the scheduler: `sc-cron` drives it through `SyncJob`, and an
external scheduler (the ATM cron module through the later `bdc tick`) can
call it. Only one ticker may run per cache file. The `[tick]` keys come from
`RepoConfig.tick` (`load_repo_config`, bd-cache-store).

**Refines:** REQ-BDC-0701, 0703
**Acceptance evidence:** `test_missing_team_is_refused`,
`test_tick_syncs_only_phases_with_an_open_root` (`tests/tick_phases.rs`);
the doctests of `tick` and `schedule_spec`.

## REQ-SYNC-0002: Phases come from phase files and the configured list

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A phase is `phase-<x>.toml` in `[tick] phases_dir` (default `.atm-bd/`, relative
to the working directory; `compose` resolves it against the repository root) with a `root` bead and an
optional `lead`; other keys are ignored. `[tick] phases` limits the phases
(names with or without the `phase-` prefix). The schema was confirmed against `bd_cache_cron.py` at atm-core `05831df` and a real `phase-bd.toml` / `phase-bk.toml` is a test fixture (ADR-SYNC-0007). A malformed entry, or an empty list, refuses the
tick before any work; an unusable file, an unreadable directory or a
configured phase with no file is listed as refused while the phases that are
usable still run. A phase is active while its root is not closed: a root the
cache holds in the phase's scope is read from the cache, every other root in
one `bd list --id` under the miss-fill timeout. A root that cannot be read, or
that bd does not know, makes its phase inactive for the tick; both are
reported as unreadable roots, and neither counts as closed for the stop rule
(lead ruling S23, ADR-SYNC-0004).

**Refines:** REQ-BDC-0702
**Acceptance evidence:** `test_tick_syncs_only_phases_with_an_open_root`,
`test_custom_phases_limits_sync_and_stop`,
`test_malformed_custom_phases_is_refused`,
`a_root_the_cache_holds_is_read_from_the_cache_and_a_closing_root_ends_the_phase`,
`a_configured_phase_without_a_file_and_a_broken_file_are_refused_but_the_rest_runs`,
`a_root_that_bd_does_not_know_is_no_open_root_and_is_reported`,
`a_root_with_two_phase_labels_is_trusted_only_after_its_own_phase_syncs`
(`tests/tick_phases.rs`).

## REQ-SYNC-0003: One schedule per repository, with a team and a fallback lead

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`schedule_spec` builds the repository's one schedule (`bd-cache:<cache file>`,
every 30 s, 25 s budget). `[tick] team` and `[tick] fallback_lead` are both
required, non-blank and addressable (no whitespace, no `@`, no leading `-`);
without them `schedule_spec` and `tick` refuse, and a refused tick makes no
`bd` call and sends nothing.

**Refines:** REQ-BDC-0701
**Acceptance evidence:** `test_missing_team_is_refused` (`tests/tick_phases.rs`).

## REQ-SYNC-0004: The tick budget

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A tick drains with `DrainBudget { start_gate: T - 5 s, hard: T - 2 s }`, then
syncs each active phase with deadline `T`. A phase whose sync has not started
by `T - 2 s` is deferred, which is not a refusal. With every `bd` call timing
out, no tick takes longer than 25 s.

**Refines:** REQ-BDC-0703, NFR-BDC-0004 (constraint)
**Acceptance evidence:** `test_tick_budget_defers_drain_entries`,
`no_tick_exceeds_25_s_with_every_bd_call_timing_out`,
`the_drain_runs_before_the_sync` (`tests/tick_budget.rs`).

## REQ-SYNC-0005: Threading of the job

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`SyncJob` holds the `Cache` in an `Arc<Mutex<_>>` and runs every tick and
stop check in `tokio::task::spawn_blocking`, with the run's deadline as `T`.
A tick that finds the mutex held (including by a blocking task left over from
a timed-out tick) is skipped, reported busy, and the scheduler records it as
refused; `should_stop` answers `Continue` when busy. A poisoned mutex is
recovered, cleared, and reported once in the tick reply.

**Refines:** REQ-BDC-0703, ADR-CORE-0001 (constraint)
**Acceptance evidence:**
`a_second_tick_while_a_blocking_sync_is_running_is_skipped_and_reported_busy`,
`a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`,
`should_stop_returns_continue_when_busy`,
`a_poisoned_mutex_is_recovered_and_reported_in_the_tick_reply`
(`tests/job_threading.rs`).

## REQ-SYNC-0006: Announcements are sent through the sender port and retried until delivered

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Every message goes through `EscalationSender`. A sync failure whose cause
differs from the phase's `sync_alert` is sent to the phase's lead (the
fallback lead when the phase has none) and recorded; a success clears the
alert; a cause that is unreachability is the outage's to announce. Every
escalated entry with no `announced_at` is sent to its phase's lead with the
`resolve` command, then marked; an entry with no scope goes to the fallback
lead; an entry whose phase has no lead is refused. A message not delivered
leaves its marker unset, makes the reply `refused` with the list, and is sent
again on the next tick.

**Refines:** REQ-BDC-0704, 0705, 0706
**Acceptance evidence:** `test_sync_failure_is_announced_once_per_cause`,
`test_escalation_is_sent_once_to_the_phase_lead`,
`test_undelivered_escalation_is_retried_and_the_tick_refuses`,
`an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`,
`an_entry_whose_phase_has_no_lead_is_refused_every_tick_until_it_has_one`,
`an_undelivered_sync_failure_is_refused_and_retried_not_recorded` (`tests/announce.rs`).

## REQ-SYNC-0007: An outage is announced once per scope through the sync alert

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

While the cache records an outage, each active scope whose alert does not
read `backend_unavailable` has its lead told once (one message per distinct
lead, covering that lead's scopes) and the alert set. When no phase is known
active (an uncached root that cannot be read), every configured phase is
told. A scope without a lead uses the fallback lead. When the outage ends the
alerts that read `backend_unavailable` are reset and no message is sent. An
undelivered message sets no alert and is retried. The outage is cleared only
by a list that answers (a-1's `call`), never by the backend check.

**Refines:** REQ-BDC-0704, 0903, ADR-BDC-0005
**Acceptance evidence:** `test_backend_outage_is_announced_once_and_sync_failures_are_not`,
`test_outage_with_uncached_roots_still_tells_every_configured_lead`,
`test_outage_is_not_cleared_while_only_the_backend_check_answers`,
`test_undelivered_outage_announcement_is_retried`,
`a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once`,
`during_an_outage_of_several_ticks_reads_report_it_writes_wait_and_recovery_drains`,
`an_outage_alert_in_bd_cache_pys_form_is_not_announced_again`
(`tests/outage.rs`); `an_outage_of_several_ticks_through_the_real_store_and_cli` (`tests/e2e.rs`).

## REQ-SYNC-0008: The stop rule never stops on uncertainty

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`stop_reason` stops the schedule when no phase is active and no journal entry
is pending or escalated-and-unannounced. It answers "keep going" when the
`[tick]` table would be refused, the phases directory or a phase file cannot be
read, a root cannot be read or bd does not return it (lead ruling S23), or the
store fails.

**Refines:** REQ-BDC-0707
**Acceptance evidence:** `test_stop_only_when_no_active_phase_and_no_open_work`,
`a_phase_whose_root_cannot_be_read_never_ends_the_schedule`,
`configuration_the_tick_would_refuse_never_ends_the_schedule`,
`a_root_bd_does_not_return_never_ends_the_schedule` (`tests/stop_rule.rs`).

## REQ-SYNC-0009: `atm send` is bounded and carries the message on stdin

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`AtmSender` runs `atm send <lead>@<team> --team tool --stdin --json` with
`ATM_IDENTITY=cron` and `ATM_TEAM=tool` (the reference's invocation,
ADR-SYNC-0007) and writes the message to stdin. An address that could be read as an option, or
split in two, is refused before a process starts. A child still running after
the timeout (10 s) is killed and the send fails; a nonzero exit fails with the
child's stderr.

**Refines:** REQ-BDC-0704..0706
**Acceptance evidence:** `tests/atm_process.rs` (its own fake `atm`, 13 cases):
`delivers_the_body_on_stdin_with_the_reference_invocation`,
`a_large_message_is_not_limited_by_the_argument_list`,
`a_failing_atm_is_a_send_error_that_says_why`,
`output_beyond_the_limit_is_dropped_and_the_error_says_so`,
`an_address_that_could_be_read_as_an_option_is_refused_before_any_process_starts`,
`a_missing_binary_is_a_send_error_and_does_not_resolve`,
`a_timeout_that_no_message_could_use_is_refused`.

## REQ-SYNC-0010: The end-to-end flow runs through the real composition

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A write journaled through the core lands in `bd` via the drain a scheduler
runs, verified by read-back, through `SyncJob`, `Cache`, `SqliteStore`,
`FlockDrainLock` and `BdCli` (the `bd` process replaced by `FakeBd` in CI,
a scratch `bd` workspace on the testbed). A failed read-back marks the entry
applied and the row stale and holds the bead until a sync. A journaled close or
reopen of a blocker, inside or outside every scope, changes its depender's
readiness in the same tick's sync.

**Refines:** REQ-BDC-0003, 0903, 0904
**Acceptance evidence:**
`a_write_journaled_through_the_core_lands_in_bd_via_the_scheduled_drain`,
`test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`,
`test_e2e_blocker_close_and_reopen_update_ready` (`tests/e2e.rs`); `tests/live.rs`
(testbed).

## REQ-SYNC-0011: Messages are sent inside the tick's budget

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Before each message the tick checks its stop flag and the time left before
`T`; the sender is given that time (`send_within`) and never runs longer. With
less than half a second left the message is not started: it is counted in
`TickReply::unsent`, stays unmarked and the next tick sends it. A tick whose
sender takes 9 s per message ends within 25 s.

Amendment (fix-a-7-5, RSH-027, REQ-QA-501; **Status:** Approved (lead ruling S27, bc-lead, 2026-10-24)):
"the time left" is the time before `T` less the send reserve `SEND_RESERVE`,
1.5 s, so a message is not started with less than 2 s before `T`. The reserve
covers the most an `atm send` can run past its deadline: the read grace
(250 ms) plus one shared kill bound (1 s) in which the tree kill and the
reaping of the child are both done or given up on. Const assertions in
`atm.rs` and `control.rs` tie `SEND_RESERVE` to `READ_GRACE`, `READ_FLOOR`,
`KILL_BOUND` and the poll intervals, and keep `MIN_SEND_BUDGET + SEND_RESERVE`
within the sync step's `HARD_MARGIN` (2 s). Evidence:
`a_child_that_ignores_term_and_its_descendant_are_killed_by_the_real_tree_kill`
(the group kill, generous deadline; fix-a-7-10, FTQ-025) and
`a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped` (the deadline)
(`src/atm.rs`),
`a_send_is_given_the_time_left_minus_the_reserve_and_nothing_below_the_floor`
(`src/control.rs`).

Amendment (fix-a-7-8, RSH-029; **Status:** Approved (lead ruling S33, bc-lead, 2026-10-24)):
the marker written after a delivered message (`announced_at`, a sync alert)
waits for the store's write lock at most the time left before `T`, never the
store's full 5 s busy timeout, so a write lock another process holds can carry
that write to `T` plus the busy handler's sleep slack (tens of ms), no further
(lead ruling S35, ARCH-705, QA7-002). The bound applies to the bounded writes
only: these markers and the alert clears without a message (after a passing
sync, at the end of an outage), which S35 bounds the same way; a clear that
fails is logged and does not fail the tick. A marker not written in time fails the tick with the
store's error and leaves the message unmarked; the next tick sends it again
(at least once, ADR-SYNC-0003). Evidence: `tests/marker_wait.rs`,
`a_marker_write_may_wait_the_time_left_before_t` (`src/control.rs`),
`a_bounded_marker_write_gives_up_on_a_held_lock_within_its_wait`
(`bd-cache-store/tests/store.rs`); for S35,
`a_failed_marker_write_fails_the_tick_and_the_next_tick_sends_again` and
`a_failed_alert_clear_is_logged_and_does_not_fail_the_tick`
(`tests/marker_wait.rs`).

**Refines:** REQ-BDC-0703, 0706, NFR-BDC-0004 (constraint)
**Acceptance evidence:** `many_escalations_and_a_slow_sender_stay_within_the_tick_budget`,
`a_stopped_tick_does_nothing_and_the_next_tick_does_it_all` (`tests/announce.rs`).

## REQ-SYNC-0012: Cooperative shutdown

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A `StopFlag` ends a running tick after the drain, before the one
`bd list --id` of the uncached phase roots, between phases and before every message;
given to the `AtmSender` it kills the child and its whole process tree. An
`atm` that outlives its timeout takes its descendants with it.

**Refines:** REQ-BDC-0703
**Acceptance evidence:** `the_stop_flag_ends_a_tick_between_phases`
(`tests/job_threading.rs`); `a_set_stop_flag_skips_the_root_refresh` (`src/active.rs`);
`the_stop_flag_kills_the_child_and_its_descendants`,
`an_atm_that_hangs_is_killed_and_reaped` (kill and reap, stop-flag form: the
stop comes once the fakes' pid files are written) and
`the_time_a_tick_passes_bounds_the_send` (the one timer-driven kill: `TimedOut`
and its detail only) (`tests/atm_process.rs`; fix-a-7-10, FTQ-026, FTQ-027).

## REQ-SYNC-0013: A cached `get` costs the same at any scope size

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Owned by a-7 through NFR-BDC-0001, implemented in `bd-cache` and
`bd-cache-store` by lead ruling S4 (bc-lead, 2026-10-24): a cached `get` reads the
bead's own entry of the stored ready base (`CacheStore::base_entry`) and
adjusts the whole base only when a pending or recently applied entry could
have changed it.

**Refines:** NFR-BDC-0001
**Acceptance evidence:** `a_cached_get_touches_the_same_rows_at_10_100_and_1000_beads`,
`a_pending_entry_makes_get_adjust_the_whole_base_and_still_agree` (`tests/perf.rs`);
`base_entry_rules` (the store contract suite); release-profile medians in the sprint doc.
