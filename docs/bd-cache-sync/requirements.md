# bd-cache-sync Requirements

Status: Draft (sprint a-7, awaiting lead ruling). Crate-local requirements (domain code `SYNC`)
refining [`../requirements.md`](../requirements.md). Architecture:
[`architecture.md`](architecture.md). Created by sprint a-7.

Test files are under `crates/bd-cache-sync/tests/`. The ported cases of
`test_bd_cache_cron.py` keep their Python names.

## REQ-SYNC-0001: The tick is a library entry point

**Status:** Draft

`tick(&mut Cache, &TickConfig, &dyn EscalationSender, &dyn Clock, Deadline)`
runs one tick that must finish by the deadline `T`, and returns a `TickReply`:
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

**Status:** Draft

A phase is `phase-<x>.toml` in `[tick] phases_dir` (default `.atm-bd/`, relative
to the working directory, which is the beads root) with a `root` bead and an
optional `lead`; other keys are ignored. `[tick] phases` limits the phases
(names with or without the `phase-` prefix). A malformed entry refuses the
tick before any work; an unusable file, an unreadable directory or a
configured phase with no file is listed as refused while the phases that are
usable still run. A phase is active while its root is not closed: a root the
cache holds in the phase's scope is read from the cache, every other root in
one `bd list --id` under the miss-fill timeout. A root that cannot be read, or
that bd does not know, makes its phase inactive for the tick; unreadable
roots are reported.

**Refines:** REQ-BDC-0702
**Acceptance evidence:** `test_tick_syncs_only_phases_with_an_open_root`,
`test_custom_phases_limits_sync_and_stop`,
`test_malformed_custom_phases_is_refused`,
`a_root_the_cache_holds_is_read_from_the_cache_and_a_closing_root_ends_the_phase`,
`a_configured_phase_without_a_file_and_a_broken_file_are_refused_but_the_rest_runs`
(`tests/tick_phases.rs`).

## REQ-SYNC-0003: One schedule per repository, with a team and a fallback lead

**Status:** Draft

`schedule_spec` builds the repository's one schedule (`bd-cache:<cache file>`,
every 30 s, 25 s budget). `[tick] team` and `[tick] fallback_lead` are both
required, non-blank and addressable (no whitespace, no `@`, no leading `-`);
without them `schedule_spec` and `tick` refuse, and a refused tick makes no
`bd` call and sends nothing.

**Refines:** REQ-BDC-0701
**Acceptance evidence:** `test_missing_team_is_refused` (`tests/tick_phases.rs`).

## REQ-SYNC-0004: The tick budget

**Status:** Draft

A tick drains with `DrainBudget { start_gate: T - 5 s, hard: T - 2 s }`, then
syncs each active phase with deadline `T`. A phase whose sync has not started
by `T - 2 s` is deferred, which is not a refusal. With every `bd` call timing
out, no tick takes longer than 25 s.

**Refines:** REQ-BDC-0703, NFR-BDC-0004 (constraint)
**Acceptance evidence:** `test_tick_budget_defers_drain_entries`,
`no_tick_exceeds_25_s_with_every_bd_call_timing_out`,
`the_drain_runs_before_the_sync` (`tests/tick_budget.rs`).

## REQ-SYNC-0005: Threading of the job

**Status:** Draft

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

**Status:** Draft

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

**Status:** Draft

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
`during_an_outage_of_several_ticks_reads_report_it_writes_wait_and_recovery_drains`
(`tests/outage.rs`); `an_outage_of_several_ticks_through_the_real_store_and_cli` (`tests/e2e.rs`).

## REQ-SYNC-0008: The stop rule never stops on uncertainty

**Status:** Draft

`stop_reason` stops the schedule when no phase is active and no journal entry
is pending or escalated-and-unannounced. It answers "keep going" when the
`[tick]` table would be refused, the phases directory or a phase file cannot be
read, a root cannot be read, or the store fails.

**Refines:** REQ-BDC-0707
**Acceptance evidence:** `test_stop_only_when_no_active_phase_and_no_open_work`,
`a_phase_whose_root_cannot_be_read_never_ends_the_schedule`,
`configuration_the_tick_would_refuse_never_ends_the_schedule` (`tests/stop_rule.rs`).

## REQ-SYNC-0009: `atm send` is bounded and carries the message on stdin

**Status:** Draft

`AtmSender` runs `atm send <lead>@<team> --team tool --as cron --stdin` and
writes the message to stdin. An address that could be read as an option, or
split in two, is refused before a process starts. A child still running after
the timeout (10 s) is killed and the send fails; a nonzero exit fails with the
child's stderr.

**Refines:** REQ-BDC-0704..0706
**Acceptance evidence:** `tests/atm_process.rs` (its own fake `atm`, 7 cases).

## REQ-SYNC-0010: The end-to-end flow runs through the real composition

**Status:** Draft

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
