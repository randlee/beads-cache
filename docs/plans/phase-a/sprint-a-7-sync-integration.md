---
id: a-7
phase: a
status: in-review
closure_type: integration
target_boundary: bd-cache-sync and phase composition
vertical_rationale: "The bd-cache-sync boundary does not exist yet: this sprint creates it, then composes it with sc-bd-cli, sc-cron, bd-cache-store and bd-cache. Its only contract (EscalationSender) is already frozen in a-1."
track: integration
wave: 3
branch: sprint/a-7-sync-integration
worktree: sprint/a-7-sync-integration
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [a-2, a-3, a-4, a-5, a-6]
parallel_safe: [a-8]
requirements:
  owns: [REQ-BDC-0003, REQ-BDC-0701, REQ-BDC-0702, REQ-BDC-0703, REQ-BDC-0704, REQ-BDC-0705, REQ-BDC-0706, REQ-BDC-0707, REQ-BDC-0903, NFR-BDC-0001, NFR-BDC-0003, REQ-SYNC-0001, REQ-SYNC-0002, REQ-SYNC-0003, REQ-SYNC-0004, REQ-SYNC-0005, REQ-SYNC-0006, REQ-SYNC-0007, REQ-SYNC-0008, REQ-SYNC-0009, REQ-SYNC-0010]
  constrained_by: [REQ-BDC-0101, REQ-BDC-0401, REQ-BDC-0402, REQ-BDC-0405, REQ-BDC-0503, REQ-BDC-0604, REQ-BDC-0904, REQ-BDC-0905, REQ-BDC-0911, REQ-BDC-0912, NFR-BDC-0004, NFR-BDC-0005, NFR-BDC-0006, NFR-BDC-0007, REQ-CORE-0001, REQ-CORE-0002]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0006, ADR-BDC-0007, ADR-CORE-0001, ADR-CORE-0101, ADR-CORE-0202, ADR-CRON-0005, ADR-STORE-0003, ADR-SYNC-0001, ADR-SYNC-0002, ADR-SYNC-0003, ADR-SYNC-0004, ADR-SYNC-0005]
owned_paths:
  - "crates/bd-cache-sync/**"
  - "crates/bd-cache-sync/benches/**"
  - "crates/bd-cache-sync/tests/**"
  - ".claude/project/quality-policy.md"

  - "docs/bd-cache-sync/**"
  - "docs/plans/phase-a/evidence/**"
  - "docs/architecture.md"
---

# Sprint a-7 — sync task and phase integration

## Outcome and closure

Create `bd-cache-sync`, compose it with the real `sc-bd-cli`, `sc-cron`,
`bd-cache-store` and `bd-cache`, and close every feature-level criterion of
phase A. Closes when the cron cases pass, the end-to-end flow passes through
the real composition, and `docs/architecture.md` matches what shipped.

## must_follow rationale

Composes `BdCli` (a-2), the scheduler (a-3), `SqliteStore` (a-4) and the
write and read paths (a-5, a-6).

## Deliverables

1. `bd-cache-sync` tick (REQ-BDC-0703): 25 s budget ending at T; drain with
   `DrainBudget { start_gate: T - 5 s, hard: T - 2 s }`, then sync each
   active scope with deadline T, deferring scopes not started by T - 2 s;
   sc-cron's run `Instant` becomes T through `Deadline::at`; one schedule per repository with a team and a required `fallback_lead`
   for escalations whose entry has no scope (REQ-BDC-0701); active phases from `.atm-bd/phase-*.toml` and the
   configured list (REQ-BDC-0702); stop rule (REQ-BDC-0707); scheduled on `sc-cron`. Threading per ADR-CORE-0001: the `Cache` is held in `Arc<Mutex<_>>` and
   called under `spawn_blocking` with the tick deadline; a tick that finds
   the mutex held (including a blocking task left over from a timed-out
   tick) is skipped and reported busy; `should_stop` returns Continue when
   busy; a poisoned mutex is recovered and reported in the tick reply.
2. Announcements (REQ-BDC-0704..0706): sync failures once per cause,
   escalations once with the `resolve` command, undelivered messages
   refused and retried, through a-1's `EscalationSender`; an implementation
   that runs `atm send <lead>@<team> --team tool` as `cron`. An outage is announced through the per-scope sync alert with the cause
   key `backend_unavailable`, so each active scope's lead hears it once;
   when it ends the alert is reset and no message is sent; a failed send is
   refused and retried like any announcement (REQ-BDC-0704, 0706). The tick is a library entry point,
   `tick(&mut Cache, &TickConfig, &dyn EscalationSender, end: Deadline)`, so it
   can be driven by `sc-cron` here or by an external scheduler such as the
   ATM cron module through the later `bdc tick` (the Python path composes
   `drain`, `sync` and the announcement verbs instead); only
   one ticker may run per cache file. The tick reads its keys from
   `RepoConfig.tick` via a-4's `load_repo_config`.
3. End-to-end tests (`crates/bd-cache-sync/tests/`): a journaled write applied by a scheduled
   drain through `BdCli` with a recording invoker in CI, and against a
   scratch `bd` workspace on the testbed (REQ-BDC-0003); an outage of
   several ticks (REQ-BDC-0903).
4. Performance: in-process cached `get` on `SqliteStore`, asserted by a
   test on the median of N calls (NFR-BDC-0001; a criterion bench in
   `crates/bd-cache-sync/benches/` is informational); incremental sync of a 1000-bead scope on the testbed
   (NFR-BDC-0003).
5. Testbed evidence (team-lead on `m5-atmbench`): the a-2 live probes, the
   end-to-end run against scratch `bd`, and the sync timing, recorded in
   `docs/plans/phase-a/evidence/`. If the testbed is unavailable, the
   recording-invoker results close the sprint and the testbed items are
   recorded as deferred evidence with an owner and date.
6. Repository bookkeeping: update ADR-BDC statuses and Crate Layout in `docs/architecture.md` from
   the crate docs; add the phase-end validation command to the Validation
   Policy in `.claude/project/quality-policy.md`; `docs/bd-cache-sync/`
   requirements and architecture.

- **bd-compat workflow, real run (deferred from a-2, lead ruling bc-lead 2026-10-24).** After develop→main at phase close-out, run `.github/workflows/bd-compat.yml` once by `workflow_dispatch` at the pinned bd version (expect pass) and once by `repository_dispatch` with a different version (expect a failure that opens one issue with the `bd schema` diff). a-2's proof-branch runs 40073075092 and 40073178515 are its interim evidence.

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_tick_syncs_only_phases_with_an_open_root`, `test_custom_phases_limits_sync_and_stop`, `test_malformed_custom_phases_is_refused` | req:REQ-BDC-0702 |
| `test_missing_team_is_refused` | req:REQ-BDC-0701 |
| `test_tick_budget_defers_drain_entries`; no tick exceeds 25 s with every `bd` call timing out | req:REQ-BDC-0703 |
| `test_sync_failure_is_announced_once_per_cause` | req:REQ-BDC-0704 |
| `test_escalation_is_sent_once_to_the_phase_lead` | req:REQ-BDC-0705 |
| `test_undelivered_escalation_is_retried_and_the_tick_refuses` | req:REQ-BDC-0706 |
| `test_stop_only_when_no_active_phase_and_no_open_work` | req:REQ-BDC-0707 |
| `test_backend_outage_is_announced_once_and_sync_failures_are_not`, `test_outage_with_uncached_roots_still_tells_every_configured_lead`, `test_outage_is_not_cleared_while_only_the_backend_check_answers`, `test_undelivered_outage_announcement_is_retried` | req:REQ-BDC-0903 |
| `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`, through the real store | req:REQ-BDC-0003 |
| A write journaled through the core lands in `bd` via the scheduled drain and is verified by read-back, end to end | req:REQ-BDC-0003 |
| During an outage of several ticks, reads of cached beads answer with
`outage` set in `GetReply`/`ListReply`, no escalation is sent, and the journal drains on recovery (the attempts rule itself is a-5's) | req:REQ-BDC-0903 |
| Median in-process cached `get` on `SqliteStore` under 1 ms over N calls (test) | req:NFR-BDC-0001 |
| Incremental sync of a 1000-bead scope under 1 s on the testbed, recorded (or deferred evidence per deliverable 5) | req:NFR-BDC-0003 |
| A second tick while a blocking sync is still running is skipped and reported busy | req:REQ-BDC-0703 |
| An outage is announced once per scope with a lead under `backend_unavailable` and reset when it ends; a scope without a lead uses `fallback_lead` | req:REQ-BDC-0704 |
| `test_e2e_blocker_close_and_reopen_update_ready`: a journaled close of a cached blocker, applied by a scheduled drain on the real store, makes its depender ready in the same tick's sync; reopen makes it blocked again; the same for a blocker outside every scope | req:REQ-BDC-0904 |
| An escalation on an entry with no scope goes to `fallback_lead`, and the stop rule still fires | req:REQ-BDC-0706 |
| `docs/architecture.md` ADRs and Crate Layout match the shipped crates | phase plan |
| Planted bugs: unannounced filter dropped, open-work ignores announcement, refusal dropped, team check dropped, closed roots active, stop ignores open work each fail a test | req:REQ-BDC-0705 |

## Required validation

Full workspace validation; the end-to-end tests; the testbed run recorded.

## Requirement scope notes

- **REQ-BDC-0405** (read-back after a write; a-5 owns the rule): a-7 runs it through the real
  store and `BdCli`: `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`
  (`tests/e2e.rs`).
- **REQ-BDC-0503** (a stale row is re-read by the next sync; a-6): the same test.
- **REQ-BDC-0904, 0911, 0912** (blocked state and `ready`; a-6): a-7 adds
  `test_e2e_blocker_close_and_reopen_update_ready` on the real store; 0911 and 0912 are a-6's
  rules and are not re-tested.
- **REQ-BDC-0101, 0401, 0402, 0604** (cache location, drain lock, drain budget, escalation
  bookkeeping; a-4 and a-5): `compose` uses the first two; the tick passes the third; the sender
  loop uses the fourth through `unannounced_escalations` / `mark_announced`.
- **REQ-BDC-0905** (bd release compatibility): a-7 owns the real `workflow_dispatch` /
  `repository_dispatch` run (lead ruling, bc-lead, 2026-10-24); see "bd-compat workflow" below.
- **NFR-BDC-0004** (calls capped by the deadline): `no_tick_exceeds_25_s_with_every_bd_call_timing_out`.
- **NFR-BDC-0005** (planted bugs) and **NFR-BDC-0006** (live tests only on the testbed or in the
  compat workflow): see the evidence sections.
- **NFR-BDC-0007** (one SQLite connection per process): the store refuses a second connection to
  the cache file, which is how "one ticker per cache file" is enforced inside a process; across
  processes it is the caller's rule.
- **REQ-SYNC-0001..0010 and ADR-SYNC-0001..0005** are this crate's own ids
  (`docs/bd-cache-sync/`), created by this sprint and **Draft until the lead rules** (see
  Hand-off).

## Deviations from this document

1. **`tick` takes the clock.** Deliverable 2 gives
   `tick(&mut Cache, &TickConfig, &dyn EscalationSender, end: Deadline)`. The shipped signature is
   `tick(&mut Cache, &TickConfig, &dyn EscalationSender, &dyn Clock, end: Deadline)`. The tick must
   defer a sync that has not started by `T - 2 s` and size the root read by the time left, both on
   the clock the cache and its backend share, and `Cache` does not expose its clock. A
   `Cache::clock()` accessor (an a-1 amendment) would restore the four-parameter form; ADR-SYNC-0001.
2. **Phase file format.** BD-CACHE.md says only "`.atm-bd/phase-*.toml` (`root`, `lead`)" and the
   Python reference is not in this repository. A phase file is `phase-<x>.toml` with a string
   `root` and an optional string `lead`; other keys are ignored. `phases_dir` defaults to
   `.atm-bd`, relative to the working directory (the beads root, where the schedule is started).
   REQ-SYNC-0002. Confirm against `bd_cache_cron.py` before the cutover.
3. **Fallback lead.** REQ-BDC-0706 says an entry whose phase has no lead makes the tick `refused`;
   the acceptance criterion for REQ-BDC-0704 says a scope without a lead uses `fallback_lead`.
   Both are implemented as written: an *alert about the backend* (a sync failure or an outage)
   for a phase without a lead goes to `fallback_lead`; an *escalated entry* whose phase has no
   lead is refused (an entry with no scope goes to `fallback_lead`). ADR-SYNC-0003.
4. **The stop rule requires positive knowledge** (ADR-SYNC-0004): where BD-CACHE's cron counts an
   unreadable root as inactive for the stop rule, `stop_reason` keeps going.
5. **Path dependencies.** `bd-cache-store`, `sc-bd-cli` and `sc-cron` are not in
   `[workspace.dependencies]` (a-1 listed only the crates sprints 2 to 6 consume), so
   `crates/bd-cache-sync/Cargo.toml` names them by path. The lead can add them to the table and
   the crate can switch to `workspace = true` without code changes.
6. **New third-party edges** (the Crate Layout row is updated): `toml` (phase files), `serde`,
   `serde_json`, `async-trait`, `tracing`, `tokio`; dev: `chrono` (sc-cron's `ManualClock` takes
   a `chrono::Duration`), `criterion`, `tempfile`. All are in the workspace table already.
7. **Sprint status.** The cursor task's gate asks for `status: complete`; the lead notes for
   wave 2 (QA gate: zero findings, CI green) ask for `in-review` at hand-off. This doc says
   `in-review`; the lead sets `complete` at merge.
8. **NFR-BDC-0001 is asserted on a 100-bead scope**, and does not hold for 1000 beads
   (Findings, F-1).
9. **bd-compat real run**: `workflow_dispatch` ran for real (twice); `repository_dispatch` could
   not (below).

## Acceptance evidence

CI-run tests use `FakeBackend`, `FakeStore` or, for the end-to-end rows, the real composition
with `FakeBd` as the `bd` process. Files are in `crates/bd-cache-sync/tests/`.

| Criterion | Tests |
|---|---|
| REQ-BDC-0702 | `test_tick_syncs_only_phases_with_an_open_root`, `test_custom_phases_limits_sync_and_stop`, `test_malformed_custom_phases_is_refused`, `a_root_the_cache_holds_is_read_from_the_cache_and_a_closing_root_ends_the_phase`, `a_configured_phase_without_a_file_and_a_broken_file_are_refused_but_the_rest_runs`, `an_unreadable_phases_directory_is_refused_and_the_journal_still_drains` (`tick_phases.rs`) |
| REQ-BDC-0701 | `test_missing_team_is_refused` (`tick_phases.rs`) |
| REQ-BDC-0703 | `test_tick_budget_defers_drain_entries`, `no_tick_exceeds_25_s_with_every_bd_call_timing_out`, `the_drain_runs_before_the_sync` (`tick_budget.rs`); `a_second_tick_while_a_blocking_sync_is_running_is_skipped_and_reported_busy`, `a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`, `should_stop_returns_continue_when_busy`, `a_poisoned_mutex_is_recovered_and_reported_in_the_tick_reply` (`job_threading.rs`) |
| REQ-BDC-0704 | `test_sync_failure_is_announced_once_per_cause`, `an_undelivered_sync_failure_is_refused_and_retried_not_recorded`, `a_phase_without_a_lead_hears_about_a_sync_failure_through_the_fallback_lead` (`announce.rs`); `a_sync_failure_caused_by_an_unreachable_backend_is_not_announced_per_phase` (unit, `announce.rs`); `a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once` (`outage.rs`) |
| REQ-BDC-0705 | `test_escalation_is_sent_once_to_the_phase_lead` (`announce.rs`); `an_escalation_is_announced_once_and_the_store_remembers_it` (`e2e.rs`, real store) |
| REQ-BDC-0706 | `test_undelivered_escalation_is_retried_and_the_tick_refuses`, `an_entry_whose_phase_has_no_lead_is_refused_every_tick_until_it_has_one`, `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires` (`announce.rs`); `an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried` (`e2e.rs`); `tests/atm_process.rs` (the sender, 7 cases) |
| REQ-BDC-0707 | `test_stop_only_when_no_active_phase_and_no_open_work`, `a_phase_whose_root_cannot_be_read_never_ends_the_schedule`, `configuration_the_tick_would_refuse_never_ends_the_schedule` (`stop_rule.rs`) |
| REQ-BDC-0903 (outage) | `test_backend_outage_is_announced_once_and_sync_failures_are_not`, `test_outage_with_uncached_roots_still_tells_every_configured_lead`, `test_outage_is_not_cleared_while_only_the_backend_check_answers`, `test_undelivered_outage_announcement_is_retried`, `the_alert_of_a_scope_that_is_not_synced_is_reset_when_the_outage_ends` (`outage.rs`) |
| Outage of several ticks | `during_an_outage_of_several_ticks_reads_report_it_writes_wait_and_recovery_drains` (`outage.rs`, fakes); `an_outage_of_several_ticks_through_the_real_store_and_cli` (`e2e.rs`, real store): reads answer with `outage` set, no escalation, no attempt counted, the journal drains on recovery |
| REQ-BDC-0003 | `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync` and `a_write_journaled_through_the_core_lands_in_bd_via_the_scheduled_drain` (`e2e.rs`: `SyncJob` on an `sc-cron` `Scheduler`, `SqliteStore`, `FlockDrainLock`, `BdCli` over `FakeBd`, which records the argv); the same flow against a scratch `bd`: `tests/live.rs` (testbed, deferred) |
| REQ-BDC-0904 | `test_e2e_blocker_close_and_reopen_update_ready` (`e2e.rs`): a journaled close of a cached blocker, applied by a scheduled drain on the real store, makes its depender ready in the same tick's sync; reopen blocks it again; the same for a blocker outside every scope |
| NFR-BDC-0001 | `median_in_process_cached_get_on_sqlite_store_is_under_1_ms` (`perf.rs`, 100-bead scope); **not met at 1000 beads**: `median_cached_get_on_a_1000_bead_scope_is_under_1_ms` (ignored; Findings F-1); `benches/get.rs` (informational) |
| NFR-BDC-0003 | `tests/live.rs::an_incremental_sync_of_a_1000_bead_phase_is_under_1_s` (testbed, deferred evidence: `docs/plans/phase-a/evidence/a-7-testbed.md`); developer smoke run 544 ms |
| `docs/architecture.md` matches the crates | Crate Layout rewritten from the shipped crates and the ADR implementation table added; the CI boundary check passes on the table |
| Planted bugs | below |

## Evidence: planted bugs (NFR-BDC-0005)

Run on commit `0332079`, the last commit that changes code (later commits change documents
only). A script (not committed) applied each mutation to the working tree (one literal
replacement, checked to match exactly once), ran the whole crate suite, and restored the file:

```text
cargo test -p bd-cache-sync --all-features --no-fail-fast
git checkout -- <file>
```

The six bugs the sprint names come first (the first two are the store's rules, planted in
`FakeStore`, and caught by the tick's tests); the rest are this sprint's own rules.

| Planted bug | File | Exit | Compile error | Failing tests |
|---|---|---|---|---|
| unannounced filter dropped | `crates/bd-cache-model/src/testing/store.rs` | 101 | no | `test_escalation_is_sent_once_to_the_phase_lead` |
| open-work ignores announcement | `crates/bd-cache-model/src/testing/store.rs` | 101 | no | `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`, `test_stop_only_when_no_active_phase_and_no_open_work` |
| refusal dropped | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried`, `test_undelivered_escalation_is_retried_and_the_tick_refuses` |
| team check dropped | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `configuration_the_tick_would_refuse_never_ends_the_schedule`, `test_missing_team_is_refused` |
| closed roots active | `crates/bd-cache-sync/src/active.rs` | 101 | no | `a_phase_whose_root_cannot_be_read_never_ends_the_schedule`, `a_root_the_cache_holds_is_read_from_the_cache_and_a_closing_root_ends_the_phase`, `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`, `configuration_the_tick_would_refuse_never_ends_the_schedule`, `should_stop_returns_continue_when_busy`, `test_custom_phases_limits_sync_and_stop`, `test_stop_only_when_no_active_phase_and_no_open_work`, `test_tick_syncs_only_phases_with_an_open_root`, `the_alert_of_a_scope_that_is_not_synced_is_reset_when_the_outage_ends` |
| stop ignores open work | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `test_stop_only_when_no_active_phase_and_no_open_work` |
| fallback lead dropped for entries with no scope | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires` |
| escalation not marked announced | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `an_escalation_is_announced_once_and_the_store_remembers_it`, `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`, `an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried`, `test_escalation_is_sent_once_to_the_phase_lead`, `test_stop_only_when_no_active_phase_and_no_open_work`, `test_undelivered_escalation_is_retried_and_the_tick_refuses` |
| escalation message without the resolve command | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`, `test_escalation_is_sent_once_to_the_phase_lead` |
| sync failure announced every tick (alert not compared) | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `test_sync_failure_is_announced_once_per_cause` |
| sync failure alert not recorded | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `announce::tests::a_sync_failure_caused_by_an_unreachable_backend_is_not_announced_per_phase`, `test_sync_failure_is_announced_once_per_cause` |
| sync success does not clear the alert | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `test_sync_failure_is_announced_once_per_cause` |
| outage alert not recorded after the send | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once`, `an_outage_of_several_ticks_through_the_real_store_and_cli`, `during_an_outage_of_several_ticks_reads_report_it_writes_wait_and_recovery_drains`, `test_backend_outage_is_announced_once_and_sync_failures_are_not`, `test_outage_is_not_cleared_while_only_the_backend_check_answers`, `test_outage_with_uncached_roots_still_tells_every_configured_lead`, `test_undelivered_outage_announcement_is_retried`, `the_alert_of_a_scope_that_is_not_synced_is_reset_when_the_outage_ends` |
| outage alert not reset when it ends | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `the_alert_of_a_scope_that_is_not_synced_is_reset_when_the_outage_ends` |
| outage: no lead for a scope without one | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `a_phase_without_a_lead_hears_about_a_sync_failure_through_the_fallback_lead`, `a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once`, `test_outage_with_uncached_roots_still_tells_every_configured_lead` |
| outage not told to every configured lead when no root is readable | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `test_outage_with_uncached_roots_still_tells_every_configured_lead` |
| outage message per phase instead of per lead | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once`, `an_outage_of_several_ticks_through_the_real_store_and_cli`, `test_backend_outage_is_announced_once_and_sync_failures_are_not`, `test_outage_with_uncached_roots_still_tells_every_configured_lead` |
| undelivered outage message recorded as delivered | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `test_undelivered_outage_announcement_is_retried` |
| sync failure of an unreachable cause announced per phase | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `announce::tests::a_sync_failure_caused_by_an_unreachable_backend_is_not_announced_per_phase` |
| drain without the start gate and hard deadline | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `test_tick_budget_defers_drain_entries` |
| sync not deferred before T - 2 s | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `test_tick_budget_defers_drain_entries` |
| sync run with no deadline (a day) | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `no_tick_exceeds_25_s_with_every_bd_call_timing_out` |
| custom phase list ignored | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `test_custom_phases_limits_sync_and_stop` |
| malformed phase entry skipped instead of refused | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `configuration_the_tick_would_refuse_never_ends_the_schedule`, `test_malformed_custom_phases_is_refused` |
| unreadable root stops the schedule | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `a_phase_whose_root_cannot_be_read_never_ends_the_schedule` |
| configuration the tick refuses can stop the schedule | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `configuration_the_tick_would_refuse_never_ends_the_schedule` |
| broken phase file does not hold the stop rule | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `configuration_the_tick_would_refuse_never_ends_the_schedule` |
| busy: stop check stops under a running task | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`, `should_stop_returns_continue_when_busy` |
| busy tick not reported (runs as an empty pass-through) | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`, `a_second_tick_while_a_blocking_sync_is_running_is_skipped_and_reported_busy` |
| poison not cleared (reported on every tick) | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_poisoned_mutex_is_recovered_and_reported_in_the_tick_reply` |
| poisoned mutex not reported | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_poisoned_mutex_is_recovered_and_reported_in_the_tick_reply` |
| a refused tick reported as ok to the scheduler | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`, `an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried` |
| schedule started without a team | `crates/bd-cache-sync/src/job.rs` | 101 | no | `test_missing_team_is_refused` |
| atm address not validated | `crates/bd-cache-sync/src/atm.rs` | 101 | no | `atm_process (process test)` |
| atm message not on stdin | `crates/bd-cache-sync/src/atm.rs` | 101 | no | `atm_process (process test)` |
| compose ignores the [tick] table | `crates/bd-cache-sync/src/compose.rs` | 101 | no | `a_write_journaled_through_the_core_lands_in_bd_via_the_scheduled_drain`, `an_escalation_is_announced_once_and_the_store_remembers_it`, `an_outage_of_several_ticks_through_the_real_store_and_cli`, `an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried`, `test_e2e_blocker_close_and_reopen_update_ready`, `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync` |

Every one of the 36 bugs fails at least one test, none by a compile error, and `git status`
was clean after the run. A first run of the same list (on `2bad273`) found three survivors, which
became tests: "outage alert not reset when it ends" (the alert of a scope that is not synced),
"sync not deferred before T - 2 s" (a deferred sync makes no bd call) and "sync failure of an
unreachable cause announced per phase" (a unit test, because the sync reports an unreachable
backend as an outage and never produces that cause); two patterns that did not match after
`cargo fmt` were fixed.

Contract suites: the store contract suite runs in `bd-cache-store` and `bd-cache-model`; a-7
implements no `CacheStore`, `CacheBackend` or `DrainLock`, so none applies here.

## bd-compat workflow, real run

Recorded in `docs/plans/phase-a/evidence/a-7-bd-compat.md`. Summary: `workflow_dispatch` cannot run
on the default branch before the file is on `main` (HTTP 422) but **can** be dispatched on a
branch ref. Two real `workflow_dispatch` runs on this branch: the pinned version passed
([40083096528](https://github.com/randlee/beads-cache/actions/runs/40083096528)); the failure path
with the `expect_schema_sha` hook failed as intended and opened one issue
([40083225647](https://github.com/randlee/beads-cache/actions/runs/40083225647), issue #12, closed
with a note). `repository_dispatch` always runs the default branch's copy and cannot run until
`main` has the file; it, and `workflow_dispatch` on `main`, remain for phase close-out (team-lead).

## Testbed

Deferred evidence, owner team-lead, date 2026-10-24: `docs/plans/phase-a/evidence/a-7-testbed.md`
(the commands, what each test does, and a developer-machine smoke run that is explicitly not
testbed evidence).

## Findings

- **F-1 (NFR-BDC-0001, a-6): a cached `get` is not under 1 ms on a large scope.** `Cache::get`
  calls `adjusted()` (`crates/bd-cache/src/ready.rs`), which loads the scope's whole ready base
  from the store and recomputes it with the pending and recently applied entries on every call,
  so a `get` costs time proportional to the scope. Measured on the real `SqliteStore`, median of
  1000 `get`s on this machine (macOS arm64; scope with a tenth of its beads blocked):

  | Beads in scope | release | debug |
  |---|---|---|
  | 10 | 22 us | 54 us |
  | 100 | 53 us | 160 us |
  | 300 | 154 us | 648 us |
  | 1000 | 1431 us | 5267 us |

  The target is met up to a few hundred beads in release and 300 in debug, not for a 1000-bead
  phase (the size NFR-BDC-0003 uses). The asserting test therefore uses 100 beads (a margin of 6
  in a debug CI build), and the 1000-bead assertion is an ignored test that fails today
  (`median_cached_get_on_a_1000_bead_scope_is_under_1_ms`) so the gap stays visible. The fix is
  in a-6's `get`/`ready` code, outside this sprint's paths: compute `blocked` for the one bead
  (its `blocked_by` from the stored base row) instead of adjusting the whole base, and adjust
  only when there is a pending or recently applied entry. **Needs a lead ruling** (a-6
  amendment, or accept the 1000-bead gap for phase a).
- **F-2: `Cache` does not expose its clock**, which is why `tick` takes one (Deviation 1).
- **F-3: `workflow_dispatch` can run from a branch ref** before the file is on `main`; the ruling
  and a-2's notes assumed it could not.
- **F-4: `docs/project-plan.md` still shows a-7 as `planned`** (not an a-7 path): the lead updates it.

## Validation

At `0332079` (the last code commit), locally (macOS arm64, Rust 1.98.1), each gate with
`set -o pipefail`:

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo clippy --workspace --lib -- -D warnings` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | pass |
| `cargo test --workspace --all-features` | pass (698 passed, 17 ignored) |
| CI boundary check (`cargo metadata` against the Boundaries table) | pass |

CI (per the CI-evidence convention in the phase plan): run
[40083978831](https://github.com/randlee/beads-cache/actions/runs/40083978831) is green on
ubuntu, macos and windows at `0332079`, the last code commit; later commits change documents only.

## Hand-off: Draft ids for the lead

Crate-level ids created by this sprint, all Draft: REQ-SYNC-0001..0010
(`docs/bd-cache-sync/requirements.md`) and ADR-SYNC-0001..0005
(`docs/bd-cache-sync/architecture.md`). Contract deviations to rule on: Deviation 1 (the `tick`
signature), Deviation 3 (fallback lead) and Findings F-1.
